use serde::Serialize;
use serde_json::{json, Value};
use std::io::BufReader;
use std::process::{Command, Stdio};
use std::sync::TryLockError;

use crate::commands::diagnostic_trace::{
    trace_command_end, trace_command_error, trace_command_start,
};
use crate::engine::runtime_state::runtime_generation_is_authoritative;

use super::bridge_paths::{
    helper_stderr_log_path, resolve_worker_python_command, slash_path,
    worker_python_unavailable_message, worker_root, worker_script,
};
use super::helper_bridge_runtime::{
    action_result, apply_worker_status, read_worker_response_direct_with_deadline, runtime,
    set_blocked, spawn_stderr_logger, status_from_runtime, stop_child, unix_ms,
    worker_response_deadline_ms, write_worker_request_direct_with_deadline,
    validate_worker_response_contract, HelperBridgeActionResult,
    HelperBridgeRuntime, HelperBridgeStatus,
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
use request_policy::{
    clear_any_meeting_outbound_pipeline, clear_meeting_outbound_pipeline_for_session,
};

const STARTUP_PING_REQUEST_ID: &str = "helper-startup-ping";
const STARTUP_STATUS_REQUEST_ID: &str = "helper-startup-status";

const REQUIRED_OUTBOUND_FUNCTIONAL_VOICE_OUTPUT: &str =
    "UserData/CacheData/helper_functional_readiness/required_outbound_myvoice.wav";
const REQUIRED_OUTBOUND_DIAGNOSTIC_VOICE_OUTPUT: &str =
    "UserData/CacheData/helper_functional_readiness/diagnostic_myvoice.wav";

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
    pub worker_response_json: String,
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

pub(crate) fn invalidate_required_outbound_readiness_for_voice_change() {
    invalidate_required_outbound_ai_readiness();
}

#[cfg(target_os = "windows")]
pub(crate) fn invalidate_required_outbound_readiness_for_power_transition() {
    invalidate_required_outbound_ai_readiness();
}



pub fn verify_required_outbound_ai_runtime() -> Result<(), &'static str> {
    let (generation_token, actor_token) = run_required_outbound_ai_probe(
        None,
        REQUIRED_OUTBOUND_DIAGNOSTIC_VOICE_OUTPUT,
    )?;
    remember_required_outbound_functional_readiness(generation_token, 0, actor_token);
    Ok(())
}

pub fn prepare_required_outbound_ai_runtime(meeting_generation: u64) -> Result<(), &'static str> {
    let (generation_token, actor_token) = run_required_outbound_ai_probe(
        Some(meeting_generation),
        REQUIRED_OUTBOUND_FUNCTIONAL_VOICE_OUTPUT,
    )?;
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

