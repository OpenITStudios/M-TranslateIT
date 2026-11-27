use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use super::evidence::{condition_asr_sample, noise_suppression_threshold};
use super::finalized_utterance::FinalizedMeetingUtterance;
use super::{duration_ms, TARGET_SAMPLE_RATE_HZ};
use crate::engine::paths::ProjectPaths;

const MIN_ASR_SEGMENT_DURATION_MS: u32 = 300;
// Safety/storage ceiling for the finalized producer only; this must not be used to
// force a speech boundary. Overlong speech is dropped by the producer rather than
// cut into a fake final utterance.
const MAX_FINALIZED_ASR_SEGMENT_SAMPLES: usize = TARGET_SAMPLE_RATE_HZ as usize * 60;
const FINALIZED_SEGMENT_ROOT_LABEL: &str = "UserData/CacheData/audio_segments/";

#[derive(Debug, Clone, Serialize)]
pub struct LiveSegmentWavWriteReport {
    pub ok: bool,
    pub audio_path: Option<String>,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub sample_count: usize,
    pub duration_ms: u32,
    pub blocker: String,
    pub note: String,
}

#[cfg(test)]
pub fn write_finalized_outbound_utterance_wav(
    utterance: &FinalizedMeetingUtterance,
) -> LiveSegmentWavWriteReport {
    write_finalized_outbound_utterance_wav_with_noise_suppression(utterance, false)
}

pub fn write_finalized_outbound_utterance_wav_with_noise_suppression(
    utterance: &FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
) -> LiveSegmentWavWriteReport {
    if utterance.lane != "you" || utterance.generation.is_none() {
        return invalid_lane_report(utterance, "you", "finalized_outbound_writer");
    }
    write_finalized_meeting_utterance_wav(utterance, noise_suppression_enabled)
}

#[cfg(test)]
pub fn write_finalized_incoming_utterance_wav(
    utterance: &FinalizedMeetingUtterance,
) -> LiveSegmentWavWriteReport {
    write_finalized_incoming_utterance_wav_with_noise_suppression(utterance, false)
}

pub fn write_finalized_incoming_utterance_wav_with_noise_suppression(
    utterance: &FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
) -> LiveSegmentWavWriteReport {
    if utterance.lane != "incoming" || utterance.generation.is_some() {
        return invalid_lane_report(utterance, "incoming", "finalized_incoming_writer");
    }
    write_finalized_meeting_utterance_wav(utterance, noise_suppression_enabled)
}

// Shared pure frame gate: the scheduler handoff can reject invalid input
// before queueing without creating a legacy file or changing the limits.
pub(crate) fn finalized_asr_frame_rejection(
    utterance: &FinalizedMeetingUtterance,
) -> Option<LiveSegmentWavWriteReport> {
    let frame = &utterance.frame;
    validate_target_frame(
        frame.sample_rate_hz,
        frame.channels,
        &frame.samples,
        duration_ms(frame.samples.len(), frame.sample_rate_hz),
        MAX_FINALIZED_ASR_SEGMENT_SAMPLES,
        "finalized_utterance_writer",
    )
}

