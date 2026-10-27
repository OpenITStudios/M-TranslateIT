//! Immutable, content-addressed copies of accepted My Voice recordings.
//! Existing cache Takes remain the active GPT-SoVITS input until a separately
//! validated migration changes the read authority. Nothing in this module
//! deletes accepted audio, existing archive entries, or incomplete staging.
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::{inspect_canonical_wav, sha256_digest, VoiceLabStoragePaths};

const ARCHIVE_VERSION: u32 = 1;
const MAX_CANONICAL_BYTES: u64 = 4 * 1024 * 1024 + 4096;
const MAX_NATIVE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_RECEIPT_BYTES: u64 = 16 * 1024;
static ARCHIVE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ArchivedTake {
    schema_version: u32,
    line_id: u32,
    exact_text: String,
    canonical_sha256: String,
    native_rate_sha256: Option<String>,
}

fn sha256_hex(bytes: &[u8]) -> Result<String, String> {
    let digest = sha256_digest(bytes).ok_or("voice_lab:archive_sha256_unavailable")?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in digest {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    Ok(result)
}

fn bounded_regular_bytes(path: &Path, cap: u64) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|error| format!("voice_lab:archive_source_metadata_failed:{error}"))?;
    if meta.file_type().is_symlink() || !meta.is_file() || !(44..=cap).contains(&meta.len()) {
        return Err("voice_lab:archive_source_invalid".to_string());
    }
    let mut file = File::open(path)
        .map_err(|error| format!("voice_lab:archive_source_open_failed:{error}"))?;
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("voice_lab:archive_source_read_failed:{error}"))?;
    if bytes.len() as u64 != meta.len() {
        return Err("voice_lab:archive_source_changed".to_string());
    }
    Ok(bytes)
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut target = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|error| format!("voice_lab:archive_stage_create_failed:{error}"))?;
    target.write_all(bytes)
        .map_err(|error| format!("voice_lab:archive_stage_write_failed:{error}"))?;
    target.sync_all()
        .map_err(|error| format!("voice_lab:archive_stage_sync_failed:{error}"))
}

fn verified_archive(dir: &Path, expected: &ArchivedTake) -> Result<(), String> {
    let meta = fs::symlink_metadata(dir)
        .map_err(|error| format!("voice_lab:archive_missing:{error}"))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err("voice_lab:archive_directory_invalid".to_string());
    }
    let receipt = bounded_regular_bytes(&dir.join("take.json"), MAX_RECEIPT_BYTES)?;
    let parsed = serde_json::from_slice::<ArchivedTake>(&receipt)
        .map_err(|_| "voice_lab:archive_receipt_invalid")?;
    if &parsed != expected {
        return Err("voice_lab:archive_receipt_mismatch".to_string());
    }
    let canonical = dir.join("canonical.wav");
    inspect_canonical_wav(&canonical)?;
    let canonical_bytes = bounded_regular_bytes(&canonical, MAX_CANONICAL_BYTES)?;
    if sha256_hex(&canonical_bytes)? != expected.canonical_sha256 {
        return Err("voice_lab:archive_canonical_hash_mismatch".to_string());
    }
    let native_path = dir.join("native-rate.wav");
    match expected.native_rate_sha256.as_ref() {
        Some(hash) => {
            let native_bytes = bounded_regular_bytes(&native_path, MAX_NATIVE_BYTES)?;
            if sha256_hex(&native_bytes)? != *hash {
                return Err("voice_lab:archive_native_hash_mismatch".to_string());
            }
        }
        None => {
            if fs::symlink_metadata(native_path).is_ok() {
                return Err("voice_lab:archive_unexpected_native_audio".to_string());
            }
        }
    }
    Ok(())
}

fn checked_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("voice_lab:archive_directory_create_failed:{error}"))?;
    let meta = fs::symlink_metadata(path)
        .map_err(|error| format!("voice_lab:archive_directory_metadata_failed:{error}"))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err("voice_lab:archive_directory_untrusted".to_string());
    }
    Ok(())
}


