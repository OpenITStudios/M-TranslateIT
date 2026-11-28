use serde_json::Value;
use std::sync::{Mutex, OnceLock};

use crate::engine::runtime_state::runtime_generation_is_authoritative;

use super::super::helper_bridge_runtime::{unix_ms, HelperBridgeStatus, HelperTaskPriority};
use super::{worker_text, HelperBridgeWorkerResponse};

#[derive(Debug, Clone)]
struct RequiredOutboundFunctionalReadiness {
    generation_token: u64,
    meeting_generation: u64,
    actor_token: String,
    verified_unix_ms: u128,
}

static REQUIRED_OUTBOUND_FUNCTIONAL_READINESS: OnceLock<
    Mutex<Option<RequiredOutboundFunctionalReadiness>>,
> = OnceLock::new();

fn required_outbound_functional_readiness_store(
) -> &'static Mutex<Option<RequiredOutboundFunctionalReadiness>> {
    REQUIRED_OUTBOUND_FUNCTIONAL_READINESS.get_or_init(|| Mutex::new(None))
}

fn required_outbound_functional_readiness_verified_unix_ms(generation_token: u64) -> Option<u128> {
    if generation_token == 0 {
        return None;
    }
    let guard = required_outbound_functional_readiness_store().lock().ok()?;
    guard.as_ref()
        .filter(|cached| cached.generation_token == generation_token && cached.verified_unix_ms > 0)
        .map(|cached| cached.verified_unix_ms)
}

pub(super) fn decorate_functional_readiness_status(mut status: HelperBridgeStatus) -> HelperBridgeStatus {
    let verified_unix_ms =
        required_outbound_functional_readiness_verified_unix_ms(status.generation_token);
    status.functional_outbound_ready =
        status.state == "ready" && status.provider_ready && verified_unix_ms.is_some();
    status.functional_outbound_verified_unix_ms = verified_unix_ms;
    status
}

pub(super) fn remember_required_outbound_functional_readiness(
    generation_token: u64,
    meeting_generation: u64,
    actor_token: String,
) {
    // meeting_generation == 0 is the explicit diagnostic/setup functional proof.
    // It may establish helper-generation readiness but can never be returned by
    // required_outbound_voice_actor_token(), which rejects generation zero and
    // requires current Meeting authority.
    if generation_token == 0 || actor_token.is_empty() {
        return;
    }
    if let Ok(mut guard) = required_outbound_functional_readiness_store().lock() {
        *guard = Some(RequiredOutboundFunctionalReadiness {
            generation_token,
            meeting_generation,
            actor_token,
            verified_unix_ms: unix_ms(),
        });
    }
}

pub fn required_outbound_voice_actor_token(meeting_generation: u64) -> Option<String> {
    if meeting_generation == 0 || !runtime_generation_is_authoritative(meeting_generation) {
        return None;
    }
    let guard = required_outbound_functional_readiness_store().lock().ok()?;
    guard.as_ref()
        .filter(|cached| cached.meeting_generation == meeting_generation && !cached.actor_token.is_empty())
        .map(|cached| cached.actor_token.clone())
}

pub(super) fn invalidate_required_outbound_ai_readiness() {
    if let Ok(mut guard) = required_outbound_functional_readiness_store().lock() {
        *guard = None;
    }
}

pub(super) fn functional_translation_output(response: &HelperBridgeWorkerResponse) -> Option<String> {
    if !response.ok {
        return None;
    }
    let value = &response.worker_response_json;
    let complete = value.get("complete").and_then(Value::as_bool) == Some(true);
    let finished_with_eos = value.get("finished_with_eos").and_then(Value::as_bool) == Some(true);
    let correct_direction = value.get("direction_pair").and_then(Value::as_str) == Some("id->en");
    if !complete || !finished_with_eos || !correct_direction {
        return None;
    }
    worker_text(value, "translated_text")
}

pub(super) fn functional_asr_output(response: &HelperBridgeWorkerResponse) -> bool {
    if !response.ok {
        return false;
    }
    let value = &response.worker_response_json;
    value.get("stage").and_then(Value::as_str) == Some("transcribe")
        && worker_text(value, "transcript_text").is_some()
}