fn write_finalized_meeting_utterance_wav(
    utterance: &FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
) -> LiveSegmentWavWriteReport {
    let frame = &utterance.frame;
    let frame_duration_ms = duration_ms(frame.samples.len(), frame.sample_rate_hz);
    if let Some(report) = finalized_asr_frame_rejection(utterance) {
        return report;
    }

    let session_component = safe_file_component(&utterance.session_id);
    let lane_component = safe_file_component(&utterance.lane);
    if session_component.is_empty() || lane_component.is_empty() || utterance.sequence == 0 {
        return LiveSegmentWavWriteReport {
            ok: false,
            audio_path: None,
            sample_rate_hz: frame.sample_rate_hz,
            channels: frame.channels,
            sample_count: frame.samples.len(),
            duration_ms: frame_duration_ms,
            blocker: "finalized_utterance_writer:invalid_event_identity".to_string(),
            note: "Finalized Meeting utterance WAV was not written because its session/lane/event identity is invalid."
                .to_string(),
        };
    }

    let generation_component = utterance
        .generation
        .map(|generation| format!("g{generation}_"))
        .unwrap_or_default();
    let filename = format!(
        "final_{}_s{}_{}_{}u{}.wav",
        session_component,
        utterance.sequence,
        lane_component,
        generation_component,
        utterance.utterance_id
    );
    let project_paths = ProjectPaths::discover();
    let audio_dir = PathBuf::from(project_paths.user_cache_dir).join("audio_segments");
    let audio_path = audio_dir.join(&filename);
    let label = format!("{FINALIZED_SEGMENT_ROOT_LABEL}{filename}");

    let noise_threshold = if noise_suppression_enabled {
        noise_suppression_threshold(&frame.samples)
    } else {
        None
    };

    match write_pcm16_wav_with_promotion(
        &audio_path,
        frame.sample_rate_hz,
        frame.channels,
        &frame.samples,
        noise_threshold,
        |staged, destination| fs::rename(staged, destination),
    ) {
        Ok(()) => LiveSegmentWavWriteReport {
            ok: true,
            audio_path: Some(label),
            sample_rate_hz: frame.sample_rate_hz,
            channels: frame.channels,
            sample_count: frame.samples.len(),
            duration_ms: frame_duration_ms,
            blocker: String::new(),
            note: format!(
                "Finalized {} utterance {} for Meeting event sequence {} was written once as temporary PCM16 WAV.",
                utterance.lane, utterance.utterance_id, utterance.sequence
            ),
        },
        Err(_error) => LiveSegmentWavWriteReport {
            ok: false,
            audio_path: None,
            sample_rate_hz: frame.sample_rate_hz,
            channels: frame.channels,
            sample_count: frame.samples.len(),
            duration_ms: frame_duration_ms,
            blocker: "finalized_utterance_writer:wav_write_failed".to_string(),
            note: "Failed to write finalized Meeting utterance WAV. No AI/output stage should consume this utterance and no finalized path is promoted."
                .to_string(),
        },
    }
}

fn invalid_lane_report(
    utterance: &FinalizedMeetingUtterance,
    expected_lane: &str,
    blocker_prefix: &str,
) -> LiveSegmentWavWriteReport {
    LiveSegmentWavWriteReport {
        ok: false,
        audio_path: None,
        sample_rate_hz: utterance.frame.sample_rate_hz,
        channels: utterance.frame.channels,
        sample_count: utterance.frame.samples.len(),
        duration_ms: duration_ms(utterance.frame.samples.len(), utterance.frame.sample_rate_hz),
        blocker: format!("{blocker_prefix}:lane_mismatch"),
        note: format!("Finalized utterance did not match the expected {expected_lane} lane contract."),
    }
}

// Stop and optional incoming disable must distinguish a removed/missing WAV
// from a failed filesystem deletion. A bool keeps existing best-effort event
// callers compatible while allowing the deferred queue to retain failed work.
pub fn remove_finalized_meeting_utterance_wav(audio_path: &str) -> bool {
    let project_paths = ProjectPaths::discover();
    let audio_dir = PathBuf::from(project_paths.user_cache_dir).join("audio_segments");
    remove_finalized_meeting_utterance_wav_in_dir(&audio_dir, audio_path)
}

fn remove_finalized_meeting_utterance_wav_in_dir(
    audio_dir: &PathBuf,
    audio_path: &str,
) -> bool {
    let Some(filename) = finalized_audio_filename(audio_path) else {
        return false;
    };
    match fs::remove_file(audio_dir.join(filename)) {
        Ok(()) => true,
        // A completed or duplicate cleanup is idempotent; no file remains.
        Err(error) if error.kind() == io::ErrorKind::NotFound => true,
        Err(_) => false,
    }
}

