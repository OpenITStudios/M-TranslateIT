use serde_json::{json, Value};
use std::cell::{Cell, RefCell};

#[cfg(windows)]
mod native;
use std::time::Instant;

use crate::engine::audio::finalized_utterance::{finalized_asr_frame_rejection, FinalizedMeetingUtterance};

use crate::engine::runtime_state::runtime_generation_is_authoritative;

use super::functional_readiness::{
    failed_required_outbound_task_invalidates_cache, invalidate_required_outbound_ai_readiness,
};
use super::request_policy::{
    clear_meeting_outbound_pipeline, incoming_session_generation,
    mark_meeting_outbound_pipeline, meeting_generation,
    meeting_outbound_pipeline_active, task_priority, RequestAdmission,
};
use super::{
    incoming_deferred_for_required_outbound,
    worker_message, HelperBridgeWorkerResponse,
};
use super::super::helper_bridge_runtime::{
    acquire_scoped_helper_task_permit, apply_worker_response, clear_active_request, SCHEDULER_SCOPE_REVOKED, SCHEDULER_DEFERRED_EXPIRED, SCHEDULER_INCOMING_DEFERRED,
    runtime, runtime_claim, unix_ms,
    worker_response_deadline_for_priority,
    HelperBridgeRuntime, HelperTaskPriority,
};

fn response_with_runtime(
    ok: bool,
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
    message: String,
    worker_response: Value,
    runtime: &HelperBridgeRuntime,
    claim: String,
) -> HelperBridgeWorkerResponse {
    HelperBridgeWorkerResponse {
        ok,
        state: runtime.state.clone(),
        task: task.to_string(),
        request_id: request_id.to_string(),
        scheduler_priority: priority.label().to_string(),
        message,
        generation_token: runtime.generation_token,
        runtime_claim: claim,
        worker_response_json: worker_response,
    }
}

fn standalone_blocked_response(
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
    state: &str,
    blocker: &str,
    message: &str,
) -> HelperBridgeWorkerResponse {
    HelperBridgeWorkerResponse {
        ok: false,
        state: state.to_string(),
        task: task.to_string(),
        request_id: request_id.to_string(),
        scheduler_priority: priority.label().to_string(),
        message: message.to_string(),
        generation_token: 0,
        runtime_claim: "helper_scheduler_request_not_executed".to_string(),
        worker_response_json: json!({
            "ok": false,
            "stage": task,
            "request_id": request_id,
            "scheduler_priority": priority.label(),
            "blocker": blocker,
            "note": message,
        }),
    }
}

// The permit is held by send_worker_task_inner. A failed request can still
// have spent time waiting for it (worker absent, transport failure, stale
// generation, or worker replacement). Keep that Rust-measured admission
// evidence on every post-permit failure, without inventing a measurement for
// rejected/expired scheduler requests which never acquired a permit.
fn attach_admitted_wait_to_failure(
    mut response: HelperBridgeWorkerResponse,
    scheduler_wait_ms: u64,
) -> HelperBridgeWorkerResponse {
    // Successful worker replies already receive this field before the bridge
    // applies the result. Only failed replies need this in-place update.
    if response.ok {
        return response;
    }
    if let Some(object) = response.worker_response_json.as_object_mut() {
        // Overwrite any worker-reported value: only Rust owns admission timing.
        object.insert("scheduler_wait_ms".to_string(), json!(scheduler_wait_ms));
    }
    response
}

#[cfg(windows)]
fn status_busy_response(request_id: &str) -> HelperBridgeWorkerResponse {
    standalone_blocked_response("status", request_id, HelperTaskPriority::Diagnostic,
        "checking", "helper_bridge:status_busy",
        "Worker capability was not checked because the runtime is busy.")
}