struct ArchiveInputs {
    receipt: ArchivedTake,
    receipt_bytes: Vec<u8>,
    id: String,
    canonical_bytes: Vec<u8>,
    native_bytes: Option<Vec<u8>>,
}

fn archive_inputs(
    line_id: u32, exact_text: &str, canonical_path: &Path, native_rate_path: Option<&Path>,
) -> Result<ArchiveInputs, String> {
    if line_id == 0 || exact_text.trim().is_empty() {
        return Err("voice_lab:archive_prompt_invalid".to_string());
    }
    inspect_canonical_wav(canonical_path)?;
    let canonical_bytes = bounded_regular_bytes(canonical_path, MAX_CANONICAL_BYTES)?;
    let native_bytes = native_rate_path.map(|path| bounded_regular_bytes(path, MAX_NATIVE_BYTES)).transpose()?;
    let receipt = ArchivedTake {
        schema_version: ARCHIVE_VERSION, line_id,
        exact_text: exact_text.to_string(),
        canonical_sha256: sha256_hex(&canonical_bytes)?,
        native_rate_sha256: native_bytes.as_ref().map(|v| sha256_hex(v)).transpose()?,
    };
    let receipt_bytes = serde_json::to_vec(&receipt)
        .map_err(|_| "voice_lab:archive_receipt_serialize_failed")?;
    let id = sha256_hex(&receipt_bytes)?;
    Ok(ArchiveInputs { receipt, receipt_bytes, id, canonical_bytes, native_bytes })
}

