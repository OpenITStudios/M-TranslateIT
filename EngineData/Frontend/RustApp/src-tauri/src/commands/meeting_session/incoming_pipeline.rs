use crate::engine::audio::finalized_utterance::FinalizedMeetingUtterance;
use crate::engine::runtime_settings::load_settings;

use super::committed_turns::{
    commit_meeting_turn, finish_meeting_turn_translation,
};
use super::incoming_deferred::{
    classify_incoming_asr_response, deferred_expiration, enqueue_deferred_incoming,
    record_deferred_expiration,
    requeue_deferred_incoming_front,
    take_due_deferred_incoming, DeferredIncomingJob, DeferredIncomingStage,
    IncomingAsrDisposition, MAX_DEFERRED_INCOMING,
};
use super::{incoming_session_is_eligible, validated_meeting_translation, worker_blocker, MeetingTurnIdentity};
use super::session_state::update_incoming_status;
use super::super::helper_bridge::{
    incoming_deferred_for_required_outbound, send_helper_worker_pcm_task, send_helper_meeting_translation,
};
use std::borrow::Cow;
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum IncomingAudioProcessResult {
    Complete,
    DeferredAsr,
    DeferredTranslation,
}

pub(super) fn process_authoritative_finalized_incoming_pcm(
    utterance: FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
) -> IncomingAudioProcessResult {
    process_incoming_pcm_with_direction(utterance, noise_suppression_enabled, None, None)
}