fn deferred_deadline_elapsed(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

fn expired_deferred_response(task: &str, request_id: &str, priority: HelperTaskPriority) -> HelperBridgeWorkerResponse {
    standalone_blocked_response(task, request_id, priority, "expired",
        SCHEDULER_DEFERRED_EXPIRED,
        "Deferred incoming Meeting work expired before native inference.")
}

fn incoming_deferred_response(task: &str, request_id: &str) -> HelperBridgeWorkerResponse {
    standalone_blocked_response(
        task,
        request_id,
        HelperTaskPriority::MeetingIncoming,
        "deferred",
        SCHEDULER_INCOMING_DEFERRED,
        "Optional incoming Meeting work yielded before execution because required outbound translation currently owns the helper pipeline.",
    )
}

#[derive(Clone, Copy)]
enum StaleMeetingScope { Generation, Session }

impl StaleMeetingScope {
    fn response(self, task: &str, request_id: &str, priority: HelperTaskPriority) -> HelperBridgeWorkerResponse {
        let (state, blocker, message) = match self {
            Self::Generation => ("stale_generation", "helper_scheduler:meeting_generation_not_authoritative",
                "Queued outbound Meeting work was discarded because its generation no longer owns output authority."),
            Self::Session => ("stale_session", "helper_scheduler:incoming_meeting_session_not_eligible",
                "Queued incoming Meeting work was discarded because its application Meeting session is no longer eligible for incoming promotion."),
        };
        standalone_blocked_response(task, request_id, priority, state, blocker, message)
    }
}

// Resolve the exact current incoming generation once before waiting.
// A queued request can only retain this epoch, never adopt a newer session.
fn meeting_wait_generation(
    generation: Option<u64>, session_id: Option<&str>, lane: Option<&str>,
) -> Result<Option<u64>, StaleMeetingScope> {
    if generation.is_some_and(|value| !runtime_generation_is_authoritative(value)) {
        return Err(StaleMeetingScope::Generation);
    }
    if lane == Some("incoming") {
        let incoming = session_id.and_then(incoming_session_generation)
            .ok_or(StaleMeetingScope::Session)?;
        return Ok(generation.or(Some(incoming)));
    }
    Ok(generation)
}

fn stale_meeting_request(
    task: &str, request_id: &str, priority: HelperTaskPriority,
    generation: Option<u64>, session_id: Option<&str>, lane: Option<&str>,
    wait_generation: Option<u64>,
) -> Option<HelperBridgeWorkerResponse> {
    let stale = match meeting_wait_generation(generation, session_id, lane) {
        Err(stale) => Some(stale),
        Ok(current) if current != wait_generation => Some(if generation.is_some() {
            StaleMeetingScope::Generation
        } else { StaleMeetingScope::Session }),
        Ok(_) => None,
    };
    stale.map(|stale| stale.response(task, request_id, priority))
}

#[derive(Clone, Copy)]
struct MeetingPcmInput<'a> {
    utterance: &'a RefCell<Option<FinalizedMeetingUtterance>>,
    language: &'a str,
    noise_suppression_enabled: bool,
    prepare_ms: &'a Cell<u64>,
    deferred_deadline: Option<Instant>,
}

impl MeetingPcmInput<'_> {
    fn matches(&self, task: &str, payload: &Value) -> bool {
        let slot = self.utterance.borrow();
        let Some(u) = slot.as_ref() else { return false; };
        task == "transcribe"
            && payload.get("audio_path").is_none()
            && payload.get("language").is_none()
            && payload.get("meeting_session_id").and_then(Value::as_str) == Some(u.session_id.as_str())
            && payload.get("meeting_lane").and_then(Value::as_str) == Some(u.lane.as_str())
            && payload.get("meeting_sequence").and_then(Value::as_u64) == Some(u.sequence)
            && payload.get("utterance_id").and_then(Value::as_u64) == Some(u.utterance_id)
            && meeting_generation(payload) == u.generation
            && matches!(u.lane.as_str(), "you" | "incoming")
            && ((u.lane == "you") == u.generation.is_some())
    }
}

// Model content is separate from the existing request metadata. Borrow
// text through scheduler admission; only an admitted native request owns it.
#[derive(Clone, Copy)]
struct MeetingTranslationInput<'a> {
    source: &'a str,
    source_language: &'a str,
    target_language: &'a str,
    deferred_deadline: Option<Instant>,
}