/// Read-only verification, never generates or repairs the archive on a status read.
// Reuse the single immutable receipt, content-address and verified_archive
// law for both regular dataset checks and read-only interrupted Accept
// inspection. The caller must already hold ARCHIVE_LOCK.
fn verified_archive_id_for_pair_locked(
    storage: &VoiceLabStoragePaths, line_id: u32, exact_text: &str,
    canonical_path: &Path, native_rate_path: Option<&Path>,
) -> Result<Option<String>, String> {
    let prepared = archive_inputs(line_id, exact_text, canonical_path, native_rate_path)?;
    let shard = storage.saved_root.join("Recordings").join(&prepared.id[..2]);
    let complete = shard.join(&prepared.id);
    match fs::symlink_metadata(&complete) {
        Ok(_) => {
            verified_archive(&complete, &prepared.receipt)?;
            Ok(Some(prepared.id))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let pending = shard.join(format!(".pending-{}", prepared.id));
            match fs::symlink_metadata(&pending) {
                Ok(_) => {
                    verified_archive(&pending, &prepared.receipt)
                        .map_err(|_| "voice_lab:archive_pending_recovery_required")?;
                    Ok(None) // a staged receipt is not a published immutable archive
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(error) => Err(format!("voice_lab:archive_pending_metadata_failed:{error}")),
            }
        }
        Err(error) => Err(format!("voice_lab:archive_metadata_failed:{error}")),
    }
}

pub(crate) fn guided_take_is_archived(
    storage: &VoiceLabStoragePaths, line_id: u32, exact_text: &str,
    canonical_path: &Path, native_rate_path: Option<&Path>,
) -> Result<bool, String> {
    let _guard = ARCHIVE_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:archive_lock_unavailable")?;
    Ok(verified_archive_id_for_pair_locked(
        storage, line_id, exact_text, canonical_path, native_rate_path,
    )?.is_some())
}

// Files may be split across Accepted, *.previous and Draft after a crash.
// These are locations, NOT a claim about old/new chronology; only an exact
// verified immutable receipt may bind a canonical and native-rate pair.
const TAKE_LOCATIONS: [&str; 3] = ["accepted", "previous", "draft"];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcceptedTakePairEvidence {
    pub canonical_location: String,
    pub native_location: String,
    pub archive_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcceptedTakeRecoveryInspection {
    pub line_id: u32,
    pub verified_pairs: Vec<AcceptedTakePairEvidence>,
    pub unverified_files: Vec<String>,
    pub previous_backup_present: bool,
    pub verification_incomplete: bool,
    pub needs_attention: bool,
    pub note: String,
}

// Read only known exact application-owned files and explicitly reject
// symlinks, directories and oversized artifacts. Unknown file ownership
// fails closed rather than allowing an apparently complete status.
fn present_accepted_take_candidates<'a>(
    paths: [&'a Path; 3], max_bytes: u64,
) -> Result<Vec<(usize, &'a Path)>, String> {
    let mut result = Vec::new();
    for (slot, path) in paths.into_iter().enumerate() {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if !metadata.file_type().is_file()
                    || metadata.len() < 44 || metadata.len() > max_bytes
                {
                    return Err("voice_lab:accepted_recovery:untrusted_take_file".to_string());
                }
                result.push((slot, path));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("voice_lab:accepted_recovery:take_metadata_unknown".to_string()),
        }
    }
    Ok(result)
}

// Pure classification, independent from IO and Windows hashing for narrow
// regression fixtures. Archive evidence is exact identity, not permission
// to move, delete or replace files in the currently interrupted transaction.
fn classify_accepted_take_candidates(
    line_id: u32,
    canonical: &[(usize, &Path)],
    native: &[(usize, &Path)],
    mut verify: impl FnMut(&Path, Option<&Path>) -> Result<Option<String>, String>,
) -> AcceptedTakeRecoveryInspection {
    let mut verified_pairs = Vec::new();
    let mut covered_canonical = [false; 3];
    let mut covered_native = [false; 3];
    let mut verification_incomplete = false;

    for &(ci, canonical_path) in canonical {
        if native.is_empty() {
            match verify(canonical_path, None) {
                Ok(Some(id)) => {
                    covered_canonical[ci] = true;
                    verified_pairs.push(AcceptedTakePairEvidence {
                        canonical_location: TAKE_LOCATIONS[ci].to_string(),
                        native_location: "none".to_string(),
                        archive_id: id,
                    });
                }
                Ok(None) => {}
                Err(_) => verification_incomplete = true,
            }
        } else {
            for &(ni, native_path) in native {
                match verify(canonical_path, Some(native_path)) {
                    Ok(Some(id)) => {
                        covered_canonical[ci] = true;
                        covered_native[ni] = true;
                        verified_pairs.push(AcceptedTakePairEvidence {
                            canonical_location: TAKE_LOCATIONS[ci].to_string(),
                            native_location: TAKE_LOCATIONS[ni].to_string(),
                            archive_id: id,
                        });
                    }
                    Ok(None) => {}
                    Err(_) => verification_incomplete = true,
                }
            }
        }
    }

    let mut unverified_files = Vec::new();
    for &(index, _) in canonical {
        if !covered_canonical[index] {
            unverified_files.push(format!("{} canonical WAV", TAKE_LOCATIONS[index]));
        }
    }
    for &(index, _) in native {
        if !covered_native[index] {
            unverified_files.push(format!("{} native-rate WAV", TAKE_LOCATIONS[index]));
        }
    }
    let previous_backup_present = canonical.iter().any(|(slot, _)| *slot == 1)
        || native.iter().any(|(slot, _)| *slot == 1);
    let active_pair_verified = verified_pairs.iter().any(|pair| {
        pair.canonical_location == "accepted"
            && (pair.native_location == "accepted" || pair.native_location == "none")
    });
    let needs_attention = previous_backup_present || !unverified_files.is_empty()
        || verification_incomplete || (!canonical.is_empty() && !active_pair_verified);
    let note = if verification_incomplete {
        "Some source/archive combinations could not be verified. No automatic recovery or cleanup was attempted."
    } else if needs_attention {
        "Verified pairs identify exact immutable archive contents, not the correct promotion order. Preserve all files until explicit recovery."
    } else if verified_pairs.is_empty() {
        "No accepted WAV candidate was present. No files were changed."
    } else {
        "The active WAV pairing matches an immutable archive receipt. Inspection only; no files were changed."
    }.to_string();

    AcceptedTakeRecoveryInspection {
        line_id, verified_pairs, unverified_files, previous_backup_present,
        verification_incomplete, needs_attention, note,
    }
}

/// Inspect at most nine exact candidate combinations, on explicit request.
/// Never publish staged receipts as proof; never infer chronology from a
/// .previous suffix. The result never moves, replaces or deletes a WAV.
pub(crate) fn inspect_accepted_take_recovery(
    storage: &VoiceLabStoragePaths,
    line_id: u32,
    exact_text: &str,
    canonical_paths: [&Path; 3],
    native_paths: [&Path; 3],
) -> Result<AcceptedTakeRecoveryInspection, String> {
    if line_id == 0 || exact_text.trim().is_empty() {
        return Err("voice_lab:accepted_recovery:invalid_guided_line".to_string());
    }
    let _guard = ARCHIVE_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:archive_lock_unavailable")?;
    let canonical = present_accepted_take_candidates(canonical_paths, MAX_CANONICAL_BYTES)?;
    let native = present_accepted_take_candidates(native_paths, MAX_NATIVE_BYTES)?;
    Ok(classify_accepted_take_candidates(
        line_id, &canonical, &native,
        |review, sidecar| verified_archive_id_for_pair_locked(
            storage, line_id, exact_text, review, sidecar,
        ),
    ))
}

/// Archive a guided take BEFORE permitting the legacy accepted-take replacement.
///
/// Idempotent on exact line/text/audio bytes. A different recording of the
/// same line produces a different archive identity and never replaces a prior
/// entry. An incomplete archive remains for explicit recovery, not deletion.
pub(crate) fn archive_guided_take(
    storage: &VoiceLabStoragePaths,
    line_id: u32,
    exact_text: &str,
    canonical_path: &Path,
    native_rate_path: Option<&Path>,
) -> Result<String, String> {
    let _guard = ARCHIVE_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:archive_lock_unavailable")?;
    let ArchiveInputs { receipt, receipt_bytes, id, canonical_bytes, native_bytes } =
        archive_inputs(line_id, exact_text, canonical_path, native_rate_path)?;
    let root = storage.saved_root.join("Recordings");
    checked_dir(&root)?;
    let shard: PathBuf = root.join(&id[..2]);
    checked_dir(&shard)?;
    let complete = shard.join(&id);
    if fs::symlink_metadata(&complete).is_ok() {
        verified_archive(&complete, &receipt)?;
        return Ok(id);
    }
    let pending = shard.join(format!(".pending-{id}"));
    if fs::symlink_metadata(&pending).is_ok() {
        // A fully synced staging directory can be promoted after a crash.
        // An incomplete one must not be overwritten or assumed disposable.
        verified_archive(&pending, &receipt)
            .map_err(|_| "voice_lab:archive_pending_recovery_required".to_string())?;
        fs::rename(&pending, &complete)
            .map_err(|error| format!("voice_lab:archive_recovery_commit_failed:{error}"))?;
        verified_archive(&complete, &receipt)?;
        return Ok(id);
    }
    fs::create_dir(&pending)
        .map_err(|error| format!("voice_lab:archive_pending_create_failed:{error}"))?;
    write_new_synced(&pending.join("canonical.wav"), &canonical_bytes)?;
    if let Some(native) = native_bytes.as_ref() {
        write_new_synced(&pending.join("native-rate.wav"), native)?;
    }
    write_new_synced(&pending.join("take.json"), &receipt_bytes)?;
    verified_archive(&pending, &receipt)?;
    fs::rename(&pending, &complete)
        .map_err(|error| format!("voice_lab:archive_commit_failed:{error}"))?;
    verified_archive(&complete, &receipt)?;
    Ok(id)
}


const LIBRARY_PAGE_SIZE: usize = 25;
const MAX_LIBRARY_ENTRIES: usize = 8192;

#[derive(Debug, Clone, Serialize)]
pub struct SavedRecording {
    pub archive_id: String,
    pub line_id: u32,
    pub text: String,
    pub duration_ms: u64,
    pub native_rate_available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SavedRecordingPage {
    pub items: Vec<SavedRecording>,
    pub next_cursor: Option<String>,
    pub attention_entries: usize,
}

fn valid_archive_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn verified_saved_folder(path: &Path) -> Result<(), String> {
    let m = fs::symlink_metadata(path).map_err(|_| "voice_lab:saved_recording_location_missing")?;
    if m.file_type().is_symlink() || !m.is_dir() {
        return Err("voice_lab:saved_recording_location_untrusted".into());
    }
    Ok(())
}

fn saved_recording_location(storage: &VoiceLabStoragePaths, id: &str) -> Result<PathBuf, String> {
    if !valid_archive_id(id) {
        return Err("voice_lab:saved_recording_id_invalid".into());
    }
    let root = storage.saved_root.join("Recordings");
    verified_saved_folder(&root)?;
    let shard = root.join(&id[..2]);
    verified_saved_folder(&shard)?;
    Ok(shard.join(id))
}

fn receipt_for_archive(dir: &Path, id: &str) -> Result<ArchivedTake, String> {
    if !valid_archive_id(id) {
        return Err("voice_lab:saved_recording_id_invalid".into());
    }
    let bytes = bounded_regular_bytes(&dir.join("take.json"), MAX_RECEIPT_BYTES)?;
    let receipt = serde_json::from_slice::<ArchivedTake>(&bytes)
        .map_err(|_| "voice_lab:saved_recording_receipt_invalid")?;
    if receipt.schema_version != ARCHIVE_VERSION || receipt.line_id == 0 ||
        receipt.exact_text.trim().is_empty()
    {
        return Err("voice_lab:saved_recording_receipt_unsupported".into());
    }
    let serialized = serde_json::to_vec(&receipt)
        .map_err(|_| "voice_lab:saved_recording_receipt_invalid")?;
    if sha256_hex(&serialized)? != id {
        return Err("voice_lab:saved_recording_identity_mismatch".into());
    }
    Ok(receipt)
}

/// Paginated, **verified** local library; no delete/mutation operations.
/// Missing/corrupt/pending artifacts are counted as attention, not displayed as
/// saved recordings. The cursor is a deterministic line-id + archive-id pair.
pub(crate) fn saved_recordings_page(
    storage: &VoiceLabStoragePaths,
    after: Option<&str>,
) -> Result<SavedRecordingPage, String> {
    let _guard = ARCHIVE_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:archive_lock_unavailable")?;
    let after = after.unwrap_or("");
    // Validate cursor bytes BEFORE slicing: arbitrary Unicode input must
    // produce an ordinary error rather than panic on a UTF-8 boundary.
    let cursor_bytes = after.as_bytes();
    let valid_cursor = after.is_empty() || (cursor_bytes.len() == 75
        && cursor_bytes[..10].iter().all(u8::is_ascii_digit)
        && cursor_bytes[10] == b'-'
        && cursor_bytes[11..].iter().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b)));
    if !valid_cursor {
        return Err("voice_lab:saved_recording_cursor_invalid".into());
    }
    let root = storage.saved_root.join("Recordings");
    match fs::symlink_metadata(&root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SavedRecordingPage { items: vec![], next_cursor: None, attention_entries: 0 });
        }
        Err(_) => return Err("voice_lab:saved_recording_library_unavailable".into()),
        Ok(_) => verified_saved_folder(&root)?,
    }
    let mut attention = 0usize;
    let mut scanned = 0usize;
    let mut candidates: Vec<(String, String, ArchivedTake, PathBuf)> = Vec::new();
    let shards = fs::read_dir(&root).map_err(|_| "voice_lab:saved_recording_library_unreadable")?;
    for shard in shards {
        let shard = shard.map_err(|_| "voice_lab:saved_recording_library_scan_failed")?;
        let label = shard.file_name().to_string_lossy().to_string();
        if label.len() != 2 || !label.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || verified_saved_folder(&shard.path()).is_err()
        {
            attention += 1;
            continue;
        }
        let entries = fs::read_dir(shard.path())
            .map_err(|_| "voice_lab:saved_recording_shard_unreadable")?;
        for item in entries {
            scanned += 1;
            if scanned > MAX_LIBRARY_ENTRIES {
                return Err("voice_lab:saved_recording_library_scan_limit".into());
            }
            let item = item.map_err(|_| "voice_lab:saved_recording_entry_unreadable")?;
            let id = item.file_name().to_string_lossy().to_string();
            if !valid_archive_id(&id) || &id[..2] != label {
                attention += 1;
                continue;
            }
            let dir = item.path();
            let receipt = match receipt_for_archive(&dir, &id) {
                Ok(receipt) => receipt,
                Err(_) => { attention += 1; continue; }
            };
            let key = format!("{:010}-{id}", receipt.line_id);
            candidates.push((key, id, receipt, dir));
        }
    }
    candidates.sort_by(|a,b| a.0.cmp(&b.0));
    let mut items = Vec::new();
    let mut cursor = None;
    let mut has_more = false;
    for (key, id, receipt, path) in candidates {
        if key.as_str() <= after { continue; }
        if items.len() == LIBRARY_PAGE_SIZE {
            has_more = true;
            break;
        }
        if verified_archive(&path, &receipt).is_err() {
            attention += 1;
            continue;
        }
        let duration_ms = inspect_canonical_wav(&path.join("canonical.wav"))?.duration_ms;
        items.push(SavedRecording {
            archive_id: id,
            line_id: receipt.line_id,
            text: receipt.exact_text,
            duration_ms,
            native_rate_available: receipt.native_rate_sha256.is_some(),
        });
        cursor = Some(key);
    }
    Ok(SavedRecordingPage {
        items,
        next_cursor: if has_more { cursor } else { None },
        attention_entries: attention,
    })
}