pub(super) fn failed_required_outbound_task_invalidates_cache(
    task: &str,
    priority: HelperTaskPriority,
    response: &HelperBridgeWorkerResponse,
) -> bool {
    if response.ok || priority == HelperTaskPriority::MeetingIncoming {
        return false;
    }
    let value = &response.worker_response_json;
    let blocker = worker_text(value, "blocker").unwrap_or_default();
    // Host-owned, pre-execution admission pressure is not a model fault.
    // Do not infer this from arbitrary response text or a backend failure.
    if response.runtime_claim == "helper_scheduler_request_not_executed"
        && response.generation_token == 0 && response.task == task
        && (blocker.starts_with("helper_scheduler:admission_capacity_exceeded:")
            || blocker.starts_with("helper_scheduler:wait_deadline_exceeded:")
            || (task == "status" && blocker == "helper_bridge:status_busy"))
    {
        return false;
    }
    match task {
        "status" | "asr_preload" | "voice_actor_preflight" => true,
        // An explicit optional EN->ID preload failure is not evidence that the
        // required outbound ID->EN runtime became unusable. Unknown/malformed
        // preload direction stays fail-closed.
        "translation_preload" => {
            value.get("direction_pair").and_then(Value::as_str) != Some("en->id")
        }
        // A finalized speech event may legitimately contain no stable transcript.
        // That is not evidence that the loaded ASR runtime is broken.
        "transcribe" => blocker != "asr:empty_transcript",
        // User/input contract failures do not prove the translation runtime itself
        // became unusable. Other failures conservatively invalidate the cache.
        "translate" => !matches!(
            blocker.as_str(),
            "translation:empty_text"
                | "translation:text_too_large"
                | "translation:input_too_long_for_model"
                | "translation:direction_not_supported"
        ),
        "voice_actor_synthesize" => !matches!(
            blocker.as_str(),
            "voice_actor:empty_text" | "voice_actor:text_too_large"
        ),
        _ => false,
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn functional_cache_identity_keeps_diagnostic_and_meeting_scopes_distinct() {
        let diagnostic = RequiredOutboundFunctionalReadiness {
            generation_token: 9,
            meeting_generation: 0,
            actor_token: "actor-v1".to_string(),
            verified_unix_ms: 1,
        };
        assert_eq!(diagnostic.generation_token, 9);
        assert_eq!(diagnostic.meeting_generation, 0);
        assert_eq!(diagnostic.actor_token, "actor-v1");
        assert!(diagnostic.verified_unix_ms > 0);

        let meeting = RequiredOutboundFunctionalReadiness {
            meeting_generation: 41,
            ..diagnostic
        };
        assert_eq!(meeting.meeting_generation, 41);
        assert_ne!(meeting.meeting_generation, 0);
    }


    #[test]
    fn admission_pressure_is_not_runtime_failure() {
        for task in ["status", "transcribe", "translate", "voice_actor_synthesize"] {
            let mut response = HelperBridgeWorkerResponse {
                ok: false, state: "error".into(), task: task.into(),
                request_id: "scheduler-unavailable".into(), scheduler_priority: "diagnostic".into(),
                message: String::new(), generation_token: 0,
                runtime_claim: "helper_scheduler_request_not_executed".into(),
                worker_response_json: json!({}),
            };
            for blocker in ["helper_scheduler:admission_capacity_exceeded:diagnostic:total=5:limit=5",
                "helper_scheduler:wait_deadline_exceeded:diagnostic:5000ms"] {
                response.worker_response_json = json!({"blocker": blocker});
                assert!(!failed_required_outbound_task_invalidates_cache(task, HelperTaskPriority::Diagnostic, &response));
                response.runtime_claim = "native_cuda_inference_receipt".into();
                assert!(failed_required_outbound_task_invalidates_cache(task, HelperTaskPriority::Diagnostic, &response));
                response.runtime_claim = "helper_scheduler_request_not_executed".into();
            }
            response.worker_response_json = json!({"blocker":"helper_bridge:status_busy"});
            assert_eq!(failed_required_outbound_task_invalidates_cache(task, HelperTaskPriority::Diagnostic, &response), task != "status");
            for blocker in ["helper_scheduler:lock_poisoned", "native_inference:lifecycle_not_ready",
                "native_inference:owner_or_backend_failed", "voice_actor:actor_changed_since_meeting_start"] {
                response.worker_response_json = json!({"blocker": blocker});
                assert!(failed_required_outbound_task_invalidates_cache(task, HelperTaskPriority::Diagnostic, &response));
            }
        }
    }

    #[test]
    fn voice_change_invalidation_clears_functional_readiness() {
        invalidate_required_outbound_ai_readiness();
        remember_required_outbound_functional_readiness(
            17,
            0,
            "actor-before-change".to_string(),
        );
        let mut status = HelperBridgeStatus::default();
        status.state = "ready".to_string();
        status.provider_ready = true;
        status.generation_token = 17;

        assert!(decorate_functional_readiness_status(status.clone()).functional_outbound_ready);

        invalidate_required_outbound_ai_readiness();

        assert!(!decorate_functional_readiness_status(status).functional_outbound_ready);
    }
}