#[derive(Clone, Copy)]
enum MeetingInput<'a> {
    Pcm(MeetingPcmInput<'a>),
    Translation(MeetingTranslationInput<'a>),
}

impl<'a> MeetingInput<'a> {
    fn pcm(self) -> Option<MeetingPcmInput<'a>> {
        match self { Self::Pcm(input) => Some(input), _ => None }
    }
    fn translation(self) -> Option<MeetingTranslationInput<'a>> {
        match self { Self::Translation(input) => Some(input), _ => None }
    }
    fn deferred_deadline(self) -> Option<Instant> {
        match self {
            Self::Pcm(input) => input.deferred_deadline,
            Self::Translation(input) => input.deferred_deadline,
        }
    }
}

// PCM stays in Rust ownership, never in the public worker JSON.
enum NativeVoiceIo<'a> {
    Bind,
    Synthesize { text: &'a str, actor_token: &'a str, samples: &'a mut Option<Vec<f32>> },
    ProbeSpeech(Vec<f32>),
}

fn send_worker_task_inner(task: &str, mut payload: Value, input: Option<MeetingInput<'_>>,
                          voice: Option<NativeVoiceIo<'_>>, admission: RequestAdmission,
                          wait_generation: Option<u64>) -> HelperBridgeWorkerResponse {
    let RequestAdmission { priority, generation: meeting_generation,
        session_id: meeting_session_id, lane: meeting_lane } = admission;
    let deferred_deadline = input.and_then(MeetingInput::deferred_deadline);
    let permit = match acquire_scoped_helper_task_permit(priority, wait_generation, deferred_deadline) {
        Ok(permit) => permit,
        Err(error) if error == SCHEDULER_INCOMING_DEFERRED => {
            return incoming_deferred_response(task, "scheduler-incoming-deferred");
        }
        Err(error) if error == SCHEDULER_DEFERRED_EXPIRED => {
            return expired_deferred_response(task, "scheduler-expired-before-admission", priority);
        }
        Err(error) if error == SCHEDULER_SCOPE_REVOKED => {
            let stale = if meeting_generation.is_some() { StaleMeetingScope::Generation } else { StaleMeetingScope::Session };
            return stale.response(task, "scheduler-scope-revoked", priority);
        }
        Err(error) => {
            return standalone_blocked_response(
                task,
                "scheduler-unavailable",
                priority,
                "error",
                &error,
                "Helper scheduler could not admit the request.",
            )
        }
    };
    let request_id = permit.request_id();
    let scheduler_wait_ms = permit.scheduler_wait_ms();
    let response_deadline_ms = worker_response_deadline_for_priority(task, priority);

    // All early exits below occur AFTER this request acquired its permit.
    // Preserve its measured wait even when no valid Python reply exists.
    let response = (|| -> HelperBridgeWorkerResponse {
    if deferred_deadline_elapsed(deferred_deadline) {
        return expired_deferred_response(task, request_id, priority);
    }
    // An incoming request may have entered the scheduler before outbound claimed the
    // pipeline. Re-check after permit acquisition so queued optional work cannot slip
    // between required outbound ASR -> translation -> TTS stages.
    if priority == HelperTaskPriority::MeetingIncoming && meeting_outbound_pipeline_active() {
        return incoming_deferred_response(task, request_id);
    }

    if let Some(response) = stale_meeting_request(
        task,
        request_id,
        priority,
        meeting_generation,
        meeting_session_id.as_deref(),
        meeting_lane,
        wait_generation,
    ) {
        return response;
    }

    // ASR and MT never cross Python IPC, including on preparation errors.
    if voice.is_some() || matches!(task, "asr_preload" | "translation_preload" | "transcribe" | "translate") {
        #[cfg(windows)]
        return native::dispatch(task, &mut payload, input, voice, priority, &permit, response_deadline_ms, meeting_lane, wait_generation);
        #[cfg(not(windows))]
        return standalone_blocked_response(
            task, request_id, priority, "error", "native_inference:platform_unsupported",
            "Native CUDA translation requires Windows.",
        );
    }
    #[cfg(windows)]
    return native::control(task, &payload, &permit, priority, wait_generation, false);
    #[cfg(not(windows))]
    return standalone_blocked_response(task, request_id, priority, "error",
        "native_inference:platform_unsupported", "Native CUDA requires Windows.");
    })();
    attach_admitted_wait_to_failure(response, scheduler_wait_ms)
}

