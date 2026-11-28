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
use super::super::helper_bridge::{
    required_outbound_voice_actor_token, send_helper_worker_pcm_task,
    send_helper_meeting_translation, OutboundPipelineRelease,
    synthesize_native_voice, native_voice_pcm_receipt_matches,
};
use crate::engine::audio::finalized_utterance::FinalizedMeetingUtterance;
use super::{
    generation_is_live, validated_meeting_asr_transcript, validated_meeting_translation,
    worker_blocker, worker_number,
    MeetingOutboundProcessResult,
};
use super::playback_runtime::{
    cleanup_job, enqueue_meeting_playback, PreparedPlaybackJob,
};
use super::session_state::{
    elapsed_millis, set_outbound_timing, update_outbound_status, OutboundTimingContext,
};

fn meeting_tts_directory() -> PathBuf {
    PathBuf::from(ProjectPaths::discover().user_cache_dir).join("meeting_tts")
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

pub(super) fn process_outbound_pcm(
    utterance: FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
    mut timing: OutboundTimingContext,
) -> MeetingOutboundProcessResult {
    let generation = utterance.generation.unwrap_or_default();
    let session_id = utterance.session_id.as_str();
    let event_sequence = utterance.sequence;
    let utterance_id = utterance.utterance_id;
    let speech_duration_ms = utterance.speech_duration_ms;
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

    // Exactly one serialized outbound utterance owns this scope. Release
    // its generation marker even when ASR/MT succeeds but later promotion fails.
    let _pipeline_release = OutboundPipelineRelease::new(generation);
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
    // Keep only the small event identity after transferring PCM ownership.
    let owned_session_id = utterance.session_id.clone();
    let session_id = owned_session_id.as_str();
    let (asr, _) = send_helper_worker_pcm_task(
        json!({
            "meeting_session_id": session_id,
            "meeting_lane": "you",
            "meeting_generation": generation,
            "meeting_sequence": event_sequence,
            "utterance_id": utterance_id,
            "source_speech_duration_ms": speech_duration_ms,
        }),
        "id",
        utterance,
        noise_suppression_enabled,
    );
    let asr_wall_ms = elapsed_millis(asr_started_at, Instant::now());
    timing.metrics.audio_prepare_ms = asr.worker_response_json.get("audio_prepare_ms")
        .and_then(serde_json::Value::as_u64).unwrap_or(0);
    timing.metrics.asr_ms = Some(asr_wall_ms.saturating_sub(timing.metrics.audio_prepare_ms));
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
    // The ASR owner already released its PCM before returning.
    process_outbound_transcript(
        generation,
        session_id,
        event_sequence,
        utterance_id,
        source_epoch,
        transcript,
        timing,
    )
}

fn process_outbound_transcript(
    generation: u64,
    session_id: &str,
    event_sequence: u64,
    utterance_id: u64,
    source_epoch: u64,
    transcript: &str,
    mut timing: OutboundTimingContext,
) -> MeetingOutboundProcessResult {
    if !super::local_translated_meeting_route_admitted() {
        return unadmitted_outbound_result(generation, session_id, event_sequence, utterance_id);
    }
    if !generation_is_live(generation)
        || !original_voice_pcm::translated_epoch_matches(generation, source_epoch)
    {
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    }
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
        transcript,
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
    let translation = send_helper_meeting_translation(
        json!({
            "meeting_session_id": session_id,
            "meeting_lane": "you",
            "meeting_generation": generation,
            "meeting_sequence": event_sequence,
            "utterance_id": utterance_id,
        }),
        transcript, "id", "en",
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
        session_id, event_sequence, Some(generation), utterance_id, "you", Some(translated_text),
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
    let tts_started_at = Instant::now();
    let (tts, native_pcm) = synthesize_native_voice(json!({
        "meeting_session_id": session_id,
        "meeting_lane": "you",
        "meeting_generation": generation,
        "meeting_sequence": event_sequence,
        "utterance_id": utterance_id,
    }), translated_text, &actor_token);
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
    let valid_output = native_voice_pcm_receipt_matches(&tts, &actor_token, native_pcm.as_deref());
    if !generation_is_live(generation) || !original_voice_pcm::translated_epoch_matches(generation, source_epoch) {
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    }
    if !valid_output {
        let blocker = if tts.ok {
            "voice_actor:invalid_synthesis_response".to_string()
        } else {
            worker_blocker(&tts, "voice_actor:missing_output")
        };
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

    let audio = native_pcm.expect("checked native receipt");
    let mut playback_job = PreparedPlaybackJob {
        generation,
        source_epoch,
        session_id: session_id.to_string(),
        event_sequence,
        utterance_id,
        audio,
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
        cleanup_job(&mut playback_job);
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
}
