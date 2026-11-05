use serde_json::json;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::engine::paths::ProjectPaths;
use crate::engine::audio::original_voice_pcm;

use super::committed_turns::{
    commit_meeting_turn, finish_meeting_turn_translation,
    update_committed_turn_delivery_state,
    update_committed_turn_outbound_timing,
};
use super::super::helper_bridge::{required_outbound_voice_actor_token, send_helper_worker_task};
use super::{
    generation_is_live, validated_meeting_asr_transcript, validated_meeting_translation,
    validated_meeting_voice_output_path, worker_blocker, worker_number,
    MeetingOutboundProcessResult,
};
use super::playback_runtime::{enqueue_meeting_playback, PreparedPlaybackJob};
use super::session_state::{
    elapsed_millis, set_outbound_timing, update_outbound_status, OutboundTimingContext,
};

fn tts_output_path(session_id: &str, generation: u64, event_sequence: u64) -> String {
    format!(
        "UserData/CacheData/meeting_tts/{}_g{}_s{}.wav",
        session_id, generation, event_sequence
    )
}

const MEETING_TTS_REQUEST_PREFIX: &str = "UserData/CacheData/meeting_tts/";
// The canonical WAV decoder later validates the container and audio samples.
// At admission, reject files that cannot even contain one sampled WAV frame.
const MIN_MEETING_TTS_WAV_BYTES: u64 = 44;

fn meeting_tts_directory() -> PathBuf {
    PathBuf::from(ProjectPaths::discover().user_cache_dir).join("meeting_tts")
}

fn expected_tts_path_in_dir(tts_dir: &Path, requested_path: &str) -> Option<PathBuf> {
    // Rust owns the requested relative identity. Worker-reported paths do not
    // grant permission to read or remove arbitrary files.
    let filename = requested_path.strip_prefix(MEETING_TTS_REQUEST_PREFIX)?;
    if !filename.ends_with(".wav")
        || filename.len() <= ".wav".len()
        || filename.contains("..")
        || !filename
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return None;
    }
    Some(tts_dir.join(filename))
}

fn verified_tts_output_in_dir(
    tts_dir: &Path,
    requested_path: &str,
    reported_path: &str,
) -> Option<PathBuf> {
    let expected = expected_tts_path_in_dir(tts_dir, requested_path)?;
    // Inspect the original path before canonicalization. A symlink to another
    // file within this SAME cache directory otherwise passes the parent check
    // and can borrow a different session's WAV identity.
    let owned_entry = fs::symlink_metadata(&expected).ok()?;
    if !owned_entry.file_type().is_file()
        || owned_entry.len() <= MIN_MEETING_TTS_WAV_BYTES
    {
        return None;
    }
    let parent = fs::canonicalize(tts_dir).ok()?;
    let resolved_expected = fs::canonicalize(&expected).ok()?;
    if resolved_expected.parent() != Some(parent.as_path())
        || fs::canonicalize(reported_path).ok()? != resolved_expected
    {
        return None;
    }
    // This cheap metadata/path gate is not a WAV decoder. The existing
    // playback owner decodes and validates samples before stream creation.
    Some(resolved_expected)
}

// File identity is checked across validation and open, not only by length.
// Unix inode/device is exact for the observed files; stable Windows std APIs
// expose a best-effort creation/write-time/attribute comparison, not a file ID.
fn checked_tts_metadata_matches_open(checked: &fs::Metadata, opened: &fs::Metadata) -> bool {
    if !checked.is_file() || !opened.is_file() || checked.len() != opened.len() {
        return false;
    }
    if !matches!(
        (checked.modified(), opened.modified()),
        (Ok(before), Ok(after)) if before == after
    ) {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        checked.dev() == opened.dev() && checked.ino() == opened.ino()
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        opened.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
            && checked.creation_time() == opened.creation_time()
            && checked.file_attributes() == opened.file_attributes()
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}

fn open_protected_tts_file(path: &Path) -> Option<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Keep reading allowed, but reject new writers and pathname deletion
        // while this Meeting TTS handle is queued. Inspect reparse points
        // themselves instead of silently following a late-created link.
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path).ok()
}

// Keep a source-owned handle from accepted TTS output through the playback
// queue; no WAV sample data is read or duplicated during admission.
fn open_verified_tts_output_in_dir(
    tts_dir: &Path,
    requested_path: &str,
    reported_path: &str,
) -> Option<(PathBuf, fs::File)> {
    let path = verified_tts_output_in_dir(tts_dir, requested_path, reported_path)?;
    let entry = fs::symlink_metadata(&path).ok()?;
    if !entry.file_type().is_file() || entry.len() <= MIN_MEETING_TTS_WAV_BYTES {
        return None;
    }
    let opened = open_protected_tts_file(&path)?;
    let opened_meta = opened.metadata().ok()?;
    // A same-sized substitute between symlink_metadata and open is not
    // trusted. On Windows the acquired handle additionally denies new writes
    // and renames. On Unix require the same device/inode.
    let after_open_entry = fs::symlink_metadata(&path).ok()?;
    if !checked_tts_metadata_matches_open(&entry, &opened_meta)
        || !checked_tts_metadata_matches_open(&after_open_entry, &opened_meta)
    {
        return None;
    }
    Some((path, opened))
}

