use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use crate::engine::audio::guided_take::{
    review_guided_samples, resample_guided_native_for_review,
    GuidedTakeReview, GUIDED_TAKE_SAMPLE_RATE_HZ,
};

// Disk files are not a review decision. Discover generated file names only;
// never guess quality/ownership from their presence and never delete a partial
// or unrecognized Draft while scanning. Native and review files stay in the
// existing Draft/NativeRate directories, not a parallel metadata store.
#[derive(Debug, Clone, Serialize)]
pub struct OrphanedGuidedDraft {
    pub line_id: u32,
    pub pair_present: bool,
    pub note: String,
}

fn generated_draft_id(filename: &str) -> Option<(u32, bool)> {
    let (stem, staged) = if let Some(stem) = filename.strip_suffix(".wav.tmp") {
        (stem, true)
    } else {
        (filename.strip_suffix(".wav")?, false)
    };
    let suffix = stem.strip_prefix("take_")?;
    if suffix.len() != 4 || !suffix.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((suffix.parse::<u32>().ok()?, staged))
}

pub(super) fn find_orphaned_guided_drafts(
    draft_root: &Path,
    valid_line: impl Fn(u32) -> bool,
) -> Result<Vec<OrphanedGuidedDraft>, String> {
    let mut found = BTreeMap::<u32, (bool, bool, bool)>::new();
    for native in [false, true] {
        let dir = if native { draft_root.join("NativeRate") } else { draft_root.to_path_buf() };
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err("Could not inspect My Voice Draft files safely.".to_string()),
        };
        for entry in entries {
            let entry = entry.map_err(|_| "Could not inspect a My Voice Draft entry.".to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some((id, staged)) = generated_draft_id(&name) else { continue };
            if !valid_line(id) { continue; }
            let item = found.entry(id).or_default();
            // Do not advertise symlinks or directories as a recoverable pair.
            let regular = entry.file_type().map(|value| value.is_file()).unwrap_or(false);
            if staged || !regular {
                item.2 = true;
            } else if native {
                item.1 = true;
            } else {
                item.0 = true;
            }
        }
    }
    Ok(found.into_iter().map(|(line_id, (review, native, unsafe_entry))| {
        let pair_present = review && native && !unsafe_entry;
        OrphanedGuidedDraft {
            line_id,
            pair_present,
            note: if pair_present {
                "A saved Draft pair was found. Recover it for review only after verifying its audio."
            } else {
                "A partial or unverified Draft remains on disk. Files were preserved; do not record over them."
            }.to_string(),
        }
    }).collect())
}

// Strictly read only the canonical 44-byte PCM16 mono RIFF produced by this
// app's WAV writer. Malformed, overly large, symlinked or incomplete files
// never become PendingDraft and remain intact for recovery investigation.
const MAX_RECOVERED_NATIVE_RATE_HZ: u32 = 192_000;
const MAX_RECOVERED_DURATION_SECONDS: u64 = 60;
const MAX_RECOVERED_WAV_BYTES: u64 =
    44 + MAX_RECOVERED_NATIVE_RATE_HZ as u64 * MAX_RECOVERED_DURATION_SECONDS * 2;

// The recovered review is bound to the exact bytes inspected as canonical
// audio, not merely filenames that may later be replaced on disk.
#[derive(Clone)]
pub(super) struct RecoveredDraftIdentity {
    review_sha256: [u8; 32],
    native_sha256: [u8; 32],
}

struct CheckedRecoveryWav {
    rate: u32,
    samples: Vec<f32>,
    sha256: [u8; 32],
}

fn checked_recovery_pcm16(path: &Path, canonical: bool) -> Result<CheckedRecoveryWav, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "voice_lab:recovery:draft_missing_or_inaccessible".to_string())?;
    if !metadata.file_type().is_file() || metadata.len() < 46
        || metadata.len() > MAX_RECOVERED_WAV_BYTES
    {
        return Err("voice_lab:recovery:untrusted_draft_file".to_string());
    }
    let bytes = fs::read(path).map_err(|_| "voice_lab:recovery:draft_read_failed".to_string())?;
    if bytes.len() as u64 != metadata.len()
        || &bytes[0..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || &bytes[12..16] != b"fmt "
        || u32::from_le_bytes(bytes[16..20].try_into().unwrap()) != 16
        || u16::from_le_bytes(bytes[20..22].try_into().unwrap()) != 1
        || u16::from_le_bytes(bytes[22..24].try_into().unwrap()) != 1
        || &bytes[36..40] != b"data"
        || u16::from_le_bytes(bytes[32..34].try_into().unwrap()) != 2
        || u16::from_le_bytes(bytes[34..36].try_into().unwrap()) != 16
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize != bytes.len() - 8
        || u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize != bytes.len() - 44
        || (bytes.len() - 44) % 2 != 0
    {
        return Err("voice_lab:recovery:not_canonical_pcm16_wav".to_string());
    }
    let rate = u32::from_le_bytes(bytes[24..28].try_into().unwrap());
    if !(8_000..=MAX_RECOVERED_NATIVE_RATE_HZ).contains(&rate)
        || (canonical && rate != GUIDED_TAKE_SAMPLE_RATE_HZ)
        || u32::from_le_bytes(bytes[28..32].try_into().unwrap()) != rate * 2
        || (bytes.len() - 44) as u64 > u64::from(rate) * MAX_RECOVERED_DURATION_SECONDS * 2
    {
        return Err("voice_lab:recovery:invalid_audio_format_or_duration".to_string());
    }
    // Hash the very same validated bytes. Re-reading afterward would permit
    // a file swap between integrity validation and its retained identity.
    let digest = Sha256::digest(&bytes);
    let mut sha256 = [0_u8; 32];
    sha256.copy_from_slice(&digest);
    let samples = bytes[44..].chunks_exact(2)
        .map(|pcm| f32::from(i16::from_le_bytes([pcm[0], pcm[1]])) / f32::from(i16::MAX))
        .collect();
    Ok(CheckedRecoveryWav { rate, samples, sha256 })
}