// The canonical helper mutex owns cold Python ping/status and worker transport.
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
        Ok(mut runtime) => {
            let child_exited = runtime
                .child
                .as_mut()
                .and_then(|child| child.try_wait().ok().flatten())
                .is_some();
            if child_exited {
                invalidate_required_outbound_ai_readiness();
                let clean = stop_child(&mut runtime);
                runtime.state = if clean { "stopped" } else { "blocked" }.to_string();
                runtime.message = if clean {
                    "Helper worker process exited.".to_string()
                } else {
                    "Helper worker exited but process/stderr cleanup is not yet verified. Retry the worker action after cleanup.".to_string()
                };
                runtime.cuda_ready = false;
                runtime.provider_ready = false;
                runtime.degraded_mode = false;
                runtime.last_error = Some(if clean {
                    "helper_bridge:worker_exited"
                } else {
                    "helper_bridge:cleanup_incomplete"
                }.to_string());
                runtime.active_task = None;
                runtime.active_request_id = None;
                runtime.active_meeting_generation = None;
                runtime.active_meeting_session_id = None;
                runtime.active_meeting_lane = None;
                runtime.updated_unix_ms = unix_ms();
            }
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

// A failed startup cannot report its ordinary ping/status error as final
// if the child or stderr reader is still owned. Preserve those handles for
// the next explicit attempt and keep readiness blocked until reaped.
fn fail_start_after_spawn(
    runtime: &mut HelperBridgeRuntime,
    message: &str,
    code: &str,
) -> HelperBridgeActionResult {
    if !stop_child(runtime) {
        return set_blocked(
            runtime,
            "Python helper startup failed and process/stderr cleanup remains unverified. Retry only after cleanup completes.",
            "helper_bridge:cleanup_incomplete",
        );
    }
    set_blocked(runtime, message, code)
}

fn start_helper_bridge_internal(clear_outbound_pipeline: bool) -> HelperBridgeActionResult {
    match runtime().lock() {
        Ok(mut runtime) => {
            // A competing Start may have completed while this caller waited.
            // Recheck process liveness under the single authoritative mutex.
            if runtime.state == "ready"
                && runtime.child.as_mut().is_some_and(|child| matches!(child.try_wait(), Ok(None)))
            {
                return action_result(true, &runtime);
            }
            runtime.generation_token = runtime.generation_token.saturating_add(1);
            invalidate_required_outbound_ai_readiness();
            if !stop_child(&mut runtime) {
                return set_blocked(
                    &mut runtime,
                    "The previous Python helper process or stderr reader has not finished cleanup. No new worker was started.",
                    "helper_bridge:cleanup_incomplete",
                );
            }
            // Clearing the Meeting admission marker is permitted only after
            // the previous owned resources have actually been released.
            if clear_outbound_pipeline {
                clear_any_meeting_outbound_pipeline();
            }
            runtime.active_task = None;
            runtime.active_request_id = None;
            runtime.active_meeting_generation = None;
            runtime.active_meeting_session_id = None;
            runtime.active_meeting_lane = None;

            let worker = worker_script();
            if !worker.is_file() {
                return set_blocked(&mut runtime, "Missing realtime worker script. Restore EngineData/Backend/LocalWorker/WorkerRuntime/realtime_local_worker.py before starting the helper bridge.", "helper_bridge:worker_script_missing");
            }

            let Some(python) = resolve_worker_python_command() else {
                let message = worker_python_unavailable_message();
                return set_blocked(
                    &mut runtime,
                    &message,
                    "helper_bridge:python_runtime_missing",
                );
            };

            runtime.state = "starting".to_string();
            runtime.message = format!("Starting Python helper worker using {}.", python.source);
            runtime.updated_unix_ms = unix_ms();

            let mut command = Command::new(&python.program);
            command.args(&python.bootstrap_args);
            command.arg(&worker);
            command.current_dir(worker_root());
            command.stdin(Stdio::piped());
            command.stdout(Stdio::piped());
            command.stderr(Stdio::piped());

            let child = match command.spawn() {
                Ok(child) => child,
                Err(error) => {
                    return set_blocked(
                        &mut runtime,
                        &format!(
                            "Failed to spawn Python helper worker using {}: {error}",
                            python.source
                        ),
                        "helper_bridge:spawn_failed",
                    )
                }
            };

            // Transfer ownership immediately so even partial pipe setup
            // failures use the same bounded, retryable stop path.
            runtime.child = Some(child);
            let stderr_log_path = helper_stderr_log_path();
            if let Some(stderr) = runtime.child.as_mut().and_then(|owned| owned.stderr.take()) {
                runtime.stderr_logger = Some(spawn_stderr_logger(stderr, stderr_log_path.clone()));
                runtime.stderr_log_path = Some(slash_path(&stderr_log_path));
            }

            let stdin = match runtime.child.as_mut().and_then(|owned| owned.stdin.take()) {
                Some(stdin) => stdin,
                None => {
                    return fail_start_after_spawn(
                        &mut runtime,
                        "Python helper stdin was not available after spawn.",
                        "helper_bridge:stdin_missing",
                    );
                }
            };
            let stdout = match runtime.child.as_mut().and_then(|owned| owned.stdout.take()) {
                Some(stdout) => stdout,
                None => {
                    return fail_start_after_spawn(
                        &mut runtime,
                        "Python helper stdout was not available after spawn.",
                        "helper_bridge:stdout_missing",
                    );
                }
            };
            let stdout = BufReader::new(stdout);

            let ping_deadline_ms = worker_response_deadline_ms("ping");
            // The runtime mutex is already owned during cold bootstrap.
            // Register the one writer before pipe I/O and await its real
            // deadline; a timeout retains the writer for bounded cleanup.
            let stdin = match write_worker_request_direct_with_deadline(
                &mut runtime,
                stdin,
                json!({ "command": "ping", "request_id": STARTUP_PING_REQUEST_ID }),
                ping_deadline_ms,
            ) {
                Ok(stdin) => stdin,
                Err(error) => return fail_start_after_spawn(
                    &mut runtime,
                    &format!("Failed to send ping to helper worker: {error}"),
                    "helper_bridge:ping_write_failed",
                ),
            };
            let (ping, mut stdout) =
                match read_worker_response_direct_with_deadline(&mut runtime, stdout, ping_deadline_ms) {
                    Ok((value, stdout)) => (value, stdout),
                    Err(error) => {
                        return fail_start_after_spawn(
                            &mut runtime,
                            &format!(
                            "Failed to read helper worker ping response before deadline: {error}"
                        ),
                            "helper_bridge:ping_read_failed",
                        );
                    }
                };
            if let Err(error) = validate_worker_response_contract(
                "ping", STARTUP_PING_REQUEST_ID, &ping,
            ) {
                return fail_start_after_spawn(
                    &mut runtime,
                    &format!("Helper startup ping response violated protocol: {error}"),
                    "helper_bridge:ping_response_invalid",
                );
            }
            if ping.get("ok").and_then(Value::as_bool) != Some(true) {
                return fail_start_after_spawn(
                    &mut runtime,
                    "Helper worker ping returned a non-ready response.",
                    "helper_bridge:ping_not_ok",
                );
            }

            let status_deadline_ms = worker_response_deadline_ms("status");
            // A failed or timed-out status request must not silently promote
            // ping-only startup into Ready while stdin is still being written.
            let stdin = match write_worker_request_direct_with_deadline(
                &mut runtime,
                stdin,
                json!({ "command": "status", "request_id": STARTUP_STATUS_REQUEST_ID }),
                status_deadline_ms,
            ) {
                Ok(stdin) => stdin,
                Err(error) => return fail_start_after_spawn(
                    &mut runtime,
                    &format!("Failed to send helper startup status request: {error}"),
                    "helper_bridge:status_write_failed",
                ),
            };
            let status = match read_worker_response_direct_with_deadline(
                &mut runtime, stdout, status_deadline_ms,
            ) {
                Ok((value, next_stdout)) => {
                    if let Err(error) = validate_worker_response_contract(
                        "status", STARTUP_STATUS_REQUEST_ID, &value,
                    ) {
                        return fail_start_after_spawn(
                            &mut runtime,
                            &format!("Helper startup status response violated protocol: {error}"),
                            "helper_bridge:status_response_invalid",
                        );
                    }
                    stdout = next_stdout;
                    value
                }
                Err(error) => return fail_start_after_spawn(
                    &mut runtime,
                    &format!("Failed to read helper worker status response before deadline: {error}"),
                    "helper_bridge:status_read_failed",
                ),
            };

            runtime.stdin = Some(stdin);
            runtime.stdout = Some(stdout);
            runtime.state = "ready".to_string();
            runtime.active_task = None;
            runtime.active_request_id = None;
            runtime.active_meeting_generation = None;
            runtime.active_meeting_session_id = None;
            runtime.active_meeting_lane = None;
            runtime.last_error = None;
            runtime.updated_unix_ms = unix_ms();
            apply_worker_status(&mut runtime, &status);
            action_result(true, &runtime)
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
    let current = get_helper_bridge_status();
    if current.state == "ready" {
        return HelperBridgeActionResult {
            ok: true,
            state: current.state,
            message: "Python helper worker is already running and ready.".to_string(),
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
    start_helper_bridge_internal(true)
}

pub(crate) fn shutdown_helper_bridge_for_app_exit() -> bool {
    // ExitRequested is a native UI event. Do not block it while a startup
    // ping/AI task owns the helper transport mutex.
    let Ok(mut runtime) = runtime().try_lock() else {
        return false;
    };

    runtime.generation_token = runtime.generation_token.saturating_add(1);
    invalidate_required_outbound_ai_readiness();
    if !stop_child(&mut runtime) {
        set_blocked(
            &mut runtime,
            "TranslateIT cannot confirm Python worker and stderr cleanup yet. The application remains open so shutdown can be retried safely.",
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
    clear_meeting_outbound_pipeline_for_session(session_id);
    match runtime().lock() {
        Ok(mut runtime) => {
            // A prior Stop may have timed out while reaping a worker/logger.
            // Clearing active metadata is not proof the resource was released:
            // the next session-scoped Retry Stop must finish that cleanup.
            if runtime.last_error.as_deref() == Some("helper_bridge:cleanup_incomplete") {
                if !stop_child(&mut runtime) {
                    return set_blocked(
                        &mut runtime,
                        "Meeting helper cleanup is still incomplete; Retry Stop after the owned worker/logger exits.",
                        "helper_bridge:cleanup_incomplete",
                    );
                }
                runtime.state = "stopped".to_string();
                runtime.message = "Prior Meeting helper process cleanup was verified after Retry Stop.".to_string();
                runtime.last_error = Some("helper_bridge:cleanup_completed".to_string());
                runtime.updated_unix_ms = unix_ms();
                return action_result(true, &runtime);
            }
            if !session_id.is_empty()
                && runtime.active_meeting_session_id.as_deref() == Some(session_id)
            {
                runtime.generation_token = runtime.generation_token.saturating_add(1);
                invalidate_required_outbound_ai_readiness();
                if !stop_child(&mut runtime) {
                    return set_blocked(
                        &mut runtime,
                        "Meeting helper Stop has not verified Python worker/stderr cleanup. Retry Stop before starting another session.",
                        "helper_bridge:cleanup_incomplete",
                    );
                }
                runtime.state = "stopped".to_string();
                runtime.message = format!(
                    "In-flight helper inference for Meeting session {session_id} was hard-cancelled during full Meeting Stop."
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
                runtime.message = format!(
                    "No in-flight helper task belongs to Meeting session {session_id}. Queued incoming/outbound work will be rejected by session/generation guards."
                );
            }
            runtime.updated_unix_ms = unix_ms();
            action_result(true, &runtime)
        }
        Err(_) => HelperBridgeActionResult {
            ok: false,
            state: "error".to_string(),
            message: "Meeting helper session cancellation failed because state lock is poisoned."
                .to_string(),
            generation_token: 0,
            runtime_claim: "bridge_state_error".to_string(),
        },
    }
}

// Worker status probes can import large AI dependencies. Keep them off
// the native UI event thread while the canonical bridge serializes requests.
#[tauri::command]
pub async fn helper_bridge_worker_status() -> Result<HelperBridgeWorkerResponse, String> {
    tauri::async_runtime::spawn_blocking(|| send_worker_task("status", json!({})))
        .await
        .map_err(|_| "Worker status probe was interrupted.".to_string())
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
    use super::request_policy::live_outbound_stage_retry_safe;
    use super::transport::helper_transport_failure;
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
            worker_response_json: body.to_string(),
        }
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

    #[test]
    fn transport_failure_and_retry_policy_remain_bounded() {
        let write_failure = response(
            false,
            r#"{"ok":false,"blocker":"helper_bridge:translate_write_failed:broken_pipe"}"#,
        );
        assert!(helper_transport_failure(&write_failure));
        let read_failure = response(
            false,
            r#"{"ok":false,"blocker":"helper_bridge:transcribe_read_failed:response_deadline"}"#,
        );
        assert!(helper_transport_failure(&read_failure));
        let model_failure = response(
            false,
            r#"{"ok":false,"blocker":"translation:provider_failed"}"#,
        );
        assert!(!helper_transport_failure(&model_failure));

        assert!(live_outbound_stage_retry_safe("transcribe"));
        assert!(live_outbound_stage_retry_safe("translate"));
        assert!(!live_outbound_stage_retry_safe("voice_actor_synthesize"));
        assert!(!live_outbound_stage_retry_safe("status"));
    }

}
