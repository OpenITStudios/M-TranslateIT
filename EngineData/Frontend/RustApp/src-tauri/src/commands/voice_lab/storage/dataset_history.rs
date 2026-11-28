//! Persistent dataset lineage: existing immutable archive identities survive
//! temporary canonical native-reference preparation. Historical manifests
//! retain their original schema and engine revision for compatibility.
use serde::{Deserialize, Serialize};
use std::fs;

use super::immutable_store::{
    hash_bytes, persist_manifest, read_regular_bounded, valid_id, verify_manifest_by_id,
    write_new_synced,
};
use super::VoiceLabStoragePaths;
use crate::commands::voice_lab::{GuidedDatasetManifest, VOICE_ACTOR_ENGINE_REVISION};

const SNAPSHOT_SCHEMA: u32 = 1;
const SNAPSHOT_REF: &str = "snapshot.ref";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(super) struct TakeLink {
    line_id: u32,
    wav_file: String,
    archive_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub(super) struct DatasetSnapshot {
    schema_version: u32,
    engine_revision: String,
    source_manifest: GuidedDatasetManifest,
    audio: Vec<TakeLink>,
    #[serde(default)]
    from_saved_archive: bool,
}

fn snapshots_root(storage: &VoiceLabStoragePaths) -> std::path::PathBuf {
    storage.saved_root.join("DatasetSnapshots")
}

fn verify_snapshot(storage: &VoiceLabStoragePaths, id: &str) -> Result<DatasetSnapshot, String> {
    let bytes = verify_manifest_by_id(&snapshots_root(storage), id)?;
    let data: DatasetSnapshot = serde_json::from_slice(&bytes)
        .map_err(|_| "voice_lab:snapshot_manifest_invalid")?;
    if data.schema_version != SNAPSHOT_SCHEMA
        || data.engine_revision != VOICE_ACTOR_ENGINE_REVISION
        || data.audio.len() != data.source_manifest.takes.len()
        || data.audio.iter().zip(&data.source_manifest.takes).any(|(a, b)|
            a.line_id != b.line_id || a.wav_file != b.wav_file || !valid_id(&a.archive_id))
    {
        return Err("voice_lab:snapshot_contract_invalid".into());
    }
    // A hash-only manifest is insufficient if the referenced audio later goes
    // missing or becomes corrupt. Verify bytes AND literal prompt binding.
    for (reference, take) in data.audio.iter().zip(&data.source_manifest.takes) {
        super::recording_archive::verify_saved_take_binding(
            storage, &reference.archive_id, take.line_id, &take.exact_text,
        )?;
    }
    Ok(data)
}

/// Publish the same immutable snapshot schema using already verified archive
/// identities. A native-rate sidecar is never copied into the temporary dataset.
pub(super) fn preserve_frozen_dataset_with_archives(
    storage: &VoiceLabStoragePaths,
    manifest: &GuidedDatasetManifest,
    archive_ids: &[String],
    from_saved_archive: bool,
) -> Result<String, String> {
    if archive_ids.len() != manifest.takes.len() {
        return Err("voice_lab:snapshot_archive_selection_mismatch".into());
    }
    let mut audio = Vec::with_capacity(manifest.takes.len());
    for (take, id) in manifest.takes.iter().zip(archive_ids) {
        // Verify the binding and the actual written canonical bytes before
        // publishing a snapshot; the archive still verifies its native sidecar.
        let staged = read_regular_bounded(
            &storage.build_dataset_dir.join(&take.wav_file), 4 * 1024 * 1024 + 4096,
        )?;
        let verified = super::recording_archive::saved_recording_audio(storage, id)?;
        if staged != verified {
            return Err("voice_lab:snapshot_selected_take_changed".into());
        }
        audio.push(TakeLink { line_id: take.line_id, wav_file: take.wav_file.clone(), archive_id: id.clone() });
    }
    let payload = DatasetSnapshot {
        schema_version: SNAPSHOT_SCHEMA,
        engine_revision: VOICE_ACTOR_ENGINE_REVISION.into(),
        source_manifest: manifest.clone(),
        audio,
        from_saved_archive,
    };
    let bytes = serde_json::to_vec(&payload)
        .map_err(|_| "voice_lab:snapshot_serialize_failed")?;
    let id = persist_manifest(&snapshots_root(storage), &bytes)?;
    verify_snapshot(storage, &id)?;
    // Only a disposable pointer is written into the frozen worker dataset.
    // It never changes the upstream dataset.json or training WAV contents.
    let reference = storage.build_dataset_dir.join(SNAPSHOT_REF);
    if fs::symlink_metadata(&reference).is_ok() {
        let old = read_regular_bounded(&reference, 64)?;
        if old != id.as_bytes() {
            return Err("voice_lab:snapshot_ref_conflict".into());
        }
    } else {
        write_new_synced(&reference, id.as_bytes())?;
    }
    Ok(id)
}

/// An absent pointer can only mean an older pre-snapshot run. A malformed or
/// missing referenced snapshot is NOT silently downgraded to legacy success.
pub(super) fn frozen_dataset_snapshot_id(
    storage: &VoiceLabStoragePaths,
) -> Result<Option<String>, String> {
    let reference = storage.build_dataset_dir.join(SNAPSHOT_REF);
    let id = match fs::symlink_metadata(&reference) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("voice_lab:snapshot_ref_unavailable".into()),
        Ok(_) => String::from_utf8(read_regular_bounded(&reference, 64)?)
            .map_err(|_| "voice_lab:snapshot_ref_invalid")?,
    };
    if !valid_id(&id) { return Err("voice_lab:snapshot_ref_invalid".into()); }
    verify_snapshot(storage, &id)?;
    Ok(Some(id))
}


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
struct TrainingLineage {
    schema_version: u32,
    engine_revision: String,
    actor_version_id: String,
    dataset_snapshot_id: String,
}