// Keep historical timing fields readable. New native-only requests record
// exactly one attempt, excluding separately measured PCM preparation.
fn observed_elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn response_scheduler_wait_ms(response: &HelperBridgeWorkerResponse) -> Option<u64> {
    response.worker_response_json.get("scheduler_wait_ms").and_then(Value::as_u64)
}

// Rust owns these measurements; never trust a worker-provided summary and
// never turn missing/invalid response data into invented numeric evidence.
fn add_worker_attempt_observation(response: &mut HelperBridgeWorkerResponse, initial_attempt_ms: u64) {
    let wait = response_scheduler_wait_ms(response);
    let Some(object) = response.worker_response_json.as_object_mut() else { return };
    // Keep archive/DTO field compatibility without a retired retry state machine.
    object.insert("worker_attempt_timing".to_string(), json!({
        "initial_attempt_ms": initial_attempt_ms,
        "recovery_ms": null,
        "retry_attempt_ms": null,
        "initial_scheduler_wait_ms": wait,
        "retry_scheduler_wait_ms": null,
        "scheduler_wait_total_ms": wait,
    }));
}

// Only the unscoped UI observation uses this path. Explicit internal status
// and generation-bound preparation continue through normal admission.
pub(super) fn observe_worker_status() -> HelperBridgeWorkerResponse {
    let priority = HelperTaskPriority::Diagnostic;
    #[cfg(windows)]
    {
        if meeting_outbound_pipeline_active() {
            return status_busy_response("status-busy");
        }
        let permit = match crate::commands::helper_bridge_runtime::try_acquire_idle_diagnostic_permit() {
            Ok(Some(permit)) => permit,
            Ok(None) => return status_busy_response("status-busy"),
            Err(blocker) => {
                invalidate_required_outbound_ai_readiness();
                return standalone_blocked_response("status", "scheduler-unavailable", priority,
                    "error", blocker, "Worker capability observation could not verify the scheduler.");
            }
        };
        // Avoid inserting a capability scan between required outbound stages.
        if meeting_outbound_pipeline_active() {
            return attach_admitted_wait_to_failure(status_busy_response(permit.request_id()), permit.scheduler_wait_ms());
        }
        let response = native::control("status", &json!({}), &permit, priority, None, true);
        if failed_required_outbound_task_invalidates_cache("status", priority, &response) {
            invalidate_required_outbound_ai_readiness();
        }
        return attach_admitted_wait_to_failure(response, permit.scheduler_wait_ms());
    }
    #[cfg(not(windows))]
    standalone_blocked_response("status", "status-unsupported", priority, "error",
        "native_inference:platform_unsupported", "Native CUDA requires Windows.")
}

pub(super) fn send_worker_task(task: &str, payload: Value) -> HelperBridgeWorkerResponse {
    send_worker_task_with_input(task, payload, None, None)
}

pub(super) fn bind_native_voice(payload: Value) -> HelperBridgeWorkerResponse {
    send_worker_task_with_input("voice_actor_preflight", payload, None, Some(NativeVoiceIo::Bind))
}

pub(super) fn synthesize_native_voice(payload: Value, text: &str, actor_token: &str) -> (HelperBridgeWorkerResponse, Option<Vec<f32>>) {
    let mut samples = None;
    let response = send_worker_task_with_input("voice_actor_synthesize", payload, None,
        Some(NativeVoiceIo::Synthesize { text, actor_token, samples: &mut samples }));
    if !response.ok { samples = None; }
    (response, samples)
}

pub(super) fn transcribe_native_voice_probe(payload: Value, samples: Vec<f32>) -> HelperBridgeWorkerResponse {
    send_worker_task_with_input("transcribe", payload, None, Some(NativeVoiceIo::ProbeSpeech(samples)))
}

