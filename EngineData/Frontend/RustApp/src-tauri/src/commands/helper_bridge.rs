use serde::Serialize;
use serde_json::{json, Value};
use std::sync::TryLockError;

use crate::commands::diagnostic_trace::{
    trace_command_end, trace_command_error, trace_command_start,
};
use crate::engine::runtime_state::{
    admit_app_runtime_operation, latest_runtime_session_state,
    runtime_generation_is_authoritative,
};

use super::helper_bridge_runtime::{
    action_result, runtime, set_blocked, status_from_runtime, stop_child, unix_ms,
    HelperBridgeActionResult, HelperBridgeStatus,
};

mod functional_probe;
mod functional_readiness;
mod request_policy;
mod transport;

use functional_probe::run_required_outbound_ai_probe;
use transport::send_worker_task;
use functional_readiness::{
    decorate_functional_readiness_status, invalidate_required_outbound_ai_readiness,
    remember_required_outbound_functional_readiness,
};
pub use functional_readiness::required_outbound_voice_actor_token;
pub(crate) use request_policy::{meeting_outbound_pipeline_active, OutboundPipelineRelease};
use request_policy::{
    clear_any_meeting_outbound_pipeline, clear_meeting_outbound_pipeline_for_session,
    live_outbound_generation_is_authoritative,
};


#[derive(Debug, Clone, Serialize)]
pub struct HelperBridgeWorkerResponse {
    pub ok: bool,
    pub state: String,
    pub task: String,
    pub request_id: String,
    pub scheduler_priority: String,
    pub message: String,
    pub generation_token: u64,
    pub runtime_claim: String,
    // Retain the parsed worker value internally. The frontend's historical
    // JSON-string field is materialized only at the Tauri serialization boundary.
    #[serde(serialize_with = "serialize_worker_response_json")]
    pub worker_response_json: Value,
}

fn serialize_worker_response_json<S>(value: &Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&value.to_string())
}

fn clean_helper_text(value: &str, max_chars: usize) -> String {
    value
        .trim()
        .chars()
        .filter(|character| {
            *character != '\0'
                && !('\u{0001}'..='\u{0008}').contains(character)
                && !('\u{000b}'..='\u{001f}').contains(character)
                && *character != '\u{007f}'
        })
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_string()
}

// Native identity is admitted only from the Rust-owned outer receipt,
// never a JSON claim supplied by the legacy Python worker.
pub(super) const NATIVE_INFERENCE_CLAIM: &str = "native_cuda_inference_receipt";
pub(super) const NATIVE_STATUS_CLAIM: &str = "native_cuda_status_receipt";
pub(super) const NATIVE_ASR_MODEL: &str = "qwen3-asr-1.7b-q8_0";
pub(super) const NATIVE_MT_MODEL: &str = "hy-mt2-1.8b-q6_k";
pub(super) const NATIVE_MT_REVISION: &str = "a0c709d9fac510f2c807aa3af52872340dc37a4a";

pub(super) fn meeting_asr_compute_admitted(response: &HelperBridgeWorkerResponse) -> bool {
    let value = &response.worker_response_json;
    response.runtime_claim == NATIVE_INFERENCE_CLAIM
        && value.get("device").and_then(Value::as_str) == Some("cuda")
        && value.get("compute_type").and_then(Value::as_str) == Some("q8_0")
        && value.get("model_id").and_then(Value::as_str) == Some(NATIVE_ASR_MODEL)
}

pub(super) fn meeting_translation_compute_admitted(response: &HelperBridgeWorkerResponse) -> bool {
    let value = &response.worker_response_json;
    response.runtime_claim == NATIVE_INFERENCE_CLAIM
        && value.get("device").and_then(Value::as_str) == Some("cuda")
        && value.get("precision").and_then(Value::as_str) == Some("q6_k")
        && value.get("model_id").and_then(Value::as_str) == Some(NATIVE_MT_MODEL)
        && value.get("model_revision").and_then(Value::as_str) == Some(NATIVE_MT_REVISION)
        && value.get("translation_degraded").and_then(Value::as_bool) == Some(false)
        && value.get("translation_fallback_reason").and_then(Value::as_str) == Some("")
}

fn worker_text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|text| clean_helper_text(text, 500))
        .filter(|text| !text.is_empty())
}

fn worker_message(task: &str, response: &Value) -> String {
    worker_text(response, "note")
        .or_else(|| worker_text(response, "blocker"))
        .or_else(|| worker_text(response, "stage"))
        .unwrap_or_else(|| format!("Helper worker task {task} completed."))
}





















pub fn send_helper_worker_task(task: &str, payload: Value) -> HelperBridgeWorkerResponse {
    send_worker_task(task, payload)
}

// Local development is native-first by maintainer decision. The old
// opt-in switch cannot re-enable a retired voice creation path.
// Keep this predicate temporarily for remaining legacy caller cleanup.
pub(crate) fn native_voice_candidate_enabled() -> bool {
    cfg!(windows)
}

pub(crate) fn synthesize_native_voice(payload: Value, text: &str, actor_token: &str) -> (HelperBridgeWorkerResponse, Option<Vec<f32>>) {
    transport::synthesize_native_voice(payload, text, actor_token)
}