/// Verify a frozen dataset's specific prompt-to-audio binding, not just that
/// some other archived take exists under a valid-looking SHA-256 identifier.
pub(super) fn verify_saved_take_binding(
    storage: &VoiceLabStoragePaths,
    archive_id: &str,
    line_id: u32,
    exact_text: &str,
) -> Result<(), String> {
    let _guard = ARCHIVE_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:archive_lock_unavailable")?;
    let dir = saved_recording_location(storage, archive_id)?;
    let receipt = receipt_for_archive(&dir, archive_id)?;
    if receipt.line_id != line_id || receipt.exact_text != exact_text {
        return Err("voice_lab:snapshot_archive_prompt_mismatch".into());
    }
    verified_archive(&dir, &receipt)
}

/// Serves only audio whose bounded content, metadata and canonical archive ID
/// have been verified. No caller-provided filesystem paths are accepted.
pub(crate) fn saved_recording_audio(
    storage: &VoiceLabStoragePaths, archive_id: &str,
) -> Result<Vec<u8>, String> {
    let _guard = ARCHIVE_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:archive_lock_unavailable")?;
    let dir = saved_recording_location(storage, archive_id)?;
    let receipt = receipt_for_archive(&dir, archive_id)?;
    verified_archive(&dir, &receipt)?;
    let audio = bounded_regular_bytes(&dir.join("canonical.wav"), MAX_CANONICAL_BYTES)?;
    if sha256_hex(&audio)? != receipt.canonical_sha256 {
        return Err("voice_lab:saved_recording_audio_changed".into());
    }
    Ok(audio)
}