fn process_incoming_pcm_with_direction(
    utterance: FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
    deferred_deadline: Option<Instant>,
    deferred_direction: Option<(String, String)>,
) -> IncomingAudioProcessResult {
    let session_id = utterance.session_id.as_str();
    let event_sequence = utterance.sequence;
    let utterance_id = utterance.utterance_id;
    let retrying_deferred = deferred_deadline.is_some();
    // Codex selection must not borrow Local Faster-Whisper even for optional
    // incoming captions. Check outside deferred queue ownership so no
    // settings-IO lock is taken while the queue mutex is held.
    if !incoming_session_is_eligible(session_id)
        || !super::local_translated_meeting_route_admitted()
    {
        return IncomingAudioProcessResult::Complete;
    }
    let (source_language, target_language) = deferred_direction
        .unwrap_or_else(|| {
            let settings = load_settings();
            (settings.meeting_listen_source_language, settings.meeting_listen_target_language)
        });

    update_incoming_status(
        session_id,
        "transcribing",
        false,
        "",
        "Finalized Meeting Sound is being transcribed locally.",
    );
    // Keep only the small event identity after transferring PCM ownership.
    let owned_session_id = utterance.session_id.clone();
    let session_id = owned_session_id.as_str();
    let turn = MeetingTurnIdentity {
        session_id, generation: None, event_sequence, utterance_id,
    };
    let (asr, deferred_utterance) = send_helper_worker_pcm_task(
        turn.worker_metadata(None),
        &source_language,
        utterance,
        noise_suppression_enabled,
        deferred_deadline,
    );
    if !incoming_session_is_eligible(session_id) {
        return IncomingAudioProcessResult::Complete;
    }

    if record_deferred_expiration(&asr) {
        update_incoming_status(session_id, "listening", false, "",
            "Expired deferred Meeting Sound was discarded before inference.");
        return IncomingAudioProcessResult::Complete;
    }
    let transcript = match classify_incoming_asr_response(&asr, &source_language) {
        IncomingAsrDisposition::Deferred => {
            let expires_at =
                deferred_expiration(deferred_deadline, Instant::now());
            // Only a verified pre-execution deferral returns owned PCM.
            let Some(utterance) = deferred_utterance else {
                update_incoming_status(
                    session_id, "degraded", true,
                    "meeting_incoming:deferred_pcm_missing",
                    "Incoming ASR returned a deferral without its original PCM. The event was not queued.",
                );
                return IncomingAudioProcessResult::Complete;
            };
            let job = DeferredIncomingJob {
                session_id: owned_session_id.clone(),
                event_sequence,
                utterance_id,
                stage: DeferredIncomingStage::NeedsAsr {
                    utterance,
                    noise_suppression_enabled,
                    source_language,
                    target_language,
                },
                expires_at,
            };
            let held = if retrying_deferred {
                requeue_deferred_incoming_front(job)
            } else {
                enqueue_deferred_incoming(job)
            };
            if held == 0 {
                update_incoming_status(
                    session_id,
                    "degraded",
                    true,
                    "meeting_incoming:deferred_queue_unavailable",
                    "Incoming ASR yielded to required outbound work, but the deferred queue was unavailable. Outbound remains available.",
                );
                return IncomingAudioProcessResult::Complete;
            }
            update_incoming_status(
                session_id,
                "listening",
                false,
                "",
                &format!(
                    "Held {held}/{MAX_DEFERRED_INCOMING} before ASR while your outbound translation finishes. This Meeting Sound segment will resume first when the helper pipeline is available."
                ),
            );
            return IncomingAudioProcessResult::DeferredAsr;
        }
        IncomingAsrDisposition::EmptyTranscript => {
            update_incoming_status(
                session_id,
                "listening",
                false,
                "",
                "Finalized Meeting Sound did not produce stable speech. Incoming remains listening.",
            );
            return IncomingAudioProcessResult::Complete;
        }
        IncomingAsrDisposition::Failed(blocker) => {
            update_incoming_status(
                session_id,
                "listening",
                true,
                &blocker,
                "Incoming ASR failed for the latest finalized Meeting Sound event. Outbound remains available.",
            );
            return IncomingAudioProcessResult::Complete;
        }
        IncomingAsrDisposition::Transcript(transcript) => transcript,
    };

    // The ASR owner already released its PCM before returning.
    // Retain a source-first mixed incoming-audio turn even if MiLMMT later fails,
    // is deferred, or the helper becomes unavailable.
    if !commit_meeting_turn(
        session_id, event_sequence, None, utterance_id, "incoming",
        &source_language, &target_language, transcript, "", None, None,
    ) {
        update_incoming_status(
            session_id, "degraded", true, "meeting_incoming:source_record_failed",
            "Incoming speech was recognized but could not enter the Meeting transcript.",
        );
        return IncomingAudioProcessResult::Complete;
    }

    let held = translate_and_commit_incoming_transcript(
        turn,
        Cow::Borrowed(transcript),
        source_language,
        target_language,
        deferred_deadline,
    );
    if held {
        IncomingAudioProcessResult::DeferredTranslation
    } else {
        IncomingAudioProcessResult::Complete
    }
}