pub(crate) fn native_voice_pcm_receipt_matches(response: &HelperBridgeWorkerResponse,
                                              token: &str, samples: Option<&[f32]>) -> bool {
    let body = &response.worker_response_json;
    response.ok && response.task == "voice_actor_synthesize"
        && response.runtime_claim == NATIVE_INFERENCE_CLAIM
        && !token.is_empty() && body.get("actor_token").and_then(Value::as_str) == Some(token)
        && body.get("stage").and_then(Value::as_str) == Some("voice_actor_synthesize")
        && response.generation_token != 0
        && body.get("device").and_then(Value::as_str) == Some("cuda")
        && body.get("cuda_pci_id").and_then(Value::as_str).is_some_and(|id| !id.is_empty())
        && body.get("sample_rate_hz").and_then(Value::as_u64) == Some(44_100)
        && body.get("model_id").and_then(Value::as_str) == Some("zonos3-mini-q8_0")
        && samples.is_some_and(|samples| !samples.is_empty() && samples.len() <= 44_100 * 60
            && body.get("sample_count").and_then(Value::as_u64) == Some(samples.len() as u64))
}

// Internal Meeting translation content stays borrowed until native admission.
pub(crate) fn send_helper_meeting_translation(
    payload: Value, source: &str, source_language: &str, target_language: &str,
) -> HelperBridgeWorkerResponse {
    transport::send_meeting_translation(payload, source, source_language, target_language)
}

// Internal Meeting-only owned PCM handoff; PCM never enters JSON. Only
// a pre-execution incoming deferral returns its original owned frame.
pub(crate) fn send_helper_worker_pcm_task(
    payload: Value,
    language: &str,
    utterance: crate::engine::audio::finalized_utterance::FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
) -> (HelperBridgeWorkerResponse, Option<crate::engine::audio::finalized_utterance::FinalizedMeetingUtterance>) {
    transport::send_worker_pcm_task(payload, language, utterance, noise_suppression_enabled)
}

pub(crate) fn incoming_deferred_for_required_outbound(response: &HelperBridgeWorkerResponse) -> bool {
    !response.ok && response.state == "deferred"
        && response.scheduler_priority == "meeting_incoming"
        && response.runtime_claim == "helper_scheduler_request_not_executed"
        && response.worker_response_json.get("blocker").and_then(Value::as_str)
            == Some("helper_scheduler:incoming_deferred_for_outbound")
}

pub(crate) fn invalidate_required_outbound_readiness_for_voice_change() {
    invalidate_required_outbound_ai_readiness();
}

#[cfg(target_os = "windows")]
pub(crate) fn invalidate_required_outbound_readiness_for_power_transition() {
    invalidate_required_outbound_ai_readiness();
}



pub fn verify_required_outbound_ai_runtime() -> Result<(), &'static str> {
    let result = run_required_outbound_ai_probe(None);
    let mut host = runtime().lock().map_err(|_| "local translation runtime")?;
    if !host.release_idle_native_inference() {
        invalidate_required_outbound_ai_readiness();
        return Err("native inference cleanup");
    }
    let (generation_token, actor_token) = result?;
    if host.generation_token != generation_token || !public_worker_start_has_idle_audio_owner() {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }
    remember_required_outbound_functional_readiness(generation_token, 0, actor_token);
    Ok(())
}

pub fn prepare_required_outbound_ai_runtime(meeting_generation: u64) -> Result<(), &'static str> {
    let (generation_token, actor_token) = run_required_outbound_ai_probe(Some(meeting_generation))?;
    if !runtime_generation_is_authoritative(meeting_generation) {
        invalidate_required_outbound_ai_readiness();
        return Err("Meeting authority");
    }
    remember_required_outbound_functional_readiness(
        generation_token,
        meeting_generation,
        actor_token,
    );
    Ok(())
}

/// Future Codex TTS-only admission check. It never proves or activates
/// cloud Voice and never overwrites the existing Local full-pipeline proof.
pub(crate) fn prepare_codex_voice_only_runtime(
    meeting_generation: u64,
) -> Result<(u64, String), &'static str> {
    if crate::engine::meeting_engine_admission()
        != crate::engine::settings::MeetingEngineAdmission::Codex
    {
        return Err("codex_voice:engine_not_selected");
    }
    let _ = meeting_generation;
    Err("codex_voice:native_voice_integration_pending")
}

// The canonical helper mutex owns native lifecycle and inference.
// Read-only status cannot wait for that owner. A busy lock is UNKNOWN, never
// Ready, Stopped, a guessed generation, or a reason to restart the worker.
fn status_lock_busy() -> HelperBridgeStatus {
    HelperBridgeStatus {
        state: "checking".to_string(),
        message: "Local AI worker status is being checked; current readiness is not yet verified.".to_string(),
        cuda_ready: false,
        provider_ready: false,
        functional_outbound_ready: false,
        functional_outbound_verified_unix_ms: None,
        degraded_mode: false,
        active_task: None,
        active_request_id: None,
        active_meeting_generation: None,
        active_meeting_session_id: None,
        active_meeting_lane: None,
        generation_token: 0,
        last_error: None,
        stderr_log_path: None,
        updated_unix_ms: unix_ms(),
        runtime_claim: "helper_bridge:status_lock_busy".to_string(),
    }
}