pub(super) fn send_meeting_translation(
    payload: Value, source: &str, source_language: &str, target_language: &str,
    deferred_deadline: Option<Instant>,
) -> HelperBridgeWorkerResponse {
    let input = MeetingTranslationInput { source, source_language, target_language, deferred_deadline };
    send_worker_task_with_input("translate", payload, Some(MeetingInput::Translation(input)), None)
}

pub(super) fn send_worker_pcm_task(
    payload: Value,
    language: &str,
    utterance: FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
    deferred_deadline: Option<Instant>,
) -> (HelperBridgeWorkerResponse, Option<FinalizedMeetingUtterance>) {
    let rejection = finalized_asr_frame_rejection(&utterance);
    let utterance = RefCell::new(Some(utterance));
    let prepare_ms = Cell::new(0);
    let input = MeetingPcmInput { utterance: &utterance, language, noise_suppression_enabled, prepare_ms: &prepare_ms, deferred_deadline };
    let mut response = if !input.matches("transcribe", &payload) {
        standalone_blocked_response(
            "transcribe", "invalid-meeting-pcm", task_priority("transcribe", &payload),
            "error", "asr:finalized_pcm_contract_invalid",
            "Finalized PCM Meeting event ownership is invalid.",
        )
    } else if let Some(rejection) = rejection {
        standalone_blocked_response(
            "transcribe", "invalid-meeting-pcm", task_priority("transcribe", &payload),
            "error", rejection, "Finalized PCM is outside the admitted native ASR contract.",
        )
    } else {
        send_worker_task_with_input("transcribe", payload, Some(MeetingInput::Pcm(input)), None)
    };
    if let Some(object) = response.worker_response_json.as_object_mut() {
        object.insert("audio_prepare_ms".into(), json!(prepare_ms.get()));
    }
    // Native execution consumes the allocation; only an admitted incoming
    // deferral before execution can return the original frame.
    let deferred = utterance.into_inner().and_then(|owned| retain_deferred_pcm(&response, owned));
    (response, deferred)
}

fn retain_deferred_pcm(
    response: &HelperBridgeWorkerResponse,
    utterance: FinalizedMeetingUtterance,
) -> Option<FinalizedMeetingUtterance> {
    (utterance.lane == "incoming" && incoming_deferred_for_required_outbound(response))
        .then_some(utterance)
}

fn send_worker_task_with_input(task: &str, payload: Value, input: Option<MeetingInput<'_>>,
                               voice: Option<NativeVoiceIo<'_>>) -> HelperBridgeWorkerResponse {
    let admission = RequestAdmission::from_payload(task, &payload);
    let priority = admission.priority;
    let outbound_generation = admission.outbound_generation();
    let wait_generation = match meeting_wait_generation(admission.generation, admission.session_id.as_deref(), admission.lane) {
        Ok(generation) => generation,
        Err(stale) => return stale.response(task, "scheduler-stale-before-admission", priority),
    };
    if deferred_deadline_elapsed(input.and_then(MeetingInput::deferred_deadline)) {
        return expired_deferred_response(task, "incoming-expired-before-scheduler", priority);
    }
    // Reject new optional incoming stages immediately while a required outbound
    // utterance owns the helper pipeline. The post-permit check in the inner path
    // also catches incoming work that was already queued before this claim existed.
    if priority == HelperTaskPriority::MeetingIncoming && meeting_outbound_pipeline_active() {
        return incoming_deferred_response(task, "incoming-deferred-before-scheduler");
    }

    if let Some(generation) = outbound_generation {
        mark_meeting_outbound_pipeline(generation);
    }

    let first_started = Instant::now();
    let mut response = send_worker_task_inner(task, payload, input, voice, admission, wait_generation);
    let initial_attempt_ms = observed_elapsed_ms(first_started)
        .saturating_sub(input.and_then(MeetingInput::pcm).map_or(0, |pcm| pcm.prepare_ms.get()));

    if let Some(generation) = outbound_generation {
        // Transport failure closes the exact native/voice owner. Stop must
        // verify cleanup; never replay a phrase or restart only Python.
        if task == "voice_actor_synthesize"
            || !response.ok
            || !runtime_generation_is_authoritative(generation)
        {
            clear_meeting_outbound_pipeline(generation);
        }
    }

    // A discarded old session cannot invalidate a newer generation's
    // readiness merely because it was cancelled while waiting.
    if !matches!(response.state.as_str(), "stale_generation" | "stale_session")
        && failed_required_outbound_task_invalidates_cache(task, priority, &response) {
        invalidate_required_outbound_ai_readiness();
    }
    if outbound_generation.is_some() {
        add_worker_attempt_observation(&mut response, initial_attempt_ms);
    }
    response
}