// Reuse the original path gate as a last-moment P11 check, while the P13
// decoder reads the already-opened file rather than resolving its pathname.
pub(super) fn prepared_tts_output_still_owned(requested_path: &str, actual_path: &str) -> bool {
    verified_tts_output_in_dir(
        &meeting_tts_directory(), requested_path, actual_path,
    ).is_some()
}

pub(super) fn remove_temporary_tts_paths(requested_path: &str, _reported_path: &str) {
    // Deleting an arbitrary path supplied by a worker response is not safe.
    // Resolve the single requested file from the app-owned cache root instead.
    let tts_dir = meeting_tts_directory();
    remove_temporary_tts_paths_in_dir(&tts_dir, requested_path);
}

fn remove_temporary_tts_paths_in_dir(tts_dir: &Path, requested_path: &str) {
    if let Some(path) = expected_tts_path_in_dir(tts_dir, requested_path) {
        let _ = fs::remove_file(path);
    }
}

pub(super) fn cleanup_meeting_tts_for_session(
    session_id: &str,
    generation: u64,
) -> Result<usize, String> {
    let project_paths = ProjectPaths::discover();
    let tts_dir = PathBuf::from(project_paths.user_cache_dir).join("meeting_tts");
    cleanup_meeting_tts_for_session_in_dir(&tts_dir, session_id, generation)
}

fn cleanup_meeting_tts_for_session_in_dir(
    tts_dir: &Path,
    session_id: &str,
    generation: u64,
) -> Result<usize, String> {
    cleanup_meeting_tts_for_scope_in_dir(tts_dir, session_id, Some(generation))
}

// Called only from no-snapshot Meeting Stop after all helper, playback and
// outbound consumers have verified release. The session identity comes from
// the existing finalized Meeting producer/sequence store, never from a
// worker-reported path or a directory-wide wildcard.
pub(super) fn cleanup_meeting_tts_for_idle_session(session_id: &str) -> Result<usize, String> {
    let tts_dir = meeting_tts_directory();
    cleanup_meeting_tts_for_scope_in_dir(&tts_dir, session_id, None)
}

fn session_tts_filename_matches(name: &str, session_id: &str, generation: Option<u64>) -> bool {
    let prefix = format!("{session_id}_g");
    let Some(rest) = name.strip_prefix(&prefix) else { return false };
    let stem = rest.strip_suffix(".wav.tmp")
        .or_else(|| rest.strip_suffix(".wav"));
    let Some(stem) = stem else { return false };
    let Some((gen, event)) = stem.split_once("_s") else { return false };
    let Ok(actual_generation) = gen.parse::<u64>() else { return false };
    if event.parse::<u64>().is_err() {
        return false;
    }
    generation.is_none_or(|expected| expected == actual_generation)
}

fn cleanup_meeting_tts_for_scope_in_dir(
    tts_dir: &Path,
    session_id: &str,
    generation: Option<u64>,
) -> Result<usize, String> {
    if session_id.trim().is_empty()
        || session_id.contains('/')
        || session_id.contains('\\')
        || !session_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err("meeting_tts_cleanup:invalid_session_id".to_string());
    }
    let entries = match fs::read_dir(tts_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err("meeting_tts_cleanup:read_dir_failed".to_string()),
    };
    let mut removed = 0usize;
    for entry in entries {
        let entry = entry.map_err(|_| "meeting_tts_cleanup:read_entry_failed".to_string())?;
        let file_type = entry
            .file_type()
            .map_err(|_| "meeting_tts_cleanup:file_type_failed".to_string())?;
        // Stop may encounter a symlink created after TTS admission. Removing
        // a matching link removes only that directory entry, not its target.
        // Do not follow directory links or delete arbitrary foreign files.
        if !file_type.is_file() && !file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !session_tts_filename_matches(&name, session_id, generation) {
            continue;
        }
        fs::remove_file(entry.path()).map_err(|_| "meeting_tts_cleanup:remove_failed".to_string())?;
        removed = removed.saturating_add(1);
    }
    Ok(removed)
}

fn stale_outbound_result(
    generation: u64,
    session_id: &str,
    event_sequence: u64,
    utterance_id: u64,
) -> MeetingOutboundProcessResult {
    let _ =
        update_committed_turn_delivery_state(session_id, generation, utterance_id, "interrupted");
    MeetingOutboundProcessResult {
        ok: false,
        delivered: false,
        state: "stale_generation".to_string(),
        blocker: "meeting_outbound:generation_not_authoritative".to_string(),
        note: "Outbound work was discarded because its Meeting generation no longer owns output authority."
            .to_string(),
        generation,
        utterance_sequence: event_sequence,
        runtime_claim: "meeting_outbound_generation_rejected_before_promotion".to_string(),
    }
}