pub fn get_helper_bridge_status() -> HelperBridgeStatus {
    let started = trace_command_start("get_helper_bridge_status", "reading helper bridge status");
    match runtime().try_lock() {
        Ok(runtime) => {
            let result = decorate_functional_readiness_status(status_from_runtime(&runtime));
            // Filesystem tracing must never extend this transport-mutex lease.
            drop(runtime);
            trace_command_end(
                "get_helper_bridge_status",
                started,
                format!("state={}", result.state),
            );
            result
        }
        Err(TryLockError::WouldBlock) => {
            let result = status_lock_busy();
            trace_command_end(
                "get_helper_bridge_status",
                started,
                "state=checking; helper status is temporarily owned",
            );
            result
        }
        Err(TryLockError::Poisoned(_)) => {
            let result = HelperBridgeStatus {
                state: "error".to_string(),
                message: "Helper bridge status lock is poisoned.".to_string(),
                cuda_ready: false,
                provider_ready: false,
                functional_outbound_ready: false,
                functional_outbound_verified_unix_ms: None,
                degraded_mode: false,
                active_task: None,
                active_request_id: None,
                active_meeting_generation: None,
                active_meeting_session_id: None,
                active_meeting_lane: None,
                generation_token: 0,
                last_error: Some("helper_bridge:lock_poisoned".to_string()),
                stderr_log_path: None,
                updated_unix_ms: unix_ms(),
                runtime_claim: "bridge_state_error".to_string(),
            };
            trace_command_error(
                "get_helper_bridge_status",
                started,
                format!("state={}", result.state),
            );
            result
        }
    }
}

// User/setup worker start must not replace the one Python helper while the
// canonical audio session is active, cleanup-unverified or unavailable.
// This owner gate is required even for direct helper_bridge callers that
// bypass the commands/runtime.rs facade.
fn public_worker_start_has_idle_audio_owner() -> bool {
    let state = latest_runtime_session_state();
    !state.has_active_session
        && state.snapshot.is_none()
        && state.blocker == "runtime_session:no_active_session"
}

// A Live Meeting recovery is the ONLY internal helper restart allowed while
// a Meeting session is active. Recheck its exact generation under the helper
// mutex, after the scheduler wait and again before process creation.
fn worker_restart_generation_allowed(recovery_generation: Option<u64>) -> bool {
    recovery_generation
        .map(live_outbound_generation_is_authoritative)
        .unwrap_or(true)
}

fn helper_start_denied(
    state: &str,
    message: &str,
    claim: &str,
    generation_token: u64,
) -> HelperBridgeActionResult {
    HelperBridgeActionResult {
        ok: false,
        state: state.to_string(),
        message: message.to_string(),
        generation_token,
        runtime_claim: claim.to_string(),
    }
}

fn start_helper_bridge_internal(
    clear_outbound_pipeline: bool,
    recovery_generation: Option<u64>,
) -> HelperBridgeActionResult {
    match runtime().lock() {
        Ok(mut runtime) => {
            // The public caller checked before waiting for the helper mutex.
            // A Meeting/Mic Test/recording may have claimed runtime authority
            // in the interim; do not mutate its worker or outbound admission.
            if clear_outbound_pipeline && !public_worker_start_has_idle_audio_owner() {
                return helper_start_denied(
                    "active_runtime_session",
                    "Finish the active or unverified audio session before starting or restarting the local helper.",
                    "helper_start_denied_runtime_session_not_verified_empty",
                    runtime.generation_token,
                );
            }
            if !worker_restart_generation_allowed(recovery_generation) {
                return helper_start_denied(
                    "stale_generation",
                    "Live Meeting helper recovery was rejected because the original Meeting generation no longer owns authority.",
                    "helper_restart_denied_stale_meeting_generation",
                    runtime.generation_token,
                );
            }
            // Native Meeting selection survives native close after a voice
            // transport fault. Do not restart only Python and re-enter legacy
            // ASR/MT within that same Meeting. Explicit Stop owns release.
            if recovery_generation.is_some() && runtime.native_meeting_session_id().is_some() {
                return helper_start_denied(
                    "cleanup_incomplete",
                    "Native Meeting inference requires verified Stop before another helper startup.",
                    "helper_restart_denied_native_meeting_owner",
                    runtime.generation_token,
                );
            }
            // A no-snapshot Meeting orphan still owns cleanup, even though
            // native audio session authority is now empty. Ordinary public
            // Worker Start/Retry Worker cannot consume that attribution by
            // restarting the process behind Meeting Retry Stop's back.
            if clear_outbound_pipeline
                && runtime.has_meeting_resources()
            {
                return helper_start_denied(
                    "cleanup_incomplete",
                    "The previous Meeting still owns unverified helper resources. Retry Stop Translation before starting another worker.",
                    "helper_start_denied_orphan_meeting_cleanup",
                    runtime.generation_token,
                );
            }
            // A competing Start may have completed while this caller waited.
            // Recheck the selected lifecycle under the single authoritative mutex.
            let native_available = !cfg!(windows) || runtime.native_owner_healthy();
            if runtime.effective_state(native_available) == "ready" {
                return action_result(true, &runtime);
            }
            runtime.generation_token = runtime.generation_token.saturating_add(1);
            invalidate_required_outbound_ai_readiness();
            if !stop_child(&mut runtime) {
                return set_blocked(
                    &mut runtime,
                    "The previous native helper has not finished cleanup. No new owner was started.",
                    "helper_bridge:cleanup_incomplete",
                );
            }
            runtime.active_task = None;
            runtime.active_request_id = None;
            runtime.active_meeting_generation = None;
            runtime.active_meeting_session_id = None;
            runtime.active_meeting_lane = None;

            // stop_child can consume the bounded cleanup budget. Recheck
            // native ownership after it, BEFORE clearing Meeting outbound
            // admission or initializing a replacement native owner.
            if clear_outbound_pipeline && !public_worker_start_has_idle_audio_owner() {
                runtime.state = "stopped".to_string();
                runtime.message = "The native audio owner changed while the old helper was being cleaned up.".to_string();
                runtime.provider_ready = false;
                runtime.cuda_ready = false;
                runtime.updated_unix_ms = unix_ms();
                return helper_start_denied(
                    "active_runtime_session",
                    "Worker restart was rejected because an audio session claimed authority during helper cleanup.",
                    "helper_start_denied_after_cleanup_runtime_owner_change",
                    runtime.generation_token,
                );
            }
            if !worker_restart_generation_allowed(recovery_generation) {
                runtime.state = "stopped".to_string();
                runtime.message = "Live Meeting helper recovery was revoked before native initialization.".to_string();
                runtime.provider_ready = false;
                runtime.cuda_ready = false;
                runtime.updated_unix_ms = unix_ms();
                return helper_start_denied(
                    "stale_generation",
                    "The Meeting generation was revoked during helper cleanup; no replacement worker was started.",
                    "helper_restart_denied_after_cleanup_generation_revoke",
                    runtime.generation_token,
                );
            }
            // Old Meeting attribution is retained through native initialization.
            // A revoked generation during cold import must not leave the
            // orphan unowned or produce a stale Ready worker.
            // The marker is shared with live outbound scheduling; never
            // clear it on a stale/losing helper start admission.
            if clear_outbound_pipeline {
                clear_any_meeting_outbound_pipeline();
            }

            #[cfg(windows)]
            {
                runtime.state = "ready".into();
                runtime.last_error = None;
                let _ = super::helper_bridge_runtime::project_candidate_status(&mut runtime, false);
                action_result(true, &runtime)
            }
            #[cfg(not(windows))]
            {
                set_blocked(&mut runtime, "Native CUDA requires Windows.", "native_inference:platform_unsupported")
            }
        }
        Err(_) => HelperBridgeActionResult {
            ok: false,
            state: "error".to_string(),
            message: "Helper bridge start failed because state lock is poisoned.".to_string(),
            generation_token: 0,
            runtime_claim: "bridge_state_error".to_string(),
        },
    }
}