#[cfg(test)]
mod scheduler_failure_receipt_tests {
    use super::{attach_admitted_wait_to_failure, standalone_blocked_response};
    use super::super::super::helper_bridge_runtime::HelperTaskPriority;
    use serde_json::json;

    #[test]
    fn post_permit_bridge_failure_retains_measured_wait_and_blocker() {
        let failed = standalone_blocked_response(
            "translate", "helper-42", HelperTaskPriority::MeetingOutbound,
            "error", "helper_bridge:translate_read_failed:timeout",
            "Worker response was not received.",
        );
        let original_request = failed.request_id.clone();
        let original_claim = failed.runtime_claim.clone();
        let response = attach_admitted_wait_to_failure(failed, 275);
        let body = &response.worker_response_json;
        assert!(!response.ok);
        assert_eq!(response.request_id, original_request);
        assert_eq!(response.runtime_claim, original_claim);
        assert_eq!(body["blocker"], "helper_bridge:translate_read_failed:timeout");
        assert_eq!(body["scheduler_wait_ms"], 275);
        assert_eq!(body["scheduler_priority"], "meeting_outbound");
    }

    #[test]
    fn scheduler_rejection_without_permit_must_not_invent_wait_time() {
        let rejected = standalone_blocked_response(
            "transcribe", "scheduler-unavailable", HelperTaskPriority::MeetingOutbound,
            "error", "helper_scheduler:wait_deadline_exceeded:meeting_outbound:120000ms",
            "Scheduler could not admit the request.",
        );
        let body = &rejected.worker_response_json;
        assert!(body.get("scheduler_wait_ms").is_none());
        assert!(!rejected.ok);
    }

    #[test]
    fn rust_admission_receipt_overrides_untrusted_worker_wait() {
        let mut failure = standalone_blocked_response(
            "voice_actor_synthesize", "helper-43", HelperTaskPriority::MeetingOutbound,
            "error", "helper_bridge:worker_exited", "Worker ended.",
        );
        failure.worker_response_json = json!({
            "ok": false, "blocker": "helper_bridge:worker_exited",
            "scheduler_wait_ms": 999_999, "attempt": "failed"
        });
        let response = attach_admitted_wait_to_failure(failure, 12);
        let body = &response.worker_response_json;
        assert_eq!(body["scheduler_wait_ms"], 12);
        assert_eq!(body["attempt"], "failed");
    }
}

#[cfg(test)]
mod worker_attempt_observation_tests {
    use super::{add_worker_attempt_observation, response_scheduler_wait_ms,
        standalone_blocked_response};
    use super::super::super::helper_bridge_runtime::HelperTaskPriority;
    use serde_json::json;

    fn fake_response(wait: Option<u64>) -> super::HelperBridgeWorkerResponse {
        let mut response = standalone_blocked_response(
            "translate", "helper-71", HelperTaskPriority::MeetingOutbound,
            "error", "helper_bridge:translate_read_failed:timeout", "transport failed",
        );
        if let Some(wait) = wait {
            response.worker_response_json["scheduler_wait_ms"] = json!(wait);
        }
        response
    }

    #[test]
    fn single_attempt_preserves_wait_and_has_no_retry_evidence() {
        let mut response = fake_response(Some(13));
        add_worker_attempt_observation(&mut response, 80);
        let value = &response.worker_response_json;
        assert_eq!(value["scheduler_wait_ms"], 13);
        let timing = &value["worker_attempt_timing"];
        assert_eq!(timing["initial_attempt_ms"], 80);
        assert_eq!(timing["initial_scheduler_wait_ms"], 13);
        assert_eq!(timing["scheduler_wait_total_ms"], 13);
        for key in ["recovery_ms", "retry_attempt_ms", "retry_scheduler_wait_ms"] {
            assert!(timing[key].is_null());
        }
    }