fn translate_and_commit_incoming_transcript(
    turn: MeetingTurnIdentity<'_>,
    transcript: Cow<'_, str>,
    source_language: String,
    target_language: String,
    deferred_deadline: Option<Instant>,
) -> bool {
    let MeetingTurnIdentity { session_id, event_sequence, utterance_id, .. } = turn;
    // A deferred transcript can outlive the earlier ASR admission.
    if !incoming_session_is_eligible(session_id) {
        return false;
    }
    if !super::local_translated_meeting_route_admitted() {
        // Source text was already committed. Retire its pending translation
        // rather than leave a permanent pending turn without a queue owner.
        let _ = finish_meeting_turn_translation(
            session_id, event_sequence, None, utterance_id, "incoming", None,
        );
        update_incoming_status(
            session_id, "degraded", true, "meeting_incoming:translation_engine_not_admitted",
            "Local translation is not admitted; recognized speech was retained.",
        );
        return false;
    }
    update_incoming_status(
        session_id,
        "translating",
        false,
        "",
        "Final Meeting Sound transcript is being translated.",
    );
    let translation = send_helper_meeting_translation(
        turn.worker_metadata(None),
        transcript.as_ref(), &source_language, &target_language, deferred_deadline,
    );
    if !incoming_session_is_eligible(session_id) {
        return false;
    }
    if record_deferred_expiration(&translation) {
        let _ = finish_meeting_turn_translation(
            session_id, event_sequence, None, utterance_id, "incoming", None,
        );
        update_incoming_status(session_id, "listening", false, "",
            "Expired deferred translation was discarded; recognized speech was retained.");
        return false;
    }
    if incoming_deferred_for_required_outbound(&translation) {
        // One owner for all MT deferrals. The first deferral joins the tail;
        // a retried ASR or MT retains its original deadline and front order.
        let job = DeferredIncomingJob {
            session_id: session_id.to_string(),
            event_sequence,
            utterance_id,
            stage: DeferredIncomingStage::NeedsTranslation {
                transcript: transcript.into_owned(),
                source_language,
                target_language,
            },
            expires_at: deferred_expiration(deferred_deadline, Instant::now()),
        };
        let held = if deferred_deadline.is_some() {
            requeue_deferred_incoming_front(job)
        } else {
            enqueue_deferred_incoming(job)
        };
        if held == 0 {
            update_incoming_status(
                session_id, "degraded", true,
                "meeting_incoming:deferred_queue_unavailable",
                "Incoming translation yielded to required outbound work, but the deferred queue was unavailable. Outbound remains available.",
            );
            return false;
        }
        update_incoming_status(
            session_id, "listening", false, "",
            &format!(
                "Held {held}/{MAX_DEFERRED_INCOMING} while your outbound translation finishes. Incoming order and the original deadline are preserved."
            ),
        );
        return true;
    }
    let translated_text = validated_meeting_translation(
        &translation,
        &source_language,
        &target_language,
    );
    if translated_text.is_none() {
        let _ = finish_meeting_turn_translation(
            session_id, event_sequence, None, utterance_id, "incoming", None,
        );
        let blocker = if translation.ok {
            "translation:invalid_meeting_response".to_string()
        } else {
            worker_blocker(&translation, "translation:empty_output")
        };
        update_incoming_status(
            session_id,
            "degraded",
            true,
            &blocker,
            "Incoming translation failed for the latest event. Outbound Meeting translation remains unaffected.",
        );
        return false;
    }

    let translated_text = translated_text.unwrap_or_default();
    let committed = finish_meeting_turn_translation(
        session_id, event_sequence, None, utterance_id, "incoming", Some(translated_text),
    );
    update_incoming_status(
        session_id,
        if committed { "listening" } else { "degraded" },
        !committed,
        if committed {
            ""
        } else {
            "meeting_incoming:commit_rejected"
        },
        if committed {
            "Incoming translation was committed in finalized speech/event order. Listening for current Meeting Sound."
        } else {
            "Incoming translation finished but could not be committed to the canonical Meeting conversation store."
        },
    );
    false
}

pub(super) fn drain_due_deferred_incoming(session_id: &str) {
    for _ in 0..MAX_DEFERRED_INCOMING {
        let Some(job) = take_due_deferred_incoming(session_id, Instant::now()) else {
            break;
        };
        let DeferredIncomingJob {
            event_sequence,
            utterance_id,
            stage,
            expires_at,
            ..
        } = job;
        match stage {
            DeferredIncomingStage::NeedsAsr {
                utterance,
                noise_suppression_enabled,
                source_language,
                target_language,
            } => {
                let result = process_incoming_pcm_with_direction(
                    utterance,
                    noise_suppression_enabled,
                    Some(expires_at),
                    Some((source_language, target_language)),
                );
                if result != IncomingAudioProcessResult::Complete {
                    break;
                }
            }
            DeferredIncomingStage::NeedsTranslation {
                transcript,
                source_language,
                target_language,
            } => {
                if translate_and_commit_incoming_transcript(
                    MeetingTurnIdentity { session_id, generation: None, event_sequence, utterance_id },
                    Cow::Owned(transcript),
                    source_language,
                    target_language,
                    Some(expires_at),
                ) {
                    break;
                }
            }
        }
    }
}