pub fn start_helper_bridge() -> HelperBridgeActionResult {
    // Unlike the special generation-scoped Live recovery, ordinary worker
    // startup owns an updater operation permit for its entire cold import.
    // It must not start while an update is installing or being downloaded.
    let _update_admission = match admit_app_runtime_operation() {
        Ok(permit) => permit,
        Err(_) => return helper_start_denied(
            "update_installing",
            "A TranslateIT update is in progress. Worker startup remains blocked until installation completes.",
            "helper_start_denied_update_installing",
            0,
        ),
    };
    if !public_worker_start_has_idle_audio_owner() {
        return helper_start_denied(
            "active_runtime_session",
            "Finish the current Meeting, Mic Test or recording cleanup before restarting the local helper.",
            "helper_start_denied_runtime_session_not_verified_empty",
            0,
        );
    }
    // A public retry must not report Ready just because a stale helper
    // process is still alive while the old Meeting owns orphan cleanup.
    if meeting_helper_orphan_cleanup_pending() {
        return helper_start_denied(
            "cleanup_incomplete",
            "Finish the previous Meeting Retry Stop before starting the local helper.",
            "helper_start_denied_orphan_meeting_cleanup",
            0,
        );
    }
    let current = get_helper_bridge_status();
    if current.state == "ready" {
        return HelperBridgeActionResult {
            ok: true,
            state: current.state,
            message: "Native Rust helper lifecycle is already initialized.".into(),
            generation_token: current.generation_token,
            runtime_claim: current.runtime_claim,
        };
    }
    if current.state == "checking" {
        // Never queue a new Start behind an already-owned worker mutex.
        return HelperBridgeActionResult {
            ok: false,
            state: current.state,
            message: "The local AI worker is busy. Its status must be confirmed before starting or restarting it.".to_string(),
            generation_token: current.generation_token,
            runtime_claim: current.runtime_claim,
        };
    }
    start_helper_bridge_internal(true, None)
}

// Read-only orphan indication for the canonical Meeting lifecycle. A busy
// helper mutex is not evidence of an orphan (it may belong to standalone
// translation); Stop and native ExitRequested independently fail closed on
// lock contention. A poisoned owner is never considered clean.
pub(crate) fn meeting_helper_orphan_cleanup_pending() -> bool {
    match runtime().try_lock() {
        Ok(owner) => owner.has_meeting_resources(),
        Err(TryLockError::WouldBlock) => false,
        Err(TryLockError::Poisoned(_)) => true,
    }
}