/// Executed after human approval gates but before actor promotion. This record
/// ties the content-hashed reviewed candidate to its frozen training dataset.
/// An older build without a snapshot stays an explicitly unlinked legacy build.
pub(super) fn preserve_training_lineage(
    storage: &VoiceLabStoragePaths,
    actor_version_id: &str,
) -> Result<Option<String>, String> {
    if !valid_id(actor_version_id) {
        return Err("voice_lab:lineage_actor_id_invalid".into());
    }
    let Some(dataset_snapshot_id) = frozen_dataset_snapshot_id(storage)? else {
        return Ok(None);
    };
    let payload = TrainingLineage {
        schema_version: 1,
        engine_revision: VOICE_ACTOR_ENGINE_REVISION.into(),
        actor_version_id: actor_version_id.into(),
        dataset_snapshot_id,
    };
    let bytes = serde_json::to_vec(&payload)
        .map_err(|_| "voice_lab:lineage_serialize_failed")?;
    let root = storage.saved_root.join("TrainingLineage");
    let id = persist_manifest(&root, &bytes)?;
    let checked = verify_manifest_by_id(&root, &id)?;
    if checked != bytes {
        return Err("voice_lab:lineage_verification_failed".into());
    }
    Ok(Some(id))
}


/// Review an archived-rebuild candidate against the exact frozen WAV bytes.
/// Unlike a legacy build, new recordings accepted afterward do not invalidate
/// an already frozen, self-contained saved-archive evaluation.
pub(crate) fn archived_frozen_review_matches(
    storage: &VoiceLabStoragePaths,
) -> Result<Option<bool>, String> {
    let Some(id) = frozen_dataset_snapshot_id(storage)? else { return Ok(None); };
    let snapshot = verify_snapshot(storage, &id)?;
    let current_manifest = read_regular_bounded(
        &storage.build_dataset_dir.join("dataset.json"), 64 * 1024,
    )?;
    let frozen_manifest: GuidedDatasetManifest = serde_json::from_slice(&current_manifest)
        .map_err(|_| "voice_lab:archived_review_manifest_invalid")?;
    if frozen_manifest != snapshot.source_manifest { return Ok(Some(false)); }
    for (take, link) in frozen_manifest.takes.iter().zip(snapshot.audio.iter()) {
        let staged = read_regular_bounded(
            &storage.build_dataset_dir.join(&take.wav_file), 4 * 1024 * 1024 + 4096,
        )?;
        let verified = super::recording_archive::saved_recording_audio(
            storage, &link.archive_id,
        )?;
        if staged != verified { return Ok(Some(false)); }
    }
    // Current-take Create additionally checks the mutable accepted set.
    Ok(snapshot.from_saved_archive.then_some(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn snapshot_survives_source_dataset_cleanup_and_rejects_corrupt_archive() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir()
            .join(format!("translateit-snapshot-{}-{stamp}", std::process::id()));
        let storage = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        fs::create_dir_all(&storage.build_dataset_dir).unwrap();
        let payload = vec![0_u8; 64_000];
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + payload.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&32_000_u32.to_le_bytes());
        wav.extend_from_slice(&64_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        wav.extend_from_slice(&payload);
        fs::write(storage.build_dataset_dir.join("take_0001.wav"), wav).unwrap();
        let manifest = GuidedDatasetManifest {
            schema_version: 1, authorized_voice_confirmed: true,
            takes: vec![crate::commands::voice_lab::GuidedTakeContract {
                line_id: 1, exact_text: "Good morning.".into(),
                wav_file: "take_0001.wav".into(),
            }],
            held_out_lines: vec![],
        };
        let archive_id = super::super::archive_guided_take(
            &storage, 1, "Good morning.", &storage.build_dataset_dir.join("take_0001.wav"), None,
        ).unwrap();
        let archive_ids = vec![archive_id];
        let id = preserve_frozen_dataset_with_archives(&storage, &manifest, &archive_ids, false).unwrap();
        assert_eq!(id, preserve_frozen_dataset_with_archives(&storage, &manifest, &archive_ids, false).unwrap());
        fs::remove_dir_all(&storage.build_dataset_dir).unwrap();
        assert!(verify_snapshot(&storage, &id).is_ok());
        let item_id = verify_snapshot(&storage, &id).unwrap().audio[0].archive_id.clone();
        let recorded = storage.saved_root.join("Recordings")
            .join(&item_id[..2]).join(&item_id).join("canonical.wav");
        fs::write(recorded, b"changed").unwrap();
        assert!(verify_snapshot(&storage, &id).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dataset_snapshot_hash_distinguishes_source_text_and_take_id() {
        let t = |line: u32, text: &str| DatasetSnapshot {
            schema_version: SNAPSHOT_SCHEMA,
            engine_revision: VOICE_ACTOR_ENGINE_REVISION.into(),
            source_manifest: GuidedDatasetManifest {
                schema_version: 1,
                authorized_voice_confirmed: true,
                takes: vec![crate::commands::voice_lab::GuidedTakeContract {
                    line_id: line, exact_text: text.into(), wav_file: format!("take_{line:04}.wav"),
                }],
                held_out_lines: vec![],
            },
            from_saved_archive: false,
            audio: vec![TakeLink {
                line_id: line, wav_file: format!("take_{line:04}.wav"), archive_id: "a".repeat(64),
            }],
        };
        let a = serde_json::to_vec(&t(1, "Good morning.")).unwrap();
        assert_eq!(hash_bytes(&a), hash_bytes(&a));
        assert_ne!(hash_bytes(&a), hash_bytes(&serde_json::to_vec(&t(1, "New text.")).unwrap()));
        assert_ne!(hash_bytes(&a), hash_bytes(&serde_json::to_vec(&t(2, "Good morning.")).unwrap()));
    }
}