fn no_unfinished_recovery_stage(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path.with_extension("wav.tmp")) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        _ => Err("voice_lab:recovery:unfinished_staging_present".to_string()),
    }
}

// Inspect both existing files again under the PendingDraft owner immediately
// before replay, accept or discard. In particular, a verified filename must
// not authorize deleting or accepting a later unrelated file at that path.
// Discard permits an already-missing half following an earlier partial discard,
// but still rejects any remaining file whose bytes have changed.
fn verify_recovered_file(
    path: &Path,
    expected_sha256: &[u8; 32],
    missing_permitted: bool,
) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound && missing_permitted => {
            return Ok(());
        }
        Err(_) => return Err("voice_lab:recovery:owned_file_missing".to_string()),
    };
    if !metadata.file_type().is_file()
        || metadata.len() < 46
        || metadata.len() > MAX_RECOVERED_WAV_BYTES
    {
        return Err("voice_lab:recovery:owned_file_not_regular".to_string());
    }
    let bytes = fs::read(path)
        .map_err(|_| "voice_lab:recovery:owned_file_read_failed".to_string())?;
    if bytes.len() as u64 != metadata.len() {
        return Err("voice_lab:recovery:owned_file_changed".to_string());
    }
    let digest = Sha256::digest(&bytes);
    let mut sha256 = [0_u8; 32];
    sha256.copy_from_slice(&digest);
    if &sha256 != expected_sha256 {
        return Err("voice_lab:recovery:owned_file_changed".to_string());
    }
    Ok(())
}

pub(super) fn verify_recovered_draft_identity(
    review_path: &Path,
    native_path: &Path,
    identity: &RecoveredDraftIdentity,
    discard: bool,
) -> Result<(), String> {
    no_unfinished_recovery_stage(review_path)?;
    no_unfinished_recovery_stage(native_path)?;
    verify_recovered_file(review_path, &identity.review_sha256, discard)?;
    verify_recovered_file(native_path, &identity.native_sha256, discard)
}

// The same validated bytes must reach the replay caller. A separate digest
// check before fs::read alone cannot bind the later audio buffer to its owner.
pub(super) fn recovered_review_matches_bytes(
    identity: &RecoveredDraftIdentity,
    audio: &[u8],
) -> bool {
    let digest = Sha256::digest(audio);
    let mut sha256 = [0_u8; 32];
    sha256.copy_from_slice(&digest);
    sha256 == identity.review_sha256
}