// Only the no-snapshot Meeting Stop transaction calls this recovery. It
// must never stop a normal standalone/Text helper merely because a process
// exists. Ownership must be an existing Meeting ID on this very runtime,
// AND native runtime session authority must be verifiably idle. The Meeting
// lifecycle gate serializes other Meeting Starts; the helper mutex keeps
// the same worker owner from being replaced while cleanup runs.
pub(crate) fn finish_orphaned_meeting_helper_without_session() -> Result<(), &'static str> {
    if !public_worker_start_has_idle_audio_owner() {
        return Err("helper_orphan:runtime_session_not_verified_idle");
    }
    let mut owner = match runtime().try_lock() {
        Ok(owner) => owner,
        Err(TryLockError::WouldBlock) => return Err("helper_orphan:owner_busy"),
        Err(TryLockError::Poisoned(_)) => return Err("helper_orphan:owner_poisoned"),
    };
    if !public_worker_start_has_idle_audio_owner() {
        return Err("helper_orphan:runtime_owner_changed");
    }

    let meeting_owned = owner.has_meeting_resources();
    if !meeting_owned {
        // Ordinary helper activity or an un-attributed cleanup must not be
        // claimed or killed by Meeting's no-snapshot Stop.
        return Ok(());
    }
    owner.generation_token = owner.generation_token.saturating_add(1);
    invalidate_required_outbound_ai_readiness();
    if !stop_child(&mut owner) {
        set_blocked(
            &mut owner,
            "An orphaned Meeting helper is still releasing its original worker and I/O threads. Retry Stop before closing.",
            "helper_bridge:cleanup_incomplete",
        );
        return Err("helper_orphan:cleanup_incomplete");
    }
    owner.state = "stopped".to_string();
    owner.message = "Orphaned Meeting helper resources were verified released by no-snapshot Retry Stop.".to_string();
    owner.cuda_ready = false;
    owner.provider_ready = false;
    owner.degraded_mode = false;
    owner.active_task = None;
    owner.active_request_id = None;
    owner.active_meeting_generation = None;
    owner.active_meeting_session_id = None;
    owner.active_meeting_lane = None;
    owner.last_error = Some("helper_bridge:orphan_meeting_cleanup_verified".to_string());
    owner.updated_unix_ms = unix_ms();
    // With no Meeting session, the previous outbound marker is stale. Do
    // not clear it until the helper and its exact native idle state were
    // verified; this cannot touch another Meeting while its lifecycle gate
    // excludes concurrent Meeting Start.
    if !public_worker_start_has_idle_audio_owner() {
        return Err("helper_orphan:runtime_owner_changed_after_cleanup");
    }
    // Meeting Stop (not status polling or generic worker restart) has now
    // verified the old helper and native idle ownership for this orphan.
    owner.release_closed_native_meeting_owner();
    owner.pending_meeting_cleanup_session_id = None;
    clear_any_meeting_outbound_pipeline();
    Ok(())
}

pub(crate) fn shutdown_helper_bridge_for_app_exit() -> bool {
    // ExitRequested is a native UI event. Do not block it while a startup
    // ping/AI task owns the helper transport mutex.
    let Ok(mut runtime) = runtime().try_lock() else {
        return false;
    };
    // Even if orphan cleanup could finish here, the app-close path cannot
    // certify Meeting audio/transcript cleanup on the basis of helper process
    // exit alone. The no-snapshot Retry Stop must release its native owner.
    if runtime.has_meeting_resources() {
        return false;
    }

    runtime.generation_token = runtime.generation_token.saturating_add(1);
    invalidate_required_outbound_ai_readiness();
    if !stop_child(&mut runtime) {
        set_blocked(
            &mut runtime,
            "TranslateIT cannot confirm inference owner and I/O cleanup yet. The application remains open so shutdown can be retried safely.",
            "helper_bridge:cleanup_incomplete",
        );
        return false;
    }
    clear_any_meeting_outbound_pipeline();
    runtime.state = "stopped".to_string();
    runtime.message = "Helper worker stopped for application exit.".to_string();
    runtime.cuda_ready = false;
    runtime.provider_ready = false;
    runtime.degraded_mode = false;
    runtime.active_task = None;
    runtime.active_request_id = None;
    runtime.active_meeting_generation = None;
    runtime.active_meeting_session_id = None;
    runtime.active_meeting_lane = None;
    runtime.last_error = None;
    runtime.updated_unix_ms = unix_ms();
    true
}

