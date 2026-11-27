use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

use super::evidence::safe_sample;
#[cfg(test)]
use super::TARGET_SAMPLE_RATE_HZ;
use crate::engine::paths::ProjectPaths;

// Only old session-owned ASR artifacts can be cleaned here. New Meeting
// ASR uses PCM directly. The PCM16 writer below is used by My Voice
// recordings and explicit public-audio fixture export, not Meeting inference.
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
        path, sample_rate_hz, channels, samples, |staged, destination| {
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

    // Preserve existing recording quantization, without ASR-specific gain.
    for sample in samples {
        let value = (safe_sample(*sample) * i16::MAX as f32).round() as i16;
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
            TARGET_SAMPLE_RATE_HZ, 1, &[0.75, -0.75],
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