pub(super) fn recover_guided_draft_pair(
    review_path: &Path,
    native_path: &Path,
    line_id: u32,
) -> Result<(GuidedTakeReview, RecoveredDraftIdentity), String> {
    if review_path == native_path {
        return Err("voice_lab:recovery:identical_draft_paths".to_string());
    }
    for path in [review_path, native_path] {
        // A staged WAV can still be the only recoverable bytes. Treat it as
        // an unknown owner, never remove or silently promote it.
        no_unfinished_recovery_stage(path)?;
    }
    let canonical = checked_recovery_pcm16(review_path, true)?;
    let native = checked_recovery_pcm16(native_path, false)?;
    let expected_frames = native.samples.len() as u128
        * u128::from(GUIDED_TAKE_SAMPLE_RATE_HZ) / u128::from(native.rate);
    if canonical.samples.len() as u128 != expected_frames {
        return Err("voice_lab:recovery:draft_duration_mismatch".to_string());
    }
    let converted = resample_guided_native_for_review(&native.samples, native.rate)
        .map_err(|_| "voice_lab:recovery:native_audio_resample_failed".to_string())?;
    if converted.len() != canonical.samples.len() {
        return Err("voice_lab:recovery:native_audio_length_mismatch".to_string());
    }
    let mut error_energy = 0_f64;
    let mut signal_energy = 0_f64;
    for (actual, expected) in converted.iter().zip(&canonical.samples) {
        error_energy += f64::from(actual - expected).powi(2);
        signal_energy += f64::from(*expected).powi(2);
    }
    let count = canonical.samples.len() as f64;
    let error_rms = (error_energy / count).sqrt();
    let signal_rms = (signal_energy / count).sqrt();
    // Native-rate and resampled 32 kHz WAVs were both PCM16 quantized.
    // Accept small quantization/resampling differences, not unrelated audio
    // that merely has the same duration.
    if error_rms > 0.004 + signal_rms * 0.15 {
        return Err("voice_lab:recovery:draft_audio_mismatch".to_string());
    }
    let mut review = review_guided_samples(line_id, &canonical.samples);
    if review.quality_blocker.is_empty() {
        review.review_advice.push("voice_lab:review:recovered_from_disk".to_string());
    }
    Ok((review, RecoveredDraftIdentity {
        review_sha256: canonical.sha256,
        native_sha256: native.sha256,
    }))
}

// A fresh guided Stop owns the Draft paths only when neither existed before
// writing. An orphaned review from a prior interrupted call may be the only
// recoverable audio: never overwrite it or delete it as error cleanup.
fn require_unowned_draft_absent(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err("VoiceLab found existing Draft audio without an active review owner. The existing recording was preserved instead of being overwritten.".to_string()),
        Err(_) => Err("VoiceLab could not verify ownership of existing Draft audio. No recording was overwritten.".to_string()),
    }
}

// No new microphone session may be claimed while a generated Draft or its
// unpublished stage remains on disk. Check the two canonical paths and both
// staging paths before recording (and again when writing), never after losing
// a freshly captured recording to the collision.
pub(super) fn ensure_new_guided_draft_paths_are_free(
    review_path: &Path,
    native_path: &Path,
) -> Result<(), String> {
    if review_path == native_path {
        return Err("VoiceLab review and native Draft paths cannot be identical.".to_string());
    }
    for path in [review_path, native_path] {
        require_unowned_draft_absent(path)?;
        require_unowned_draft_absent(&path.with_extension("wav.tmp"))?;
    }
    Ok(())
}

// This is the only publish path for a fresh unaccepted guided recording.
// The app-level PendingDraft mutex spans the pair write and registration.
// The writer callback is injectable solely to exercise individual I/O failure
// ordering in isolated Rust fixtures; production uses canonical PCM16 writer.
pub(super) fn write_guided_review_draft_pair(
    review_path: &Path,
    native_path: &Path,
    write_native: impl FnOnce(&Path) -> io::Result<()>,
    write_review: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<(), String> {
    ensure_new_guided_draft_paths_are_free(review_path, native_path)?;

    // First write succeeds => this invocation, not a previous recording,
    // created native_path. First-write failures must not delete either path.
    write_native(native_path)
        .map_err(|error| format!("VoiceLab could not save the native-rate draft: {error}. Existing Draft audio was not removed."))?;

    if let Err(error) = write_review(review_path) {
        // The canonical WAV writer either publishes the complete review WAV
        // or returns error without replacing any valid destination. Delete
        // only the native draft whose successful write we own here.
        match fs::remove_file(native_path) {
            Ok(()) => Err(format!("VoiceLab could not save the review draft: {error}. This attempt's native-rate draft was rolled back.")),
            Err(cleanup) if cleanup.kind() == io::ErrorKind::NotFound => {
                Err(format!("VoiceLab could not save the review draft: {error}. No native-rate draft remains."))
            }
            Err(cleanup) => Err(format!(
                "VoiceLab could not save the review draft: {error}. Native-rate rollback could not be verified: {cleanup}. Existing files require recovery before another attempt."
            )),
        }
    } else {
        Ok(())
    }
}

pub(super) fn discard_review_take(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path)
        .map_err(|error| format!("VoiceLab could not remove the review take safely: {error}"))
}

// Accept must treat an existing *.wav.previous as a recovery obligation,
// not a disposable stale artifact. That backup may be the last valid copy
// after an interrupted previous Save; the caller has already archived any
// verified old take, but this transaction must not guess about its identity.
fn ensure_accept_backup_absent(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err("VoiceLab found an earlier accepted-take backup. The existing and new recordings were preserved; resolve the interrupted Save before trying again.".to_string()),
        Err(error) => Err(format!(
            "VoiceLab could not verify accepted-take backup state: {error}. No recording was replaced."
        )),
    }
}

fn accepted_regular_file_present(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(true),
        Ok(_) => Err("VoiceLab found an accepted-take destination that is not a regular file. Nothing was replaced.".to_string()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "VoiceLab could not verify accepted-take destination: {error}. No recording was replaced."
        )),
    }
}