pub fn cleanup_finalized_meeting_session_wavs(session_id: &str) -> Result<usize, String> {
    let project_paths = ProjectPaths::discover();
    let audio_dir = PathBuf::from(project_paths.user_cache_dir).join("audio_segments");
    cleanup_finalized_meeting_session_wavs_in_dir(&audio_dir, session_id)
}

fn cleanup_finalized_meeting_session_wavs_in_dir(
    audio_dir: &PathBuf,
    session_id: &str,
) -> Result<usize, String> {
    let session_component = safe_file_component(session_id);
    // Cleanup cannot silently normalize an invalid identity into another
    // session's filename. Real Meeting session IDs already use this alphabet.
    if session_component.is_empty() || session_component != session_id {
        return Err("finalized_utterance_cleanup:invalid_session_id".to_string());
    }
    let entries = match fs::read_dir(audio_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err("finalized_utterance_cleanup:read_dir_failed".to_string()),
    };

    let mut removed = 0usize;
    for entry in entries {
        let entry = entry.map_err(|_| "finalized_utterance_cleanup:read_entry_failed".to_string())?;
        let file_type = entry
            .file_type()
            .map_err(|_| "finalized_utterance_cleanup:file_type_failed".to_string())?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !finalized_wav_belongs_to_session(&name, &session_component) {
            continue;
        }
        fs::remove_file(entry.path())
            .map_err(|_| "finalized_utterance_cleanup:remove_failed".to_string())?;
        removed = removed.saturating_add(1);
    }
    Ok(removed)
}

// Match the complete app-owned final_<session>_s<event>_<lane>_<id>
// grammar, not merely a session prefix. Session IDs can contain "_s",
// so starts_with("final_<session>_") can delete a different session's WAV.
fn finalized_wav_belongs_to_session(filename: &str, session: &str) -> bool {
    let prefix = format!("final_{session}_s");
    let Some(rest) = filename.strip_prefix(&prefix) else { return false };
    let stem = rest.strip_suffix(".wav.tmp")
        .or_else(|| rest.strip_suffix(".wav"));
    let Some(stem) = stem else { return false };
    let Some((sequence, tail)) = stem.split_once('_') else { return false };
    if sequence.parse::<u64>().is_err() {
        return false;
    }
    let utterance = if let Some(utterance) = tail.strip_prefix("incoming_u") {
        utterance
    } else if let Some(after_gen) = tail.strip_prefix("you_g") {
        let Some((generation, utterance)) = after_gen.split_once("_u") else {
            return false;
        };
        if generation.parse::<u64>().is_err() {
            return false;
        }
        utterance
    } else {
        return false;
    };
    utterance.parse::<u64>().is_ok()
}

fn finalized_audio_filename(audio_path: &str) -> Option<String> {
    let normalized = audio_path.trim().replace('\\', "/");
    let filename = normalized.strip_prefix(FINALIZED_SEGMENT_ROOT_LABEL)?;
    if filename.is_empty()
        || filename.contains('/')
        || filename.contains('\\')
        || !filename.starts_with("final_")
        || !filename.ends_with(".wav")
    {
        return None;
    }
    Some(filename.to_string())
}

fn validate_target_frame(
    sample_rate_hz: u32,
    channels: u16,
    samples: &[f32],
    frame_duration_ms: u32,
    max_samples: usize,
    blocker_prefix: &str,
) -> Option<LiveSegmentWavWriteReport> {
    if sample_rate_hz != TARGET_SAMPLE_RATE_HZ || channels != 1 {
        return Some(LiveSegmentWavWriteReport {
            ok: false,
            audio_path: None,
            sample_rate_hz,
            channels,
            sample_count: samples.len(),
            duration_ms: frame_duration_ms,
            blocker: format!("{blocker_prefix}:frame_not_target_format"),
            note: "Finalized audio must be 16 kHz mono before worker transcription.".to_string(),
        });
    }

    if samples.is_empty() || samples.len() > max_samples {
        return Some(LiveSegmentWavWriteReport {
            ok: false,
            audio_path: None,
            sample_rate_hz,
            channels,
            sample_count: samples.len(),
            duration_ms: frame_duration_ms,
            blocker: format!("{blocker_prefix}:sample_count_out_of_range"),
            note: "Audio sample count is outside the safe WAV writer range.".to_string(),
        });
    }

    if frame_duration_ms < MIN_ASR_SEGMENT_DURATION_MS {
        return Some(LiveSegmentWavWriteReport {
            ok: false,
            audio_path: None,
            sample_rate_hz,
            channels,
            sample_count: samples.len(),
            duration_ms: frame_duration_ms,
            blocker: format!("{blocker_prefix}:segment_too_short"),
            note: format!(
                "Finalized audio is too short for the current ASR writer contract. Minimum: {MIN_ASR_SEGMENT_DURATION_MS}ms."
            ),
        });
    }

    None
}