pub fn cancel_helper_bridge_meeting_session(session_id: &str) -> HelperBridgeActionResult {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return helper_start_denied(
            "invalid_session",
            "Meeting helper cancellation requires the exact non-empty session ID.",
            "helper_cancel_rejected_missing_session_id",
            0,
        );
    }

    // The outbound marker is generation scoped. Even when the helper mutex
    // is owned, Stop may release only its already-revoked session's marker.
    clear_meeting_outbound_pipeline_for_session(session_id);

    // Cold startup holds the helper mutex across ping/status. Waiting here
    // can delay Stop for the entire model import; fail closed and retain the
    // Meeting session for an explicit Retry Stop instead.
    match runtime().try_lock() {
        Ok(mut runtime) => {
            // Resident native work and active helper work may not authorize
            // cleanup of a different Meeting session.
            if (runtime.native_meeting_session_id().is_some_and(|id| id != session_id)
                && (runtime.active_meeting_session_id.as_deref() == Some(session_id)
                    || runtime.pending_meeting_cleanup_session_id.as_deref() == Some(session_id)))
                || (runtime.native_meeting_session_id() == Some(session_id)
                    && runtime.active_meeting_session_id.as_deref().is_some_and(|id| id != session_id))
            {
                return helper_start_denied(
                    "cleanup_owner_unverified",
                    "Native and active helper resources do not belong to the same Meeting session.",
                    "helper_cancel_rejected_native_owner_mismatch",
                    runtime.generation_token,
                );
            }
            let cleanup_unverified = runtime.last_error.as_deref()
                == Some("helper_bridge:cleanup_incomplete")
                || runtime.pending_meeting_cleanup_session_id.is_some();
            if cleanup_unverified {
                // set_blocked and transport have already cleared the active
                // request. Only its retained exact Meeting ID can authorize
                // cleanup of the original worker during Retry Stop.
                if runtime.pending_meeting_cleanup_session_id.as_deref() != Some(session_id) {
                    return helper_start_denied(
                        "cleanup_owner_unverified",
                        "Native helper cleanup is unverified or belongs to a different session. This Stop will not terminate unowned worker resources.",
                        "helper_cancel_rejected_unattributed_or_other_session_cleanup",
                        runtime.generation_token,
                    );
                }
                if !stop_child(&mut runtime) {
                    return set_blocked(
                        &mut runtime,
                        "The same Meeting session still has unverified helper cleanup. Retry Stop after its owned native runtime finishes.",
                        "helper_bridge:cleanup_incomplete",
                    );
                }
                // Only the matching Meeting Stop can release pending
                // attribution after a verified helper resource cleanup.
                runtime.release_closed_native_meeting_owner();
                runtime.pending_meeting_cleanup_session_id = None;
                runtime.state = "stopped".to_string();
                runtime.message = "The same Meeting session's helper cleanup was verified on Retry Stop.".to_string();
                runtime.cuda_ready = false;
                runtime.provider_ready = false;
                runtime.degraded_mode = false;
                runtime.active_task = None;
                runtime.active_request_id = None;
                runtime.active_meeting_generation = None;
                runtime.active_meeting_session_id = None;
                runtime.active_meeting_lane = None;
                runtime.last_error = Some("helper_bridge:cleanup_completed".to_string());
                runtime.updated_unix_ms = unix_ms();
                return action_result(true, &runtime);
            }

            if runtime.active_meeting_session_id.as_deref() == Some(session_id)
                || runtime.native_meeting_session_id() == Some(session_id)
            {
                runtime.generation_token = runtime.generation_token.saturating_add(1);
                invalidate_required_outbound_ai_readiness();
                // stop_child records the exact session before any active
                // request metadata can be cleared on an incomplete cleanup.
                if !stop_child(&mut runtime) {
                    return set_blocked(
                        &mut runtime,
                        "Meeting helper Stop could not verify native owner cleanup. Retry Stop before releasing this session.",
                        "helper_bridge:cleanup_incomplete",
                    );
                }
                // Only the matching Meeting Stop can release pending
                // attribution after a verified helper resource cleanup.
                runtime.release_closed_native_meeting_owner();
                runtime.pending_meeting_cleanup_session_id = None;
                runtime.state = "stopped".to_string();
                runtime.message = format!(
                    "Helper resources for Meeting session {session_id} were released during full Meeting Stop."
                );
                runtime.cuda_ready = false;
                runtime.provider_ready = false;
                runtime.degraded_mode = false;
                runtime.active_task = None;
                runtime.active_request_id = None;
                runtime.active_meeting_generation = None;
                runtime.active_meeting_session_id = None;
                runtime.active_meeting_lane = None;
                runtime.last_error =
                    Some("helper_bridge:meeting_session_hard_cancelled".to_string());
            } else {
                // Never kill an in-flight task owned by a different session.
                // The existing generation/session guards reject queued stale work.
                runtime.message = format!(
                    "No in-flight helper task belongs to Meeting session {session_id}. Queued work remains protected by session and generation guards."
                );
            }
            runtime.updated_unix_ms = unix_ms();
            action_result(true, &runtime)
        }
        Err(TryLockError::WouldBlock) => helper_start_denied(
            "helper_busy",
            "The local inference lifecycle is owned by startup or another request. Meeting Stop retained its session for Retry Stop; helper cleanup is not yet verified.",
            "helper_cancel_deferred_owned_mutex",
            0,
        ),
        Err(TryLockError::Poisoned(_)) => helper_start_denied(
            "error",
            "Meeting helper cancellation failed because native helper state is poisoned.",
            "bridge_state_error",
            0,
        ),
    }
}

// UI capability observation never waits for the inference scheduler.
// An admitted idle refresh may still inspect disk; keep it off the UI thread.
#[tauri::command]
pub async fn helper_bridge_worker_status() -> Result<HelperBridgeWorkerResponse, String> {
    tauri::async_runtime::spawn_blocking(transport::observe_worker_status)
        .await
        .map_err(|_| "Worker status probe was interrupted.".to_string())
}

#[cfg(test)]
mod worker_restart_admission_tests {
    use super::{
        public_worker_start_has_idle_audio_owner, runtime, start_helper_bridge,
        start_helper_bridge_internal, worker_restart_generation_allowed,
    };
    use super::super::helper_bridge_runtime::{stop_child, HelperBridgeRuntime};
    use crate::engine::runtime_state::{
        begin_application_meeting_session, begin_mic_test_session,
        clear_runtime_session_if_generation, clear_runtime_session_state,
        commit_application_meeting_session_live, reserve_app_update_install,
        revoke_runtime_session_authority, runtime_test_guard,
    };

    fn reset_worker() {
        let mut owner = runtime().lock().expect("canonical helper lock");
        assert!(stop_child(&mut owner), "do not discard an unclosed native helper");
        *owner = HelperBridgeRuntime::default();
    }

    #[test]
    fn direct_helper_start_cannot_restart_while_mic_test_owns_audio() {
        let _serial = runtime_test_guard();
        let _ = clear_runtime_session_state();
        reset_worker();

        let claimed = begin_mic_test_session();
        assert!(claimed.blocker.is_empty());
        let generation = claimed.snapshot.expect("mic test owner").generation;
        assert!(!public_worker_start_has_idle_audio_owner());

        // Direct helper API and product intents share this native admission.
        let denied = start_helper_bridge();
        assert!(!denied.ok);
        assert_eq!(denied.state, "active_runtime_session");
        let owner = runtime().lock().expect("helper status after denied start");
        assert_eq!(owner.generation_token, 0);
        drop(owner);

        assert_eq!(
            clear_runtime_session_if_generation(generation).blocker,
            "runtime_session:cleared",
        );
        assert!(public_worker_start_has_idle_audio_owner());
        reset_worker();
    }