// Never restore over an occupied destination, even on operating systems
// where fs::rename would silently replace the target file. Retain backup
// ownership and report an incomplete rollback instead of destroying newer
// bytes by guessing which file should win.
fn restore_previous_to_vacant_target(previous: &Path, target: &Path) -> Result<(), String> {
    if accepted_regular_file_present(target)? {
        return Err("VoiceLab rollback destination is occupied. Previous backup was preserved.".to_string());
    }
    fs::rename(previous, target)
        .map_err(|error| format!("VoiceLab could not restore the previous accepted take: {error}. Its backup was preserved."))
}

// If the canonical WAV Save failed, first put the just-promoted native WAV
// back into Draft. Restoring the old native backup BEFORE that succeeds can
// overwrite/destroy the new native recording on POSIX and fail on Windows.
// A newly occupied native Draft path blocks rollback instead of being clobbered.
fn undo_native_promotion_after_canonical_failure(
    native_draft: &Path,
    native_target: &Path,
    previous: &Path,
    had_previous: bool,
) -> Result<(), String> {
    if accepted_regular_file_present(native_draft)? {
        return Err("VoiceLab native-rate Draft was occupied during rollback. New accepted audio and the previous backup were preserved for recovery.".to_string());
    }
    fs::rename(native_target, native_draft).map_err(|error| format!(
        "VoiceLab could not return the newly promoted native-rate take to Draft: {error}. Previous backup was NOT restored over the new take."
    ))?;
    if had_previous {
        restore_previous_to_vacant_target(previous, native_target)?;
    }
    Ok(())
}

pub(super) fn accept_review_take(draft: &Path, target: &Path) -> Result<(), String> {
    if !draft.is_file() {
        return Err("VoiceLab cannot accept a missing or non-file review Draft.".to_string());
    }
    let previous = target.with_extension("wav.previous");
    ensure_accept_backup_absent(&previous)?;
    let had_previous = accepted_regular_file_present(target)?;
    if had_previous {
        fs::rename(target, &previous).map_err(|error| {
            format!("VoiceLab could not preserve the previous accepted take: {error}")
        })?;
    }
    if let Err(error) = fs::rename(draft, target) {
        if had_previous {
            restore_previous_to_vacant_target(&previous, target).map_err(|rollback_error| {
                format!(
                    "VoiceLab could not accept this take: {error}; previous accepted take rollback also failed: {rollback_error}"
                )
            })?;
        }
        return Err(format!("VoiceLab could not accept this take: {error}"));
    }
    if had_previous {
        // Success already has a verified immutable archive of the old take.
        // A failed backup unlink is not a reason to undo a successful Save;
        // the remaining backup blocks future replacements until reconciled.
        let _ = fs::remove_file(previous);
    }
    Ok(())
}

/// Preserve a device-native-rate mono sidecar alongside the canonical 32 kHz
/// take. GPT-SoVITS continues to consume ONLY the canonical take. The caller
/// archives both old and new sources before permitting these moves; interrupted
/// filesystem state is never interpreted as permission to delete a backup.
pub(super) fn accept_review_take_with_native_rate(
    draft: &Path,
    target: &Path,
    native_draft: &Path,
    native_target: &Path,
) -> Result<(), String> {
    if !draft.is_file() || !native_draft.is_file() {
        return Err("VoiceLab cannot accept the take without both review and native-rate drafts.".to_string());
    }
    let canonical_previous = target.with_extension("wav.previous");
    let previous = native_target.with_extension("wav.previous");

    // Full-pair preflight BEFORE the first rename: a stale canonical backup
    // must not make us stage a native replacement only to fail later.
    ensure_accept_backup_absent(&canonical_previous)?;
    ensure_accept_backup_absent(&previous)?;
    let _canonical_exists = accepted_regular_file_present(target)?;
    let had_previous = accepted_regular_file_present(native_target)?;

    let parent = native_target.parent().ok_or("VoiceLab native-rate take directory is invalid.")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("VoiceLab could not prepare native-rate take storage: {error}"))?;
    if had_previous {
        fs::rename(native_target, &previous).map_err(|error| {
            format!("VoiceLab could not preserve the previous native-rate take: {error}")
        })?;
    }
    if let Err(error) = fs::rename(native_draft, native_target) {
        if had_previous {
            restore_previous_to_vacant_target(&previous, native_target)
                .map_err(|restore| format!(
                    "VoiceLab could not save the native-rate take: {error}; previous native-rate rollback failed: {restore}"
                ))?;
        }
        return Err(format!("VoiceLab could not save the native-rate take: {error}"));
    }

    if let Err(error) = accept_review_take(draft, target) {
        if let Err(rollback) = undo_native_promotion_after_canonical_failure(
            native_draft, native_target, &previous, had_previous,
        ) {
            return Err(format!(
                "{error}; native-rate rollback incomplete: {rollback}. Do not retry Save until the preserved files are reconciled."
            ));
        }
        return Err(error);
    }
    if had_previous {
        // Native output was accepted after the canonical WAV was promoted.
        // Failed backup removal leaves an explicit recovery obligation for
        // the next Save, never permission to erase it opportunistically.
        let _ = fs::remove_file(previous);
    }
    Ok(())
}