fn safe_file_component(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
        .take(96)
        .collect()
}

pub(crate) fn write_pcm16_wav(
    path: &PathBuf,
    sample_rate_hz: u32,
    channels: u16,
    samples: &[f32],
) -> io::Result<()> {
    write_pcm16_wav_with_promotion(
        path, sample_rate_hz, channels, samples, None, |staged, destination| {
            fs::rename(staged, destination)
        },
    )
}

// The small promotion seam makes rename-failure behavior testable on any OS.
// It is still the filesystem's native atomic rename in production.
fn write_pcm16_wav_with_promotion(
    path: &PathBuf,
    sample_rate_hz: u32,
    channels: u16,
    samples: &[f32],
    noise_threshold: Option<f32>,
    promote: impl FnOnce(&PathBuf, &PathBuf) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp_path = path.with_extension("wav.tmp");
    // Assemble the complete, bounded WAV payload before reserving staging.
    // A pre-existing stage is owned by another attempt or a crashed writer:
    // never truncate or delete it in response to an admission failure.
    let bits_per_sample = 16u16;
    let bytes_per_sample = bits_per_sample / 8;
    let block_align = channels * bytes_per_sample;
    let byte_rate = sample_rate_hz * u32::from(block_align);
    let data_size = (samples.len() * usize::from(bytes_per_sample)) as u32;
    let riff_size = 36u32.saturating_add(data_size);
    // Keep one buffered PCM16 write, never thousands of tiny sample writes.
    let mut wav = Vec::with_capacity(44usize.saturating_add(data_size as usize));
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate_hz.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());

    // Apply the existing optional gain directly before PCM16 quantization.
    // The finalized f32 frame stays unchanged and no filtered Vec is needed.
    for sample in samples {
        let value = (condition_asr_sample(*sample, noise_threshold) * i16::MAX as f32).round() as i16;
        wav.extend_from_slice(&value.to_le_bytes());
    }
    // Exclusive creation prevents two writers from truncating the same
    // deterministic .wav.tmp. Failing to reserve leaves the existing stage
    // untouched, and must NOT remove its owner's staged bytes.
    let mut staged = OpenOptions::new().write(true).create_new(true).open(&temp_path)?;
    let written = staged.write_all(&wav);
    // Close the write handle before promotion, including on write failure.
    drop(staged);
    let promoted = written.and_then(|()| promote(&temp_path, path));
    if promoted.is_err() {
        // Only this call's exclusively created stage may be removed. An
        // unsuccessful rename must not delete an existing valid destination.
        let _ = fs::remove_file(&temp_path);
    }
    promoted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "translateit-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn noise_suppression_attenuates_floor_without_reducing_strong_speech() {
        let mut samples = vec![0.002_f32; 320];
        samples.extend(vec![0.12_f32; 320]);
        let threshold = noise_suppression_threshold(&samples).expect("strong speech should allow cleanup");
        assert!(condition_asr_sample(samples[0], Some(threshold)).abs() < samples[0].abs() * 0.4);
        assert!((condition_asr_sample(samples[400], Some(threshold)) - samples[400]).abs() <= 1e-6);
    }

    #[test]
    fn noise_suppression_bypasses_quiet_or_clipping_audio() {
        assert!(noise_suppression_threshold(&vec![0.002_f32; 640]).is_none());
        let clipping = (0..640).map(|index| if index % 10 == 0 { 1.0 } else { 0.1 }).collect::<Vec<_>>();
        assert!(noise_suppression_threshold(&clipping).is_none());
    }

    #[test]
    fn fused_noise_gain_preserves_previous_pcm16_bytes() {
        let root = unique_test_root("wav-fused-gain");
        let mut samples = vec![0.002_f32; 320];
        samples.extend(vec![0.12_f32; 320]);
        samples.extend([0.004, -0.009, f32::NAN, f32::INFINITY, -0.002]);
        let threshold = noise_suppression_threshold(&samples).unwrap();
        let transition_end = threshold * 2.5;
        // Previous implementation, retained only as a byte-parity oracle.
        let filtered: Vec<f32> = samples.iter().map(|sample| {
            let value = condition_asr_sample(*sample, None);
            let level = value.abs();
            let gain = if level <= threshold {
                0.22
            } else if level < transition_end {
                0.22 + 0.78 * ((level - threshold) / (transition_end - threshold))
            } else {
                1.0
            };
            value * gain
        }).collect();
        let reference = root.join("reference.wav");
        let fused = root.join("fused.wav");
        write_pcm16_wav(&reference, TARGET_SAMPLE_RATE_HZ, 1, &filtered).unwrap();
        write_pcm16_wav_with_promotion(
            &fused, TARGET_SAMPLE_RATE_HZ, 1, &samples, Some(threshold),
            |staged, destination| fs::rename(staged, destination),
        ).unwrap();
        assert_eq!(fs::read(reference).unwrap(), fs::read(fused).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn direct_pcm_conditioning_matches_gain_without_quantization_or_allocation() {
        use super::super::evidence::{condition_asr_pcm, AudioEvidenceReport};
        let mut mixed = vec![0.002_f32; 320];
        mixed.extend(vec![0.12_f32; 320]);
        mixed.extend([0.004, -0.009, f32::NAN, f32::INFINITY, -0.002]);
        let mut clipping = vec![0.1; 640];
        clipping[..16].fill(0.98); // exact clipping boundary
        let mut over_clipping = clipping.clone();
        over_clipping[16] = -1.0;
        for original in [
            vec![], vec![0.1; 319], vec![0.012; 320],
            vec![0.002; 640], mixed, clipping, over_clipping,
            vec![-2.0, 2.0, f32::NAN, f32::NEG_INFINITY],
            vec![0.12; MAX_FINALIZED_ASR_SEGMENT_SAMPLES],
        ] {
            // Old full-report threshold remains only as a parity oracle.
            let evidence = AudioEvidenceReport::from_samples(&original);
            let reference_threshold = if original.len() < 320
                || evidence.rms < 0.012 || evidence.clipping_ratio > 0.025 {
                None
            } else {
                Some((evidence.rms * 0.08).clamp(0.0015, 0.006))
            };
            assert_eq!(noise_suppression_threshold(&original), reference_threshold);
            for enabled in [false, true] {
                let mut pcm = original.clone();
                let allocation = pcm.as_ptr();
                let capacity = pcm.capacity();
                let threshold = if enabled { reference_threshold } else { None };
                condition_asr_pcm(&mut pcm, enabled);
                assert_eq!(pcm.as_ptr(), allocation);
                assert_eq!(pcm.capacity(), capacity);
                assert_eq!(pcm.len(), original.len());
                for (actual, &sample) in pcm.iter().zip(&original) {
                    assert!(actual.is_finite() && (-1.0..=1.0).contains(actual));
                    assert_eq!(actual.to_bits(), condition_asr_sample(sample, threshold).to_bits());
                }
            }
        }
    }

    #[test]
    fn pcm16_writer_emits_canonical_riff_payload() {
        let root = unique_test_root("wav-canonical");
        fs::create_dir_all(&root).expect("create test root");
        let destination = root.join("sample.wav");

        write_pcm16_wav(&destination, TARGET_SAMPLE_RATE_HZ, 1, &[0.25, -0.25])
            .expect("write canonical wav");
        let bytes = fs::read(&destination).expect("read canonical wav");
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 4);
        assert_eq!(bytes.len(), 48);
        assert_eq!(&bytes[44..48], &[0x00, 0x20, 0x00, 0xE0]);

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn session_cleanup_removes_only_matching_transient_audio() {
        let root = unique_test_root("session-wav-cleanup");
        fs::create_dir_all(&root).expect("create test root");
        fs::write(root.join("final_session-a_s1_you_g1_u1.wav"), b"a").expect("write matching wav");
        fs::write(root.join("final_session-a_s2_incoming_u2.wav.tmp"), b"b")
            .expect("write matching temp");
        fs::write(root.join("final_session-b_s1_you_g1_u1.wav"), b"c")
            .expect("write foreign wav");
        fs::write(root.join("final_session-a_something_s3_you_g1_u1.wav"), b"d")
            .expect("write overlapping session prefix");
        fs::write(root.join("final_session-a_sbad_you_g1_u1.wav"), b"e")
            .expect("write misleading non-event file");
        fs::create_dir_all(root.join("final_session-a_directory.wav"))
            .expect("create matching-looking directory");

        let removed = cleanup_finalized_meeting_session_wavs_in_dir(&root, "session-a")
            .expect("cleanup matching session");

        assert_eq!(removed, 2);
        assert!(!root.join("final_session-a_s1_you_g1_u1.wav").exists());
        assert!(!root.join("final_session-a_s2_incoming_u2.wav.tmp").exists());
        assert!(root.join("final_session-b_s1_you_g1_u1.wav").exists());
        assert!(root.join("final_session-a_something_s3_you_g1_u1.wav").exists(),
            "another session with a common prefix must survive");
        assert!(root.join("final_session-a_sbad_you_g1_u1.wav").exists());
        assert!(root.join("final_session-a_directory.wav").is_dir());

        fs::remove_dir_all(root).expect("remove test root");
    }

    #[test]
    fn invalid_cleanup_session_does_not_normalize_into_another_owner() {
        let root = unique_test_root("invalid-cleanup-session");
        fs::create_dir_all(&root).expect("isolated cleanup root");
        let foreign = root.join("final_session-a_s1_you_g2_u1.wav");
        fs::write(&foreign, b"foreign").expect("foreign WAV");
        assert_eq!(
            cleanup_finalized_meeting_session_wavs_in_dir(&root, "session a"),
            Err("finalized_utterance_cleanup:invalid_session_id".to_string()),
        );
        assert!(foreign.exists());
        fs::remove_dir_all(root).expect("remove isolated root");
    }

    #[test]
    fn per_wav_cleanup_reports_failure_and_accepts_already_missing_file() {
        let root = unique_test_root("per-wav-deferred-cleanup");
        fs::create_dir_all(&root).expect("create isolated wav cleanup root");
        let owned = "UserData/CacheData/audio_segments/final_session-a_s1_incoming_u1.wav";
        let path = root.join("final_session-a_s1_incoming_u1.wav");
        assert!(remove_finalized_meeting_utterance_wav_in_dir(&root, owned),
            "an already-absent per-event WAV is clean");
        fs::create_dir_all(&path).expect("block file removal with a directory");
        assert!(!remove_finalized_meeting_utterance_wav_in_dir(&root, owned),
            "failed removal must remain observable for Retry Stop");
        assert!(path.is_dir());
        fs::remove_dir(&path).expect("release directory blocker");
        fs::write(&path, b"wav").expect("create owned WAV");
        assert!(remove_finalized_meeting_utterance_wav_in_dir(&root, owned));
        assert!(!path.exists());
        assert!(!remove_finalized_meeting_utterance_wav_in_dir(
            &root, "UserData/CacheData/audio_segments/not-a-final-wav.tmp",
        ), "an invalid path cannot count as successful cleanup");
        fs::remove_dir_all(root).expect("remove isolated test root");
    }

    #[test]
    fn successful_promotion_still_replaces_a_completed_draft() {
        let root = unique_test_root("wav-replace-success");
        fs::create_dir_all(&root).expect("create isolated WAV test root");
        let destination = root.join("draft.wav");
        write_pcm16_wav(&destination, TARGET_SAMPLE_RATE_HZ, 1, &[0.2, -0.2])
            .expect("publish first complete draft");
        let first = fs::read(&destination).unwrap();
        write_pcm16_wav(&destination, TARGET_SAMPLE_RATE_HZ, 1, &[0.7, -0.7])
            .expect("native rename should replace a complete draft");
        let second = fs::read(&destination).unwrap();
        assert_ne!(first, second, "the complete retake replaces the old draft");
        assert_eq!(&second[..4], b"RIFF");
        assert!(!destination.with_extension("wav.tmp").exists());
        fs::remove_dir_all(root).expect("remove isolated WAV test root");
    }

    #[test]
    fn failed_rename_preserves_valid_destination_and_cleans_owned_stage() {
        let root = unique_test_root("wav-rename-failure");
        fs::create_dir_all(&root).expect("create isolated WAV test root");
        let destination = root.join("preserved.wav");
        write_pcm16_wav(&destination, TARGET_SAMPLE_RATE_HZ, 1, &[0.25, -0.25])
            .expect("publish first valid WAV");
        let previous = fs::read(&destination).expect("read original WAV");
        let failed = write_pcm16_wav_with_promotion(
            &destination,
            TARGET_SAMPLE_RATE_HZ, 1, &[0.75, -0.75], None,
            |staged, target| {
                assert!(staged.is_file(), "complete staging WAV exists before promotion");
                assert_eq!(fs::read(target).unwrap(), previous);
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            },
        );
        assert_eq!(failed.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(&destination).unwrap(), previous,
            "a failed rename must never remove an already-valid WAV");
        assert!(!destination.with_extension("wav.tmp").exists(),
            "only this writer's failed stage may be removed");
        fs::remove_dir_all(root).expect("remove isolated WAV test root");
    }

    #[test]
    fn occupied_staging_file_remains_unchanged_and_blocks_competing_writer() {
        let root = unique_test_root("wav-stage-collision");
        fs::create_dir_all(&root).expect("create isolated WAV test root");
        let destination = root.join("owned.wav");
        write_pcm16_wav(&destination, TARGET_SAMPLE_RATE_HZ, 1, &[0.2, -0.2])
            .expect("publish original WAV");
        let previous = fs::read(&destination).unwrap();
        let stage = destination.with_extension("wav.tmp");
        fs::write(&stage, b"staging-from-another-writer").expect("create foreign stage");
        let attempted = write_pcm16_wav(&destination, TARGET_SAMPLE_RATE_HZ, 1, &[0.8, -0.8]);
        assert_eq!(attempted.unwrap_err().kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&destination).unwrap(), previous);
        assert_eq!(fs::read(&stage).unwrap(), b"staging-from-another-writer",
            "occupied staging must not be truncated or deleted");
        fs::remove_dir_all(root).expect("remove isolated WAV test root");
    }

    #[test]
    fn failed_wav_promotion_removes_partial_temp_file() {
        let root = unique_test_root("wav-failure-cleanup");
        fs::create_dir_all(&root).expect("create test root");
        let destination = root.join("blocked.wav");
        fs::create_dir_all(&destination).expect("create directory at destination path");
        let temp_path = destination.with_extension("wav.tmp");

        let result = write_pcm16_wav(&destination, TARGET_SAMPLE_RATE_HZ, 1, &[0.25, -0.25]);

        assert!(result.is_err(), "directory destination must reject WAV promotion");
        assert!(
            !temp_path.exists(),
            "failed WAV promotion must not leave a partial temp file"
        );
        assert!(destination.is_dir(), "failed promotion must not remove the blocking directory");

        fs::remove_dir_all(root).expect("remove test root");
    }
}