/// Verified archive material for an explicitly selected training take.
/// All paths are derived from a validated archive ID, never from a UI path.
pub(crate) struct ArchivedTrainingTake {
    pub line_id: u32,
    pub exact_text: String,
    pub canonical_audio: Vec<u8>,
    pub native_audio: Option<Vec<u8>>,
    pub duration_ms: u64,
}

pub(crate) fn archived_take_for_training(
    storage: &VoiceLabStoragePaths,
    archive_id: &str,
) -> Result<ArchivedTrainingTake, String> {
    let _guard = ARCHIVE_LOCK.get_or_init(|| Mutex::new(())).lock()
        .map_err(|_| "voice_lab:archive_lock_unavailable")?;
    let dir = saved_recording_location(storage, archive_id)?;
    let receipt = receipt_for_archive(&dir, archive_id)?;
    verified_archive(&dir, &receipt)?;
    let canonical_audio = bounded_regular_bytes(&dir.join("canonical.wav"), MAX_CANONICAL_BYTES)?;
    let duration_ms = inspect_canonical_wav(&dir.join("canonical.wav"))?.duration_ms;
    let native_audio = if receipt.native_rate_sha256.is_some() {
        Some(bounded_regular_bytes(&dir.join("native-rate.wav"), MAX_NATIVE_BYTES)?)
    } else { None };
    Ok(ArchivedTrainingTake {
        line_id: receipt.line_id,
        exact_text: receipt.exact_text,
        canonical_audio,
        native_audio,
        duration_ms,
    })
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn wav(seed: u8) -> Vec<u8> {
        let data = vec![seed; 64_000];
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36_u32 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&32_000_u32.to_le_bytes());
        wav.extend_from_slice(&64_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        wav
    }


    #[test]
    fn library_pages_verify_versions_and_playback_is_bound_to_archive_id() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit-archive-page-{}-{unique}", std::process::id()));
        let storage = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        fs::create_dir_all(&storage.takes_dir).unwrap();
        let path = storage.takes_dir.join("take_0001.wav");
        let first = wav(1);
        let mut first_id = String::new();
        for seed in 1u8..=28u8 {
            fs::write(&path, wav(seed)).unwrap();
            let id = archive_guided_take(&storage, 1, "Good morning, everyone.", &path, None).unwrap();
            if seed == 1 { first_id = id; }
        }
        let first_page = saved_recordings_page(&storage, None).unwrap();
        assert_eq!(first_page.items.len(), LIBRARY_PAGE_SIZE);
        let second_page = saved_recordings_page(&storage, first_page.next_cursor.as_deref()).unwrap();
        assert_eq!(second_page.items.len(), 3);
        assert!(second_page.next_cursor.is_none());
        assert_eq!(saved_recording_audio(&storage, &first_id).unwrap(), first);
        assert!(saved_recording_audio(&storage, "../wrong").is_err());
        let tampered = storage.saved_root.join("Recordings").join(&first_id[..2]).join(&first_id).join("canonical.wav");
        fs::write(tampered, wav(250)).unwrap();
        assert!(saved_recording_audio(&storage, &first_id).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn immutable_history_keeps_both_retake_versions_and_source_unchanged() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit-archive-{}-{unique}", std::process::id()));
        let storage = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        fs::create_dir_all(&storage.takes_dir).unwrap();
        let canonical = storage.takes_dir.join("take_0001.wav");
        let first = wav(22);
        fs::write(&canonical, &first).unwrap();
        let a = archive_guided_take(&storage, 1, "Good morning, everyone.", &canonical, None).unwrap();
        assert_eq!(a, archive_guided_take(&storage, 1, "Good morning, everyone.", &canonical, None).unwrap());
        fs::write(&canonical, wav(44)).unwrap();
        let b = archive_guided_take(&storage, 1, "Good morning, everyone.", &canonical, None).unwrap();
        assert_ne!(a, b);
        assert_eq!(fs::read(storage.saved_root.join("Recordings").join(&a[..2]).join(&a).join("canonical.wav")).unwrap(), first);
        assert_eq!(fs::read(&canonical).unwrap(), wav(44));
        assert!(storage.saved_root.join("Recordings").join(&b[..2]).join(&b).is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupted_archive_cannot_be_treated_as_saved() {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit-archive-corrupt-{}-{unique}", std::process::id()));
        let storage = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        fs::create_dir_all(&storage.takes_dir).unwrap();
        let source = storage.takes_dir.join("take_0002.wav");
        fs::write(&source, wav(33)).unwrap();
        let id = archive_guided_take(&storage, 2, "Thanks for joining today.", &source, None).unwrap();
        let item = storage.saved_root.join("Recordings").join(&id[..2]).join(&id).join("canonical.wav");
        fs::write(item, wav(12)).unwrap();
        assert!(archive_guided_take(&storage, 2, "Thanks for joining today.", &source, None).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}

// These tests exercise the bounded cross-location decision without filesystem
// execution or Windows-only native hashing. They are authored but not run in
// the REMOTE_GITHUB source environment.
#[cfg(test)]
mod interrupted_accepted_pair_contract_tests {
    use super::{classify_accepted_take_candidates, AcceptedTakePairEvidence};
    use std::path::Path;

    #[test]
    fn interrupted_save_finds_old_previous_pair_and_new_draft_active_pair() {
        let canonical = [
            (0, Path::new("canonical-accepted")),
            (1, Path::new("canonical-previous")),
            (2, Path::new("canonical-draft")),
        ];
        let native = [
            (0, Path::new("native-accepted")),
            (1, Path::new("native-previous")),
        ];
        let report = classify_accepted_take_candidates(
            17, &canonical, &native, |c, n| match (c.to_str(), n.and_then(Path::to_str)) {
                (Some("canonical-previous"), Some("native-previous")) => Ok(Some("old-receipt".into())),
                (Some("canonical-draft"), Some("native-accepted")) => Ok(Some("new-receipt".into())),
                _ => Ok(None),
            },
        );
        assert!(report.needs_attention);
        assert!(report.previous_backup_present);
        assert!(!report.verification_incomplete);
        assert_eq!(report.verified_pairs.len(), 2);
        assert!(report.verified_pairs.contains(&AcceptedTakePairEvidence {
            canonical_location: "previous".into(), native_location: "previous".into(),
            archive_id: "old-receipt".into(),
        }));
        assert!(report.verified_pairs.contains(&AcceptedTakePairEvidence {
            canonical_location: "draft".into(), native_location: "accepted".into(),
            archive_id: "new-receipt".into(),
        }));
        assert_eq!(report.unverified_files, vec!["accepted canonical WAV"]);
    }

    #[test]
    fn fully_accepted_archived_pair_does_not_require_recovery() {
        let report = classify_accepted_take_candidates(
            4, &[(0, Path::new("canonical"))], &[(0, Path::new("native"))],
            |_, _| Ok(Some("verified-receipt".into())),
        );
        assert!(!report.needs_attention);
        assert!(!report.previous_backup_present);
        assert!(report.unverified_files.is_empty());
        assert_eq!(report.verified_pairs[0].archive_id, "verified-receipt");
    }

    #[test]
    fn unmatched_backup_and_failed_lookup_must_remain_unverified() {
        let report = classify_accepted_take_candidates(
            9, &[(1, Path::new("previous"))], &[(0, Path::new("native"))],
            |_, _| Err("archive_missing_or_unavailable".into()),
        );
        assert!(report.needs_attention);
        assert!(report.verification_incomplete);
        assert!(report.verified_pairs.is_empty());
        assert!(report.unverified_files.contains(&"previous canonical WAV".to_string()));
        assert!(report.unverified_files.contains(&"accepted native-rate WAV".to_string()));
    }

    #[test]
    fn legacy_canonical_only_archive_never_claims_an_unmatched_native_sidecar() {
        let report = classify_accepted_take_candidates(
            12, &[(0, Path::new("canonical"))], &[],
            |_, sidecar| {
                assert!(sidecar.is_none());
                Ok(Some("legacy-canonical-only".into()))
            },
        );
        assert!(!report.needs_attention);
        assert_eq!(report.verified_pairs[0].native_location, "none");
    }
}