#[cfg(test)]
mod native_rate_take_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static RUN: AtomicU64 = AtomicU64::new(0);

    fn fixture(label: &str) -> std::path::PathBuf {
        let token = RUN.fetch_add(1, Ordering::Relaxed);
        let folder = std::env::temp_dir().join(format!(
            "translateit_native_take_{}_{}_{}",
            label,
            std::process::id(),
            token
        ));
        fs::create_dir_all(&folder).expect("create fixture");
        folder
    }


    #[test]
    fn disk_recovery_detects_both_complete_and_partial_draft_lines() {
        let root = fixture("recover_discovery");
        let draft = root.join("Draft");
        fs::create_dir_all(draft.join("NativeRate")).unwrap();
        fs::write(draft.join("take_0007.wav"), b"review").unwrap();
        fs::write(draft.join("NativeRate/take_0007.wav"), b"native").unwrap();
        fs::write(draft.join("take_0008.wav.tmp"), b"unfinished stage").unwrap();
        fs::write(draft.join("NativeRate/take_0008.wav"), b"native only").unwrap();
        fs::write(draft.join("take_untrusted.wav"), b"unrecognized").unwrap();
        let files = find_orphaned_guided_drafts(&draft, |id| id == 7 || id == 8).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].line_id, 7);
        assert!(files[0].pair_present);
        assert_eq!(files[1].line_id, 8);
        assert!(!files[1].pair_present);
        assert!(draft.join("take_0008.wav.tmp").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn completed_native_and_review_pcm16_can_be_reopened_for_review_only() {
        use crate::engine::audio::guided_take::{
            resample_guided_native_for_review, GUIDED_TAKE_SAMPLE_RATE_HZ,
        };
        use crate::engine::audio::live_segment_writer::write_pcm16_wav;
        let root = fixture("recover_pair");
        let draft = root.join("Draft");
        let native_path = draft.join("NativeRate/take_0017.wav");
        let review_path = draft.join("take_0017.wav");
        let native_rate = 48_000;
        let native_samples: Vec<f32> = (0..48_000)
            .map(|i| ((i as f32 * 0.023).sin() * 0.14)).collect();
        let resampled = resample_guided_native_for_review(&native_samples, native_rate).unwrap();
        write_pcm16_wav(&native_path, native_rate, 1, &native_samples).unwrap();
        write_pcm16_wav(&review_path, GUIDED_TAKE_SAMPLE_RATE_HZ, 1, &resampled).unwrap();
        let (restored, identity) = recover_guided_draft_pair(&review_path, &native_path, 17)
            .expect("native and review WAVs came from the same source");
        verify_recovered_draft_identity(&review_path, &native_path, &identity, false)
            .expect("unchanged pair matches its recovered identity");
        assert_eq!(restored.line_id, 17);
        assert_eq!(restored.duration_ms, 1_000);
        assert!(restored.quality_blocker.is_empty());
        assert!(restored.review_advice.iter().any(|code| code == "voice_lab:review:recovered_from_disk"));
        assert!(native_path.exists());
        assert!(review_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn mixed_up_or_incomplete_wav_pair_never_becomes_a_review() {
        use crate::engine::audio::live_segment_writer::write_pcm16_wav;
        let root = fixture("recover_mismatch");
        let draft = root.join("Draft");
        let native = draft.join("NativeRate/take_0018.wav");
        let review = draft.join("take_0018.wav");
        let rate = 32_000;
        let a: Vec<f32> = (0..32_000).map(|i| (i as f32 * 0.018).sin() * 0.25).collect();
        let b: Vec<f32> = (0..32_000).map(|i| (i as f32 * 0.032).sin() * 0.25).collect();
        write_pcm16_wav(&native, rate, 1, &a).unwrap();
        write_pcm16_wav(&review, rate, 1, &b).unwrap();
        assert!(recover_guided_draft_pair(&review, &native, 18).is_err(),
            "matching duration must not be mistaken for matching audio");
        let unfinished = review.with_extension("wav.tmp");
        fs::write(&unfinished, b"unpublished").unwrap();
        assert!(recover_guided_draft_pair(&review, &native, 18).is_err(),
            "an unverified writer stage blocks recovery even if both final WAVs exist");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovered_pair_identity_rejects_swapped_file_before_accept_or_replay() {
        use crate::engine::audio::live_segment_writer::write_pcm16_wav;
        let root = fixture("recover_swap");
        let draft = root.join("Draft");
        let native = draft.join("NativeRate/take_0020.wav");
        let review = draft.join("take_0020.wav");
        let first: Vec<f32> = (0..32_000).map(|i| (i as f32 * 0.018).sin() * 0.15).collect();
        write_pcm16_wav(&native, 32_000, 1, &first).unwrap();
        write_pcm16_wav(&review, 32_000, 1, &first).unwrap();
        let (_, identity) = recover_guided_draft_pair(&review, &native, 20).unwrap();
        let original_bytes = fs::read(&review).unwrap();
        assert!(recovered_review_matches_bytes(&identity, &original_bytes));
        let changed: Vec<f32> = (0..32_000).map(|i| (i as f32 * 0.035).sin() * 0.15).collect();
        write_pcm16_wav(&review, 32_000, 1, &changed).unwrap();
        assert!(!recovered_review_matches_bytes(&identity, &fs::read(&review).unwrap()),
            "a changed replay payload must not be sent as previously verified audio");
        assert!(verify_recovered_draft_identity(&review, &native, &identity, false).is_err());
        assert!(verify_recovered_draft_identity(&review, &native, &identity, true).is_err(),
            "Retry may never remove a replaced WAV under an obsolete review owner");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partial_retry_accepts_missing_half_but_rejects_replaced_survivor() {
        use crate::engine::audio::live_segment_writer::write_pcm16_wav;
        let root = fixture("recover_partial_discard");
        let review = root.join("take_0021.wav");
        let native = root.join("NativeRate/take_0021.wav");
        let waveform: Vec<f32> = (0..32_000).map(|i| (i as f32 * 0.023).sin() * 0.15).collect();
        write_pcm16_wav(&native, 32_000, 1, &waveform).unwrap();
        write_pcm16_wav(&review, 32_000, 1, &waveform).unwrap();
        let (_, identity) = recover_guided_draft_pair(&review, &native, 21).unwrap();
        fs::remove_file(&native).unwrap();
        assert!(verify_recovered_draft_identity(&review, &native, &identity, true).is_ok(),
            "an earlier partial discard must remain retryable");
        assert!(verify_recovered_draft_identity(&review, &native, &identity, false).is_err(),
            "replay and accept require the complete original pair");
        fs::write(&review, b"replacement audio").unwrap();
        assert!(verify_recovered_draft_identity(&review, &native, &identity, true).is_err(),
            "Retry may not erase an unrelated replacement of the surviving file");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_or_truncated_pcm16_recovery_never_modifies_disk() {
        let root = fixture("recover_invalid");
        let review = root.join("take_0019.wav");
        let native = root.join("NativeRate/take_0019.wav");
        fs::create_dir_all(native.parent().unwrap()).unwrap();
        fs::write(&review, b"RIFFtruncated").unwrap();
        fs::write(&native, b"unverified audio").unwrap();
        assert!(recover_guided_draft_pair(&review, &native, 19).is_err());
        assert_eq!(fs::read(&review).unwrap(), b"RIFFtruncated");
        assert_eq!(fs::read(&native).unwrap(), b"unverified audio");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unfinished_pcm16_stage_blocks_a_new_recording_before_microphone_capture() {
        let root = fixture("recover_unfinished_stage");
        let review = root.join("take.wav");
        let native = root.join("NativeRate").join("take.wav");
        let stage = native.with_extension("wav.tmp");
        fs::create_dir_all(stage.parent().unwrap()).unwrap();
        fs::write(&stage, b"unfinished WAV staging").unwrap();
        assert!(ensure_new_guided_draft_paths_are_free(&review, &native).is_err());
        assert!(write_guided_review_draft_pair(
            &review, &native,
            |_| panic!("do not start a new writer over an existing WAV stage"),
            |_| panic!("review writer cannot run"),
        ).is_err());
        assert_eq!(fs::read(&stage).unwrap(), b"unfinished WAV staging");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fresh_review_pair_publishes_both_owned_wavs() {
        let root = fixture("draft_pair");
        let review = root.join("Draft").join("take.wav");
        let native = root.join("Draft").join("NativeRate").join("take.wav");
        write_guided_review_draft_pair(
            &review, &native,
            |path| { fs::create_dir_all(path.parent().unwrap())?; fs::write(path, b"native") },
            |path| { fs::create_dir_all(path.parent().unwrap())?; fs::write(path, b"review") },
        ).expect("complete new draft pair");
        assert_eq!(fs::read(&native).unwrap(), b"native");
        assert_eq!(fs::read(&review).unwrap(), b"review");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_draft_is_preserved_before_any_write() {
        let root = fixture("prior_draft");
        let review = root.join("take.wav");
        let native = root.join("NativeRate").join("take.wav");
        fs::write(&review, b"previous valid review").unwrap();
        let mut attempted = false;
        let error = write_guided_review_draft_pair(
            &review, &native,
            |_| { attempted = true; Err(io::Error::from(io::ErrorKind::PermissionDenied)) },
            |_| panic!("review writer cannot run when old Draft audio already exists"),
        ).unwrap_err();
        assert!(error.contains("preserved"));
        assert!(!attempted, "pre-existing Draft ownership must reject before either write");
        assert_eq!(fs::read(&review).unwrap(), b"previous valid review");
        assert!(!native.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_native_rate_draft_is_not_overwritten_or_removed() {
        let root = fixture("existing_native");
        let review = root.join("take.wav");
        let native = root.join("NativeRate").join("take.wav");
        fs::create_dir_all(native.parent().unwrap()).unwrap();
        fs::write(&native, b"previous valid native rate").unwrap();
        let error = write_guided_review_draft_pair(
            &review, &native,
            |_| panic!("old native rate prevents new write"),
            |_| panic!("review write must not run with unowned native draft"),
        ).unwrap_err();
        assert!(error.contains("preserved"));
        assert_eq!(fs::read(&native).unwrap(), b"previous valid native rate");
        assert!(!review.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_first_write_does_not_delete_or_publish_either_draft() {
        let root = fixture("first_native_failure");
        let review = root.join("take.wav");
        let native = root.join("NativeRate").join("take.wav");
        let error = write_guided_review_draft_pair(
            &review, &native,
            |_| Err(io::Error::from(io::ErrorKind::PermissionDenied)),
            |_| panic!("review cannot be saved when native capture failed"),
        ).unwrap_err();
        assert!(error.contains("native-rate draft"));
        assert!(!review.exists());
        assert!(!native.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_second_draft_write_rolls_back_only_new_native_file() {
        let root = fixture("failed_review");
        let review = root.join("Draft").join("take.wav");
        let native = root.join("Draft").join("NativeRate").join("take.wav");
        let accepted = root.join("Accepted").join("take.wav");
        fs::create_dir_all(accepted.parent().unwrap()).unwrap();
        fs::write(&accepted, b"previous accepted source").unwrap();
        let error = write_guided_review_draft_pair(
            &review, &native,
            |path| { fs::create_dir_all(path.parent().unwrap())?; fs::write(path, b"owned native") },
            |_| Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ).unwrap_err();
        assert!(error.contains("rolled back"));
        assert!(!native.exists(), "only the new native artifact may be removed");
        assert!(!review.exists(), "failed second write must not claim review audio");
        assert_eq!(fs::read(&accepted).unwrap(), b"previous accepted source");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_native_rollback_is_reported_and_file_is_preserved() {
        let root = fixture("rollback_unverified");
        let review = root.join("draft.wav");
        let native = root.join("NativeRate").join("draft.wav");
        let error = write_guided_review_draft_pair(
            &review, &native,
            |path| fs::create_dir_all(path),
            |_| Err(io::Error::from(io::ErrorKind::PermissionDenied)),
        ).unwrap_err();
        assert!(error.contains("rollback could not be verified"));
        assert!(native.is_dir(), "unverified cleanup must preserve the blocking owner");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canonical_previous_backup_never_deleted_by_a_new_accept() {
        let root = fixture("canonical_backup_preserved");
        let draft = root.join("draft.wav");
        let target = root.join("accepted.wav");
        let previous = target.with_extension("wav.previous");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("accepted.wav");
        fs::create_dir_all(native_target.parent().unwrap()).unwrap();
        fs::write(&draft, b"new review").unwrap();
        fs::write(&target, b"accepted canonical").unwrap();
        fs::write(&previous, b"critical older backup").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&native_target, b"accepted native").unwrap();

        let blocked = accept_review_take_with_native_rate(
            &draft, &target, &native_draft, &native_target,
        ).unwrap_err();
        assert!(blocked.contains("earlier accepted-take backup"));
        assert_eq!(fs::read(&previous).unwrap(), b"critical older backup");
        assert_eq!(fs::read(&target).unwrap(), b"accepted canonical");
        assert_eq!(fs::read(&native_target).unwrap(), b"accepted native");
        assert_eq!(fs::read(&draft).unwrap(), b"new review");
        assert_eq!(fs::read(&native_draft).unwrap(), b"new native");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn orphan_native_previous_backup_is_not_automatically_restored_or_replaced() {
        let root = fixture("native_backup_preserved");
        let draft = root.join("draft.wav");
        let target = root.join("accepted.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("accepted.wav");
        let previous = native_target.with_extension("wav.previous");
        fs::create_dir_all(native_target.parent().unwrap()).unwrap();
        fs::write(&draft, b"new review").unwrap();
        fs::write(&target, b"accepted canonical").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&previous, b"orphan previous native").unwrap();
        let blocked = accept_review_take_with_native_rate(
            &draft, &target, &native_draft, &native_target,
        ).unwrap_err();
        assert!(blocked.contains("earlier accepted-take backup"));
        assert_eq!(fs::read(&previous).unwrap(), b"orphan previous native");
        assert!(!native_target.exists(), "recovery requires a separate explicit decision");
        assert_eq!(fs::read(&target).unwrap(), b"accepted canonical");
        assert!(native_draft.exists() && draft.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_never_restores_previous_over_an_occupied_destination() {
        let root = fixture("no_clobber_restore");
        let target = root.join("accepted.wav");
        let previous = target.with_extension("wav.previous");
        fs::write(&target, b"new accepted content").unwrap();
        fs::write(&previous, b"older accepted content").unwrap();
        assert!(restore_previous_to_vacant_target(&previous, &target).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"new accepted content");
        assert_eq!(fs::read(&previous).unwrap(), b"older accepted content");
        fs::remove_file(&target).unwrap();
        restore_previous_to_vacant_target(&previous, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"older accepted content");
        assert!(!previous.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_rollback_restores_draft_before_previous_accepted_audio() {
        let root = fixture("ordered_native_rollback");
        let native_draft = root.join("NativeRate").join("draft.wav");
        let native_target = root.join("NativeRate").join("accepted.wav");
        let previous = native_target.with_extension("wav.previous");
        fs::create_dir_all(native_target.parent().unwrap()).unwrap();
        fs::write(&native_target, b"new native staged as accepted").unwrap();
        fs::write(&previous, b"old native accepted").unwrap();

        undo_native_promotion_after_canonical_failure(
            &native_draft, &native_target, &previous, true,
        ).unwrap();
        assert_eq!(fs::read(&native_draft).unwrap(), b"new native staged as accepted");
        assert_eq!(fs::read(&native_target).unwrap(), b"old native accepted");
        assert!(!previous.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_native_draft_rollback_keeps_new_accepted_and_old_backup_intact() {
        let root = fixture("blocked_native_rollback");
        let native_draft = root.join("NativeRate").join("draft.wav");
        let native_target = root.join("NativeRate").join("accepted.wav");
        let previous = native_target.with_extension("wav.previous");
        fs::create_dir_all(native_target.parent().unwrap()).unwrap();
        fs::write(&native_draft, b"new content unexpectedly occupied Draft").unwrap();
        fs::write(&native_target, b"new native accepted").unwrap();
        fs::write(&previous, b"old native accepted").unwrap();
        let error = undo_native_promotion_after_canonical_failure(
            &native_draft, &native_target, &previous, true,
        ).unwrap_err();
        assert!(error.contains("occupied"));
        assert_eq!(fs::read(&native_draft).unwrap(), b"new content unexpectedly occupied Draft");
        assert_eq!(fs::read(&native_target).unwrap(), b"new native accepted");
        assert_eq!(fs::read(&previous).unwrap(), b"old native accepted");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accepts_both_sources_and_replaces_a_previous_pair() {
        let root = fixture("accept");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        fs::create_dir_all(native_target.parent().unwrap()).unwrap();
        fs::write(&native_target, b"old native").unwrap();
        accept_review_take_with_native_rate(&draft, &target, &native_draft, &native_target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new canonical");
        assert_eq!(fs::read(&native_target).unwrap(), b"new native");
        assert!(!native_target.with_extension("wav.previous").exists());
        assert!(!draft.exists());
        assert!(!native_draft.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_native_draft_without_changing_accepted_take() {
        let root = fixture("missing");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        assert!(accept_review_take_with_native_rate(
            &draft, &target, &native_draft, &native_target
        ).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old canonical");
        assert!(draft.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failure_of_canonical_accept_rolls_back_both_native_files() {
        let root = fixture("rollback");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        fs::create_dir_all(native_target.parent().unwrap()).unwrap();
        fs::write(&native_target, b"old native").unwrap();
        // A directory at the canonical backup location must block the whole
        // pair before staging native replacement; preserve both accepted WAVs.
        fs::create_dir(target.with_extension("wav.previous")).unwrap();
        assert!(accept_review_take_with_native_rate(
            &draft, &target, &native_draft, &native_target
        ).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old canonical");
        assert_eq!(fs::read(&native_target).unwrap(), b"old native");
        assert_eq!(fs::read(&native_draft).unwrap(), b"new native");
        assert!(draft.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discarding_unaccepted_pair_does_not_touch_existing_accepted_pair() {
        let root = fixture("discard");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("native_take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        fs::write(&native_target, b"old native").unwrap();
        discard_review_take(&native_draft).unwrap();
        discard_review_take(&draft).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"old canonical");
        assert_eq!(fs::read(&native_target).unwrap(), b"old native");
        fs::remove_dir_all(root).unwrap();
    }
}