    #[test]
    fn update_installer_reservation_blocks_direct_helper_start() {
        let _serial = runtime_test_guard();
        let _ = clear_runtime_session_state();
        reset_worker();

        let update = reserve_app_update_install(|| None).expect("idle update permit");
        let denied = start_helper_bridge();
        assert!(!denied.ok);
        assert_eq!(denied.state, "update_installing");
        drop(update);
        reset_worker();
    }

    #[test]
    fn live_recovery_requires_current_generation_even_inside_helper_lock() {
        let _serial = runtime_test_guard();
        let _ = clear_runtime_session_state();
        reset_worker();

        let denied = start_helper_bridge_internal(false, Some(741));
        assert!(!denied.ok);
        assert_eq!(denied.state, "stale_generation");
        assert_eq!(runtime().lock().expect("helper owner").generation_token, 0);

        let started = begin_application_meeting_session();
        assert!(started.blocker.is_empty());
        let generation = started.snapshot.expect("meeting owner").generation;
        let live = commit_application_meeting_session_live(
            generation, false, "test-only live authority without starting audio",
        );
        assert!(live.blocker.is_empty());
        assert!(worker_restart_generation_allowed(Some(generation)));

        let revoked = revoke_runtime_session_authority(
            generation, "test revoked Live recovery generation",
        );
        assert!(revoked.snapshot.is_some());
        assert!(!worker_restart_generation_allowed(Some(generation)));
        let denied = start_helper_bridge_internal(false, Some(generation));
        assert!(!denied.ok);
        assert_eq!(denied.state, "stale_generation");

        assert_eq!(
            clear_runtime_session_if_generation(generation).blocker,
            "runtime_session:cleared",
        );
        reset_worker();
    }
}

#[cfg(test)]
mod helper_status_lock_contention_tests {
    use super::{get_helper_bridge_status, start_helper_bridge};
    use super::super::helper_bridge_runtime::runtime;

    #[test]
    fn busy_status_never_claims_readiness_or_restarts_an_owned_worker() {
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        let owner = runtime().lock().expect("hold canonical helper owner");
        let original_generation = owner.generation_token;

        // This exact status getter must return without waiting for the held
        // mutex, even while the current owner is starting a Python worker.
        let status = get_helper_bridge_status();
        assert_eq!(status.state, "checking");
        assert_eq!(status.runtime_claim, "helper_bridge:status_lock_busy");
        assert_eq!(status.generation_token, 0);
        assert!(!status.provider_ready);
        assert!(!status.cuda_ready);
        assert!(!status.functional_outbound_ready);
        assert!(status.last_error.is_none());

        let start = start_helper_bridge();
        assert!(!start.ok);
        assert_eq!(start.state, "checking");
        assert_eq!(start.generation_token, 0);
        assert_eq!(owner.generation_token, original_generation);
    }
}

#[cfg(test)]
mod c4_functional_readiness_tests {
    use super::HelperBridgeWorkerResponse;
    use super::functional_readiness::{
        failed_required_outbound_task_invalidates_cache, functional_asr_output,
        functional_translation_output,
    };
    use super::super::helper_bridge_runtime::HelperTaskPriority;

    fn response(ok: bool, body: &str) -> HelperBridgeWorkerResponse {
        HelperBridgeWorkerResponse {
            ok,
            state: "ready".to_string(),
            task: "test".to_string(),
            request_id: "test-request".to_string(),
            scheduler_priority: "meeting_outbound".to_string(),
            message: "test".to_string(),
            generation_token: 7,
            runtime_claim: "test".to_string(),
            worker_response_json: serde_json::from_str(body).expect("valid worker fixture"),
        }
    }

    #[test]
    fn native_voice_requires_typed_pcm_and_exact_receipt() {
        use super::{native_voice_pcm_receipt_matches, NATIVE_INFERENCE_CLAIM};
        let mut receipt = response(true, r#"{"stage":"voice_actor_synthesize","actor_token":"bound",
            "device":"cuda","cuda_pci_id":"0000:01:00.0","sample_rate_hz":44100,
            "model_id":"zonos3-mini-q8_0","sample_count":2}"#);
        receipt.task = "voice_actor_synthesize".into();
        let pcm = [0.25, -0.25];
        assert!(!native_voice_pcm_receipt_matches(&receipt, "bound", Some(&pcm)));
        receipt.runtime_claim = NATIVE_INFERENCE_CLAIM.into();
        assert!(native_voice_pcm_receipt_matches(&receipt, "bound", Some(&pcm)));
        assert!(!native_voice_pcm_receipt_matches(&receipt, "stale", Some(&pcm)));
        assert!(!native_voice_pcm_receipt_matches(&receipt, "bound", None));
        assert!(!native_voice_pcm_receipt_matches(&receipt, "bound", Some(&pcm[..1])));
        receipt.worker_response_json["device"] = serde_json::json!("cpu");
        assert!(!native_voice_pcm_receipt_matches(&receipt, "bound", Some(&pcm)));
    }