/// A late finalized segment must not borrow the Local ASR/MT/TTS worker when
/// the selected engine is unadmitted. The canonical Meeting generation and
/// source/turn stores remain the only owners of playback and transcripts.
fn unadmitted_outbound_result(
    generation: u64,
    session_id: &str,
    event_sequence: u64,
    utterance_id: u64,
) -> MeetingOutboundProcessResult {
    let _ = update_committed_turn_delivery_state(
        session_id, generation, utterance_id, "interrupted",
    );
    update_outbound_status(
        generation, session_id, "attention_needed", event_sequence,
        false, false, "meeting_outbound:codex_voice_not_admitted",
        "Codex Voice has no admitted Meeting translation path. Local AI and voice output were not authorized.",
    );
    MeetingOutboundProcessResult {
        ok: false,
        delivered: false,
        state: "engine_not_admitted".to_string(),
        blocker: "meeting_outbound:codex_voice_not_admitted".to_string(),
        note: "No local AI worker or Meeting playback was authorized for this utterance."
            .to_string(),
        generation,
        utterance_sequence: event_sequence,
        runtime_claim: "meeting_outbound_engine_not_admitted_before_local_ai".to_string(),
    }
}

pub(super) fn process_outbound_wav(
    generation: u64,
    session_id: &str,
    event_sequence: u64,
    utterance_id: u64,
    speech_duration_ms: u64,
    audio_path: String,
    mut timing: OutboundTimingContext,
) -> MeetingOutboundProcessResult {
    // Defense in depth: a direct/late consumer request cannot invoke local
    // Whisper, MiLMMT or GPT-SoVITS for an unverified Codex preference,
    // even if an old Meeting generation was previously Live.
    if !super::local_translated_meeting_route_admitted() {
        return unadmitted_outbound_result(
            generation, session_id, event_sequence, utterance_id,
        );
    }
    let Some(source_epoch) = original_voice_pcm::translated_epoch(generation) else {
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    };
    if !generation_is_live(generation) || !original_voice_pcm::translated_epoch_matches(generation, source_epoch) {
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    }

    update_outbound_status(
        generation,
        session_id,
        "transcribing",
        event_sequence,
        false,
        true,
        "",
        "Finalized Indonesian speech is being transcribed locally.",
    );
    set_outbound_timing(generation, session_id, event_sequence, &timing.metrics);
    let asr_started_at = Instant::now();
    let asr = send_helper_worker_task(
        "transcribe",
        json!({
            "audio_path": audio_path,
            "language": "id",
            "beam_size": 1,
            "vad_filter": super::FINALIZED_MEETING_ASR_WORKER_VAD,
            "meeting_session_id": session_id,
            "meeting_lane": "you",
            "meeting_generation": generation,
            "meeting_sequence": event_sequence,
            "utterance_id": utterance_id,
            "source_speech_duration_ms": speech_duration_ms,
        }),
    );
    timing.metrics.asr_ms = Some(elapsed_millis(asr_started_at, Instant::now()));
    timing.metrics.asr_scheduler_wait_ms = super::worker_scheduler_wait_ms(&asr);
    timing.metrics.asr_worker_attempt_timing = super::worker_attempt_timing(&asr);
    set_outbound_timing(generation, session_id, event_sequence, &timing.metrics);
    if !generation_is_live(generation) || !original_voice_pcm::translated_epoch_matches(generation, source_epoch) {
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    }
    let transcript = validated_meeting_asr_transcript(&asr, "id");
    if transcript.is_none() {
        let blocker = if asr.ok {
            "asr:invalid_response".to_string()
        } else {
            worker_blocker(&asr, "asr:invalid_response")
        };
        let empty = !asr.ok && blocker == "asr:empty_transcript";
        update_outbound_status(
            generation,
            session_id,
            if empty {
                "listening"
            } else {
                "attention_needed"
            },
            event_sequence,
            false,
            empty,
            if empty { "" } else { &blocker },
            if empty {
                "Finalized speech did not produce a stable transcript. No Meeting output was generated."
            } else {
                "Local ASR failed before translation. No Meeting output was generated."
            },
        );
        return MeetingOutboundProcessResult {
            ok: empty,
            delivered: false,
            state: if empty {
                "no_stable_transcript"
            } else {
                "asr_failed"
            }
            .to_string(),
            blocker: if empty { String::new() } else { blocker },
            note: "No Meeting output was generated from this finalized segment.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_finalized_segment_not_delivered".to_string(),
        };
    }
    let transcript = transcript.unwrap_or_default();
    // Record recognized speech before requesting a translation. A failed model
    // must not erase what was already recognized from the Meeting transcript.
    let recorded = commit_meeting_turn(
        session_id,
        event_sequence,
        Some(generation),
        utterance_id,
        "you",
        "id",
        "en",
        &transcript,
        "",
        Some("translating"),
        Some(timing.metrics.clone()),
    );
    if !recorded {
        update_outbound_status(
            generation, session_id, "attention_needed", event_sequence, false, false,
            "meeting_outbound:source_record_failed",
            "Recognized speech could not be recorded safely. No translated voice was generated.",
        );
        return MeetingOutboundProcessResult {
            ok: false,
            delivered: false,
            state: "source_record_failed".to_string(),
            blocker: "meeting_outbound:source_record_failed".to_string(),
            note: "Recognized source was unavailable in the canonical session store.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_source_not_committed".to_string(),
        };
    }

    update_outbound_status(
        generation,
        session_id,
        "translating",
        event_sequence,
        false,
        true,
        "",
        "Final Indonesian transcript is being translated to English.",
    );
    if !super::local_translated_meeting_route_admitted() {
        return unadmitted_outbound_result(
            generation, session_id, event_sequence, utterance_id,
        );
    }
    let translation_started_at = Instant::now();
    let translation = send_helper_worker_task(
        "translate",
        json!({
            "text": transcript.clone(),
            "source_language": "id",
            "target_language": "en",
            "max_new_tokens": 96,
            "meeting_session_id": session_id,
            "meeting_lane": "you",
            "meeting_generation": generation,
            "meeting_sequence": event_sequence,
            "utterance_id": utterance_id,
        }),
    );
    timing.metrics.translation_ms = Some(elapsed_millis(translation_started_at, Instant::now()));
    timing.metrics.translation_scheduler_wait_ms = super::worker_scheduler_wait_ms(&translation);
    timing.metrics.translation_worker_attempt_timing = super::worker_attempt_timing(&translation);
    timing.metrics.translation_tokenization_ms = worker_number(&translation, "tokenization_ms");
    timing.metrics.translation_inference_ms = worker_number(&translation, "inference_ms");
    timing.metrics.translation_decode_ms = worker_number(&translation, "decode_ms");
    timing.metrics.translation_tokens_per_second =
        worker_number(&translation, "inference_tokens_per_second");
    set_outbound_timing(generation, session_id, event_sequence, &timing.metrics);
    if !generation_is_live(generation) || !original_voice_pcm::translated_epoch_matches(generation, source_epoch) {
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    }
    let translated_text = validated_meeting_translation(&translation, "id", "en");
    if translated_text.is_none() {
        let _ = finish_meeting_turn_translation(
            session_id, event_sequence, Some(generation), utterance_id, "you", None,
        );
        let blocker = if translation.ok {
            "translation:invalid_meeting_response".to_string()
        } else {
            worker_blocker(&translation, "translation:empty_output")
        };
        update_outbound_status(
            generation,
            session_id,
            "attention_needed",
            event_sequence,
            false,
            false,
            &blocker,
            "Local translation failed before TTS. No Meeting output was generated.",
        );
        return MeetingOutboundProcessResult {
            ok: false,
            delivered: false,
            state: "translation_failed".to_string(),
            blocker,
            note: "No Meeting output was generated from this finalized segment.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_translation_failed_before_output".to_string(),
        };
    }
    let translated_text = translated_text.unwrap_or_default();
    if !finish_meeting_turn_translation(
        session_id, event_sequence, Some(generation), utterance_id, "you", Some(&translated_text),
    ) {
        update_outbound_status(
            generation, session_id, "attention_needed", event_sequence, false, false,
            "meeting_outbound:translation_record_failed",
            "Translated speech could not be committed to its original source turn; playback was blocked.",
        );
        return MeetingOutboundProcessResult {
            ok: false,
            delivered: false,
            state: "translation_record_failed".to_string(),
            blocker: "meeting_outbound:translation_record_failed".to_string(),
            note: "Translation was not linked to an authoritative source turn.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_translation_not_committed".to_string(),
        };
    }

    update_outbound_status(
        generation,
        session_id,
        "synthesizing",
        event_sequence,
        false,
        true,
        "",
        "Translated English text is being synthesized locally.",
    );
    let Some(actor_token) = required_outbound_voice_actor_token(generation) else {
        let blocker = "voice_actor:meeting_actor_authority_missing".to_string();
        let _ = update_committed_turn_delivery_state(
            session_id,
            generation,
            utterance_id,
            "output_failed",
        );
        update_outbound_status(
            generation,
            session_id,
            "attention_needed",
            event_sequence,
            false,
            false,
            &blocker,
            "Meeting voice authority is no longer bound to this Meeting generation. Stop and start Translation again before producing more voice output.",
        );
        return MeetingOutboundProcessResult {
            ok: false,
            delivered: false,
            state: "tts_failed".to_string(),
            blocker,
            note: "No Meeting output was generated from this finalized segment.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_voice_actor_authority_missing".to_string(),
        };
    };
    if !super::local_translated_meeting_route_admitted() {
        return unadmitted_outbound_result(
            generation, session_id, event_sequence, utterance_id,
        );
    }
    let requested_tts_path = tts_output_path(session_id, generation, event_sequence);
    let tts_started_at = Instant::now();
    let tts = send_helper_worker_task(
        "voice_actor_synthesize",
        json!({
            "text": translated_text.clone(),
            "source_text": transcript.clone(),
            "source_speech_duration_ms": speech_duration_ms,
            "output_path": requested_tts_path,
            "expected_actor_token": actor_token,
            "meeting_session_id": session_id,
            "meeting_lane": "you",
            "meeting_generation": generation,
            "meeting_sequence": event_sequence,
            "utterance_id": utterance_id,
        }),
    );
    timing.metrics.tts_ms = Some(elapsed_millis(tts_started_at, Instant::now()));
    timing.metrics.tts_scheduler_wait_ms = super::worker_scheduler_wait_ms(&tts);
    timing.metrics.tts_worker_attempt_timing = super::worker_attempt_timing(&tts);
    set_outbound_timing(generation, session_id, event_sequence, &timing.metrics);
    let _ = update_committed_turn_outbound_timing(
        session_id,
        generation,
        utterance_id,
        &timing.metrics,
    );
    let tts_path = validated_meeting_voice_output_path(&tts, &actor_token).unwrap_or_default();
    if !generation_is_live(generation) || !original_voice_pcm::translated_epoch_matches(generation, source_epoch) {
        remove_temporary_tts_paths(&requested_tts_path, &tts_path);
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    }
    if tts_path.is_empty() {
        let blocker = if tts.ok {
            "voice_actor:invalid_synthesis_response".to_string()
        } else {
            worker_blocker(&tts, "voice_actor:missing_output")
        };
        remove_temporary_tts_paths(&requested_tts_path, &tts_path);
        let _ = update_committed_turn_delivery_state(
            session_id,
            generation,
            utterance_id,
            "output_failed",
        );
        update_outbound_status(
            generation,
            session_id,
            "attention_needed",
            event_sequence,
            false,
            false,
            &blocker,
            "Meeting voice synthesis failed before Meeting delivery. No Meeting output was generated.",
        );
        return MeetingOutboundProcessResult {
            ok: false,
            delivered: false,
            state: "tts_failed".to_string(),
            blocker,
            note: "No Meeting output was generated from this finalized segment.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_tts_failed_before_output".to_string(),
        };
    }

    let Some((verified_tts_path, opened_tts_file)) = open_verified_tts_output_in_dir(
        &meeting_tts_directory(),
        &requested_tts_path,
        &tts_path,
    ) else {
        remove_temporary_tts_paths(&requested_tts_path, &tts_path);
        let blocker = "voice_actor:output_path_mismatch";
        let _ = update_committed_turn_delivery_state(
            session_id, generation, utterance_id, "output_failed",
        );
        update_outbound_status(
            generation, session_id, "attention_needed", event_sequence, false, false,
            blocker,
            "Meeting voice output did not match the owned session WAV. No playback was attempted.",
        );
        return MeetingOutboundProcessResult {
            ok: false,
            delivered: false,
            state: "tts_failed".to_string(),
            blocker: blocker.to_string(),
            note: "Worker output was not the requested Meeting session WAV.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_tts_output_identity_mismatch".to_string(),
        };
    };
    let mut playback_job = PreparedPlaybackJob {
        generation,
        source_epoch,
        session_id: session_id.to_string(),
        event_sequence,
        utterance_id,
        requested_tts_path,
        actual_tts_path: verified_tts_path.to_string_lossy().into_owned(),
        opened_tts_file: Some(opened_tts_file),
        timing,
        prepared_at: Instant::now(),
    };

    if !update_committed_turn_delivery_state(
        session_id,
        generation,
        utterance_id,
        "queued",
    ) {
        // Do not leave the Windows read-only share guard open during cleanup:
        // the queued job has not yet transferred ownership to playback.
        drop(playback_job.opened_tts_file.take());
        remove_temporary_tts_paths(&playback_job.requested_tts_path, &playback_job.actual_tts_path);
        update_outbound_status(
            generation, session_id, "attention_needed", event_sequence, false, false,
            "meeting_outbound:delivery_reservation_failed",
            "Voice was synthesized but its locked translation could not reserve audio delivery.",
        );
        return MeetingOutboundProcessResult {
            ok: false,
            delivered: false,
            state: "delivery_reservation_failed".to_string(),
            blocker: "meeting_outbound:delivery_reservation_failed".to_string(),
            note: "No voice output was queued without committed source authority.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_delivery_not_reserved".to_string(),
        };
    }
    update_outbound_status(
        generation,
        session_id,
        "queued",
        event_sequence,
        false,
        true,
        "",
        "Translated voice is prepared and queued for bounded Meeting playback.",
    );

    if let Err(blocker) = enqueue_meeting_playback(playback_job) {
        let stale = blocker == "meeting_playback:generation_not_authoritative";
        let _ = update_committed_turn_delivery_state(
            session_id,
            generation,
            utterance_id,
            if stale { "interrupted" } else { "output_failed" },
        );
        update_outbound_status(
            generation,
            session_id,
            if stale { "listening" } else { "attention_needed" },
            event_sequence,
            false,
            stale,
            if stale { "" } else { &blocker },
            if stale {
                "Prepared voice output was discarded because its Meeting generation lost authority before playback."
            } else {
                "Prepared voice output could not enter the bounded Meeting playback runtime."
            },
        );
        return MeetingOutboundProcessResult {
            ok: stale,
            delivered: false,
            state: if stale { "interrupted" } else { "playback_enqueue_failed" }.to_string(),
            blocker: if stale { String::new() } else { blocker },
            note: "No additional playback attempt was made.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_prepared_output_not_delivered".to_string(),
        };
    }

    MeetingOutboundProcessResult {
        ok: true,
        delivered: false,
        state: "queued".to_string(),
        blocker: String::new(),
        note: "Local ASR, translation, and voice synthesis completed; bounded playback now owns delivery."
            .to_string(),
        generation,
        utterance_sequence: event_sequence,
        runtime_claim: "meeting_outbound_prepared_and_handed_to_bounded_playback".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cleanup_meeting_tts_for_scope_in_dir, cleanup_meeting_tts_for_session_in_dir,
        checked_tts_metadata_matches_open, expected_tts_path_in_dir,
        open_verified_tts_output_in_dir, remove_temporary_tts_paths_in_dir,
        tts_output_path, verified_tts_output_in_dir,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_wav(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("translateit-{label}-{nonce}.wav"))
    }

    fn one_frame_pcm16_wav() -> Vec<u8> {
        // A genuinely structured 46-byte WAV: 44-byte PCM16 mono header
        // plus one 16-bit sample. Full decoding belongs to meeting_output.
        let mut wav = Vec::with_capacity(46);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&38u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&16_000u32.to_le_bytes());
        wav.extend_from_slice(&32_000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&2u32.to_le_bytes());
        wav.extend_from_slice(&0i16.to_le_bytes());
        wav
    }

    #[test]
    fn tts_output_path_is_generation_and_event_scoped() {
        assert_eq!(
            tts_output_path("session-a", 7, 11),
            "UserData/CacheData/meeting_tts/session-a_g7_s11.wav"
        );
    }

    #[test]
    fn session_tts_cleanup_removes_only_matching_generation_files() {
        let root = temp_wav("session-cleanup-root").with_extension("");
        fs::create_dir_all(&root).expect("create tts cleanup root");
        fs::write(root.join("meeting_100_7_g9_s1.wav"), b"a").expect("write matching tts");
        fs::write(root.join("meeting_100_7_g9_s2.wav"), b"b").expect("write matching tts");
        fs::write(root.join("meeting_100_7_g8_s1.wav"), b"c").expect("write old generation");
        fs::write(root.join("other_g9_s1.wav"), b"d").expect("write foreign session");
        fs::write(root.join("meeting_100_7_other_g9_s1.wav"), b"e")
            .expect("write overlapping session");
        fs::write(root.join("meeting_100_7_g9_sbad.wav"), b"f")
            .expect("write invalid event suffix");

        let removed = cleanup_meeting_tts_for_session_in_dir(&root, "meeting_100_7", 9)
            .expect("cleanup session tts");

        assert_eq!(removed, 2);
        assert!(!root.join("meeting_100_7_g9_s1.wav").exists());
        assert!(!root.join("meeting_100_7_g9_s2.wav").exists());
        assert!(root.join("meeting_100_7_g8_s1.wav").exists());
        assert!(root.join("other_g9_s1.wav").exists());
        assert!(root.join("meeting_100_7_other_g9_s1.wav").exists());
        assert!(root.join("meeting_100_7_g9_sbad.wav").exists());

        fs::remove_dir_all(root).expect("remove tts cleanup root");
    }

    #[test]
    fn idle_session_tts_cleanup_handles_all_generations_but_no_other_session() {
        let root = temp_wav("idle-session-tts-cleanup").with_extension("");
        fs::create_dir_all(&root).expect("test cache root");
        for filename in [
            "meeting_100_7_g9_s1.wav",
            "meeting_100_7_g8_s2.wav.tmp",
            "meeting_100_7_other_g9_s1.wav",
            "meeting_100_7_g9_sbad.wav",
            "meeting_100_8_g9_s1.wav",
        ] {
            fs::write(root.join(filename), b"owned-or-foreign").expect("create cache fixture");
        }
        assert_eq!(
            cleanup_meeting_tts_for_scope_in_dir(&root, "meeting_100_7", None),
            Ok(2),
        );
        assert!(!root.join("meeting_100_7_g9_s1.wav").exists());
        assert!(!root.join("meeting_100_7_g8_s2.wav.tmp").exists());
        assert!(root.join("meeting_100_7_other_g9_s1.wav").exists());
        assert!(root.join("meeting_100_7_g9_sbad.wav").exists());
        assert!(root.join("meeting_100_8_g9_s1.wav").exists());
        assert_eq!(
            cleanup_meeting_tts_for_scope_in_dir(&root, "meeting 100 7", None),
            Err("meeting_tts_cleanup:invalid_session_id".to_string()),
        );
        fs::remove_dir_all(root).expect("cleanup test fixture");
    }

    #[test]
    fn tts_cleanup_uses_only_owned_requested_identity() {
        let root = temp_wav("owned-tts-root").with_extension("");
        fs::create_dir_all(&root).expect("create isolated tts root");
        let request = tts_output_path("session-a", 7, 11);
        let expected = root.join("session-a_g7_s11.wav");
        let unrelated = root.join("session-b_g7_s11.wav");
        fs::write(&expected, b"requested").expect("write requested");
        fs::write(&unrelated, b"unrelated").expect("write unrelated");

        remove_temporary_tts_paths_in_dir(&root, &request);
        assert!(!expected.exists());
        assert!(unrelated.exists(), "other session output must survive");

        // A path outside the prescribed identity is never used for deletion.
        assert!(expected_tts_path_in_dir(&root, "../outside.wav").is_none());
        assert!(expected_tts_path_in_dir(&root, "UserData/CacheData/meeting_tts/../other.wav").is_none());
        remove_temporary_tts_paths_in_dir(&root, unrelated.to_string_lossy().as_ref());
        assert!(unrelated.exists());
        fs::remove_dir_all(root).expect("remove isolated root");
    }


    #[cfg(unix)]
    #[test]
    fn same_directory_symlink_cannot_borrow_another_session_wav() {
        use std::os::unix::fs::symlink;

        let root = temp_wav("tts-same-dir-symlink").with_extension("");
        fs::create_dir_all(&root).expect("create isolated tts root");
        let requested = tts_output_path("session-a", 7, 11);
        let expected = root.join("session-a_g7_s11.wav");
        let foreign = root.join("session-b_g7_s11.wav");
        fs::write(&foreign, one_frame_pcm16_wav()).expect("write foreign session audio");
        fs::write(&expected, one_frame_pcm16_wav()).expect("write initially owned audio");
        let approved = verified_tts_output_in_dir(
            &root, &requested, expected.to_string_lossy().as_ref(),
        ).expect("initial regular WAV is owned");
        // Simulate replacement during the playback queue delay.
        fs::remove_file(&expected).expect("remove original queued WAV");
        symlink(&foreign, &expected).expect("replace with foreign-session symlink");
        assert!(verified_tts_output_in_dir(
            &root, &requested, approved.to_string_lossy().as_ref(),
        ).is_none(), "late symlink replacement must fail revalidation");
        assert_eq!(
            cleanup_meeting_tts_for_session_in_dir(&root, "session-a", 7),
            Ok(1), "Stop removes the matching link, not its target",
        );
        assert!(!expected.exists());
        assert!(foreign.exists(), "other session's file must survive cleanup");

        fs::remove_dir_all(root).expect("remove isolated root");
    }

    #[cfg(windows)]
    #[test]
    fn same_directory_symlink_cannot_borrow_another_session_wav_when_supported() {
        use std::os::windows::fs::symlink_file;

        let root = temp_wav("tts-same-dir-symlink").with_extension("");
        fs::create_dir_all(&root).expect("create isolated tts root");
        let requested = tts_output_path("session-a", 7, 11);
        let expected = root.join("session-a_g7_s11.wav");
        let foreign = root.join("session-b_g7_s11.wav");
        fs::write(&foreign, one_frame_pcm16_wav()).expect("write foreign session audio");
        fs::write(&expected, one_frame_pcm16_wav()).expect("write initially owned audio");
        let approved = verified_tts_output_in_dir(
            &root, &requested, expected.to_string_lossy().as_ref(),
        ).expect("initial regular WAV is owned");
        fs::remove_file(&expected).expect("remove queued output before replacement");
        match symlink_file(&foreign, &expected) {
            Ok(()) => {
                assert!(verified_tts_output_in_dir(
                    &root, &requested, approved.to_string_lossy().as_ref(),
                ).is_none(), "late symlink replacement must fail revalidation");
                assert_eq!(
                    cleanup_meeting_tts_for_session_in_dir(&root, "session-a", 7),
                    Ok(1), "Stop removes the matching symlink",
                );
                assert!(!expected.exists());
                assert!(foreign.exists(), "other session output must survive cleanup");
            }
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                // Windows link creation can require Developer Mode or privilege.
                // This OS-specific regression remains UNVERIFIED if unavailable.
            }
            Err(error) => panic!("could not create symlink fixture: {error}"),
        }
        fs::remove_dir_all(root).expect("remove isolated root");
    }


    #[cfg(unix)]
    #[test]
    fn accepted_tts_handle_preserves_original_bytes_after_queued_path_swap() {
        use std::io::Read;

        let root = temp_wav("queued-handle-identity").with_extension("");
        fs::create_dir_all(&root).expect("create isolated TTS fixture");
        let request = tts_output_path("session-a", 7, 11);
        let path = root.join("session-a_g7_s11.wav");
        let retained = root.join("original.wav");
        let original = one_frame_pcm16_wav();
        let mut replacement = original.clone();
        replacement[44] = 0x7f;
        fs::write(&path, &original).expect("create admitted TTS file");
        let (approved, mut opened) = open_verified_tts_output_in_dir(
            &root, &request, path.to_string_lossy().as_ref(),
        ).expect("admitted WAV holds the original file handle");
        assert_eq!(approved, fs::canonicalize(&path).unwrap());

        fs::rename(&path, &retained).expect("rename original WAV");
        fs::write(&path, &replacement).expect("install same-length replacement");
        let mut pinned_bytes = Vec::new();
        opened.read_to_end(&mut pinned_bytes).expect("read original open handle");
        assert_eq!(pinned_bytes, original);
        assert_eq!(fs::read(&path).unwrap(), replacement);
        drop(opened);
        fs::remove_dir_all(root).expect("remove isolated fixture");
    }

    #[test]
    fn opened_tts_handle_admission_rejects_missing_and_header_only_wav() {
        let root = temp_wav("queued-handle-negative").with_extension("");
        fs::create_dir_all(&root).expect("create isolated fixture");
        let request = tts_output_path("session-a", 7, 11);
        let path = root.join("session-a_g7_s11.wav");
        assert!(open_verified_tts_output_in_dir(
            &root, &request, path.to_string_lossy().as_ref(),
        ).is_none());
        fs::write(&path, vec![0u8; 44]).expect("create undersized WAV");
        assert!(open_verified_tts_output_in_dir(
            &root, &request, path.to_string_lossy().as_ref(),
        ).is_none());
        fs::write(&path, one_frame_pcm16_wav()).expect("create valid WAV");
        assert!(open_verified_tts_output_in_dir(
            &root, &request, path.to_string_lossy().as_ref(),
        ).is_some());
        fs::remove_dir_all(root).expect("remove fixture");
    }


    #[cfg(unix)]
    #[test]
    fn same_size_file_swapped_before_handle_open_fails_identity_check() {
        let root = temp_wav("admission-identity").with_extension("");
        fs::create_dir_all(&root).expect("create isolated fixture");
        let path = root.join("session-a_g7_s11.wav");
        let original_path = root.join("prior.wav");
        let mut replacement = one_frame_pcm16_wav();
        replacement[44] = 42;
        fs::write(&path, one_frame_pcm16_wav()).expect("write original file");
        let checked = fs::symlink_metadata(&path).expect("inspect original entry");
        fs::rename(&path, &original_path).expect("replace checked filename");
        fs::write(&path, replacement).expect("create same-size replacement");
        let opened = fs::File::open(&path).expect("open replacement");
        let actual = opened.metadata().expect("read opened file metadata");
        assert!(!checked_tts_metadata_matches_open(&checked, &actual),
            "same size must not equate two different inodes");
        assert!(checked_tts_metadata_matches_open(
            &fs::symlink_metadata(&path).unwrap(), &actual,
        ), "stable same-file admission stays valid");
        drop(opened);
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[cfg(windows)]
    #[test]
    fn held_meeting_tts_handle_denies_new_writers_until_release() {
        let root = temp_wav("protected-wav-open").with_extension("");
        fs::create_dir_all(&root).expect("create isolated fixture");
        let path = root.join("session-a_g7_s11.wav");
        let requested = tts_output_path("session-a", 7, 11);
        fs::write(&path, one_frame_pcm16_wav()).expect("write original audio");
        let (_, held) = open_verified_tts_output_in_dir(
            &root, &requested, path.to_string_lossy().as_ref(),
        ).expect("open with Windows read-sharing guard");
        assert!(fs::OpenOptions::new().write(true).open(&path).is_err(),
            "no new writer may mutate the queued TTS WAV");
        drop(held);
        assert!(fs::OpenOptions::new().write(true).open(&path).is_ok(),
            "write access returns after the last queued handle closes");
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn tts_playback_requires_matching_reported_and_owned_absolute_paths() {
        let root = temp_wav("verified-tts-root").with_extension("");
        fs::create_dir_all(&root).expect("create isolated tts root");
        let request = tts_output_path("session-a", 7, 11);
        let expected = root.join("session-a_g7_s11.wav");
        let unrelated = root.join("session-b_g7_s11.wav");
        fs::write(&expected, one_frame_pcm16_wav()).expect("write requested");
        fs::write(&unrelated, one_frame_pcm16_wav()).expect("write unrelated");

        let owned = verified_tts_output_in_dir(
            &root, &request, expected.to_string_lossy().as_ref(),
        ).expect("worker-reported absolute path matches requested session file");
        assert_eq!(owned, fs::canonicalize(&expected).unwrap());
        assert!(
            verified_tts_output_in_dir(&root, &request, unrelated.to_string_lossy().as_ref()).is_none(),
            "worker may not redirect playback to another session's WAV",
        );
        assert!(
            verified_tts_output_in_dir(&root, &request, "nonexistent-output.wav").is_none(),
            "missing output cannot be promoted",
        );
        assert!(expected_tts_path_in_dir(&root, &request).is_some());

        // A valid path must not admit an empty or header-only result.
        fs::write(&expected, []).expect("truncate owned output");
        assert!(verified_tts_output_in_dir(
            &root, &request, expected.to_string_lossy().as_ref()
        ).is_none(), "empty owned WAV cannot be queued");
        fs::write(&expected, vec![0u8; 44]).expect("write header-only output");
        assert!(verified_tts_output_in_dir(
            &root, &request, expected.to_string_lossy().as_ref()
        ).is_none(), "header-only WAV cannot be queued");

        fs::remove_dir_all(root).expect("remove isolated root");
    }
}