    #[test]
    fn missing_or_invalid_wait_never_becomes_numeric_evidence() {
        let mut response = fake_response(None);
        add_worker_attempt_observation(&mut response, 1);
        assert!(response.worker_response_json["worker_attempt_timing"]["scheduler_wait_total_ms"].is_null());
        response.worker_response_json["scheduler_wait_ms"] = json!("invalid");
        add_worker_attempt_observation(&mut response, 1);
        assert!(response.worker_response_json["worker_attempt_timing"]["scheduler_wait_total_ms"].is_null());
        response.worker_response_json = json!("non-object");
        add_worker_attempt_observation(&mut response, 1);
        assert_eq!(response.worker_response_json, json!("non-object"));
        assert_eq!(response_scheduler_wait_ms(&response), None);
    }

}

#[cfg(test)]
mod finalized_pcm_handoff_tests {
    use super::*;
    use crate::engine::audio::AudioFrame;

    #[test]
    fn meeting_pcm_requires_exact_event_identity_and_reuses_frame_limits() {
        let utterance = FinalizedMeetingUtterance {
            session_id: "session-test".into(),
            sequence: 9,
            lane: "you".into(),
            generation: Some(7),
            utterance_id: 8,
            finalized_at: Instant::now(),
            enqueued_at: Instant::now(),
            finalized_unix_ms: 1,
            speech_boundary_ms: 300,
            speech_duration_ms: 300,
            finalization_ms: 0,
            capture_provenance: None,
            frame: AudioFrame { sample_rate_hz: 16_000, channels: 1, samples: vec![0.1; 4_800] },
        };
        let prepare_ms = Cell::new(0);
        let payload = json!({
            "meeting_session_id": "session-test", "meeting_lane": "you",
            "meeting_generation": 7, "meeting_sequence": 9, "utterance_id": 8,
        });
        assert!(finalized_asr_frame_rejection(&utterance).is_none());
        let utterance = RefCell::new(Some(utterance));
        let input = MeetingPcmInput { utterance: &utterance, language: "id", noise_suppression_enabled: false, prepare_ms: &prepare_ms, deferred_deadline: None };
        assert!(input.matches("transcribe", &payload));
        for (key, value) in [
            ("meeting_session_id", json!("another")),
            ("meeting_lane", json!("incoming")),
            ("meeting_generation", json!(6)),
            ("meeting_sequence", json!(10)),
            ("utterance_id", json!(9)),
            ("audio_path", json!("unowned.wav")),
            ("language", json!("en")),
        ] {
            let mut invalid = payload.clone();
            invalid[key] = value;
            assert!(!input.matches("transcribe", &invalid), "{key}");
        }
        assert!(!input.matches("translate", &payload));
        let mut utterance = utterance.into_inner().unwrap();
        utterance.frame.samples.truncate(4_799);
        assert_eq!(finalized_asr_frame_rejection(&utterance).unwrap(),
                   "asr:finalized_segment_too_short");

        // The caller transfers its original allocation; only an actual
        // scheduler deferral may return it, never a worker-supplied blocker.
        utterance.frame.samples.push(0.1);
        utterance.lane = "incoming".into();
        utterance.generation = None;
        let ptr = utterance.frame.samples.as_ptr();
        let deferred = incoming_deferred_response("transcribe", "incoming-owned-pcm");
        let returned = retain_deferred_pcm(&deferred, utterance).expect("deferred original PCM");
        assert_eq!(ptr, returned.frame.samples.as_ptr());
        assert_eq!(returned.frame.samples.len(), 4_800);

        let mut untrusted = deferred.clone();
        untrusted.runtime_claim = "helper_process_ready_ping_verified".into();
        assert!(!incoming_deferred_for_required_outbound(&untrusted));
        let mut completed = deferred.clone();
        completed.ok = true;
        assert!(!incoming_deferred_for_required_outbound(&completed));
        assert!(retain_deferred_pcm(&untrusted, returned).is_none());
    }
}