    #[test]
    fn structured_worker_response_preserves_legacy_frontend_json_string() {
        let body = r#"{"ok":true,"stage":"status","note":"line\nquote: \"hi\"","nested":{"missing":null}}"#;
        let mut receipt = response(true, body);
        receipt.worker_response_json["scheduler_wait_ms"] = serde_json::json!(7);
        let serialized = serde_json::to_value(&receipt).expect("serialize Tauri receipt");
        let legacy_string = serialized["worker_response_json"].as_str().expect("legacy JSON string");
        let roundtrip: serde_json::Value = serde_json::from_str(legacy_string).unwrap();
        assert_eq!(roundtrip, receipt.worker_response_json);
        assert_eq!(serialized["generation_token"], 7);
        assert_eq!(serialized["ok"], true);
    }

    #[test]
    fn native_model_identity_requires_host_owned_outer_receipt() {
        use super::{meeting_asr_compute_admitted, meeting_translation_compute_admitted,
            NATIVE_INFERENCE_CLAIM};
        let mut asr = response(true, r#"{"device":"cuda","compute_type":"q8_0",
            "model_id":"qwen3-asr-1.7b-q8_0","runtime_claim":"native_cuda_inference_receipt"}"#);
        assert!(!meeting_asr_compute_admitted(&asr));
        asr.runtime_claim = NATIVE_INFERENCE_CLAIM.into();
        assert!(meeting_asr_compute_admitted(&asr));
        asr.worker_response_json["device"] = serde_json::json!("cpu");
        assert!(!meeting_asr_compute_admitted(&asr));

        let mut mt = response(true, r#"{"device":"cuda","precision":"q6_k",
            "model_id":"hy-mt2-1.8b-q6_k","model_revision":"a0c709d9fac510f2c807aa3af52872340dc37a4a",
            "translation_degraded":false,"translation_fallback_reason":""}"#);
        assert!(!meeting_translation_compute_admitted(&mt));
        mt.runtime_claim = NATIVE_INFERENCE_CLAIM.into();
        assert!(meeting_translation_compute_admitted(&mt));
        mt.worker_response_json["model_revision"] = serde_json::json!("other");
        assert!(!meeting_translation_compute_admitted(&mt));
    }

    #[test]
    fn functional_translation_requires_id_en_complete_eos_output() {
        let complete = response(
            true,
            r#"{"ok":true,"direction_pair":"id->en","complete":true,"finished_with_eos":true,"translated_text":"good morning"}"#,
        );
        assert_eq!(
            functional_translation_output(&complete).as_deref(),
            Some("good morning")
        );

        let incomplete = response(
            true,
            r#"{"ok":true,"direction_pair":"id->en","complete":false,"finished_with_eos":false,"translated_text":"partial"}"#,
        );
        assert!(functional_translation_output(&incomplete).is_none());
    }

    #[test]
    fn functional_asr_requires_real_nonempty_transcribe_output() {
        let complete = response(
            true,
            r#"{"ok":true,"stage":"transcribe","transcript_text":"good morning"}"#,
        );
        assert!(functional_asr_output(&complete));

        let empty = response(
            false,
            r#"{"ok":false,"stage":"transcribe","blocker":"asr:empty_transcript"}"#,
        );
        assert!(!functional_asr_output(&empty));
    }

    #[test]
    fn normal_empty_asr_does_not_invalidate_functional_capability() {
        let empty = response(
            false,
            r#"{"ok":false,"stage":"transcribe","blocker":"asr:empty_transcript"}"#,
        );
        assert!(!failed_required_outbound_task_invalidates_cache(
            "transcribe",
            HelperTaskPriority::MeetingOutbound,
            &empty
        ));

        let hard_tts = response(
            false,
            r#"{"ok":false,"stage":"voice_actor_synthesize","blocker":"voice_actor:actor_changed_since_meeting_start"}"#,
        );
        assert!(failed_required_outbound_task_invalidates_cache(
            "voice_actor_synthesize",
            HelperTaskPriority::MeetingOutbound,
            &hard_tts
        ));
    }

    #[test]
    fn optional_incoming_failures_cannot_invalidate_required_outbound_capability() {
        let hard_asr = response(
            false,
            r#"{"ok":false,"stage":"transcribe","blocker":"asr:provider_failed"}"#,
        );
        assert!(!failed_required_outbound_task_invalidates_cache(
            "transcribe",
            HelperTaskPriority::MeetingIncoming,
            &hard_asr
        ));
        assert!(failed_required_outbound_task_invalidates_cache(
            "transcribe",
            HelperTaskPriority::MeetingOutbound,
            &hard_asr
        ));

        let hard_translation = response(
            false,
            r#"{"ok":false,"stage":"translate","blocker":"translation:provider_failed"}"#,
        );
        assert!(!failed_required_outbound_task_invalidates_cache(
            "translate",
            HelperTaskPriority::MeetingIncoming,
            &hard_translation
        ));
        assert!(failed_required_outbound_task_invalidates_cache(
            "translate",
            HelperTaskPriority::MeetingOutbound,
            &hard_translation
        ));

        let optional_preload = response(
            false,
            r#"{"ok":false,"stage":"translation_preload","direction_pair":"en->id","blocker":"translation:provider_failed"}"#,
        );
        assert!(!failed_required_outbound_task_invalidates_cache(
            "translation_preload",
            HelperTaskPriority::Diagnostic,
            &optional_preload
        ));
        let outbound_preload = response(
            false,
            r#"{"ok":false,"stage":"translation_preload","direction_pair":"id->en","blocker":"translation:provider_failed"}"#,
        );
        assert!(failed_required_outbound_task_invalidates_cache(
            "translation_preload",
            HelperTaskPriority::Diagnostic,
            &outbound_preload
        ));
    }


}
