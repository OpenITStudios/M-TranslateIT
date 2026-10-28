use serde::Serialize;
use serde_json::{json, Value};
use std::io::BufReader;
use std::process::{Command, Stdio};
use std::sync::TryLockError;

use crate::commands::diagnostic_trace::{
    trace_command_end, trace_command_error, trace_command_start,
};
use crate::engine::runtime_state::{
    admit_app_runtime_operation, latest_runtime_session_state,
    runtime_generation_is_authoritative,
};

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
    live_outbound_generation_is_authoritative,
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
                // Reaping an exited Python process is NOT permission to
                // release the previous Meeting's Stop/cleanup attribution.
                // Preserve it until its exact Meeting Stop verifies release.
                let meeting_cleanup_pending =
                    runtime.pending_meeting_cleanup_session_id.is_some();
                runtime.state = if clean && !meeting_cleanup_pending {
                    "stopped"
                } else {
                    "blocked"
                }.to_string();
                runtime.message = if clean && !meeting_cleanup_pending {
                    "Helper worker process exited.".to_string()
                } else if meeting_cleanup_pending {
                    "Helper process exited, but the previous Meeting still requires Retry Stop cleanup verification.".to_string()
                } else {
                    "Helper worker exited but process and I/O cleanup are not yet verified.".to_string()
                };
                runtime.cuda_ready = false;
                runtime.provider_ready = false;
                runtime.degraded_mode = false;
                runtime.last_error = Some(if clean && !meeting_cleanup_pending {
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
            // A no-snapshot Meeting orphan still owns cleanup, even though
            // native audio session authority is now empty. Ordinary public
            // Worker Start/Retry Worker cannot consume that attribution by
            // restarting the process behind Meeting Retry Stop's back.
            if clear_outbound_pipeline
                && (runtime.pending_meeting_cleanup_session_id.is_some()
                    || runtime.active_meeting_session_id.is_some())
            {
                return helper_start_denied(
                    "cleanup_incomplete",
                    "The previous Meeting still owns unverified helper resources. Retry Stop Translation before starting another worker.",
                    "helper_start_denied_orphan_meeting_cleanup",
                    runtime.generation_token,
                );
            }
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
            runtime.active_task = None;
            runtime.active_request_id = None;
            runtime.active_meeting_generation = None;
            runtime.active_meeting_session_id = None;
            runtime.active_meeting_lane = None;

            // stop_child can consume the bounded cleanup budget. Recheck
            // native ownership after it, BEFORE clearing Meeting outbound
            // admission or starting a replacement Python worker.
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
                runtime.message = "Live Meeting helper recovery was revoked before Python startup.".to_string();
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
            // Old Meeting attribution is retained through Python startup.
            // A revoked generation during cold import must not leave the
            // orphan unowned or produce a stale Ready worker.
            // The marker is shared with live outbound scheduling; never
            // clear it on a stale/losing helper start admission.
            if clear_outbound_pipeline {
                clear_any_meeting_outbound_pipeline();
            }

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
                // Thread admission is fallible: preserve the child owner and
                // use the same bounded cleanup as other partial boot failures.
                let logger = match spawn_stderr_logger(stderr, stderr_log_path.clone()) {
                    Ok(logger) => logger,
                    Err(error) => {
                        return fail_start_after_spawn(
                            &mut runtime,
                            &format!("Could not start the Python stderr logger: {error}"),
                            "helper_bridge:stderr_logger_spawn_failed",
                        );
                    }
                };
                runtime.stderr_logger = Some(logger);
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

            // This may have taken many seconds in Python imports. Recheck
            // the original Live generation immediately before publishing
            // Ready or transferring its old pending cleanup attribution.
            if !worker_restart_generation_allowed(recovery_generation) {
                return fail_start_after_spawn(
                    &mut runtime,
                    "Live Meeting generation was revoked while the Python helper was starting; no stale worker readiness was committed.",
                    "helper_bridge:stale_generation_during_startup",
                );
            }
            if recovery_generation.is_some() {
                runtime.pending_meeting_cleanup_session_id = None;
            }

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
    start_helper_bridge_internal(true, None)
}

// Read-only orphan indication for the canonical Meeting lifecycle. A busy
// helper mutex is not evidence of an orphan (it may belong to standalone
// translation); Stop and native ExitRequested independently fail closed on
// lock contention. A poisoned owner is never considered clean.
pub(crate) fn meeting_helper_orphan_cleanup_pending() -> bool {
    match runtime().try_lock() {
        Ok(owner) => owner.pending_meeting_cleanup_session_id.is_some()
            || owner.active_meeting_session_id.is_some(),
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

    let meeting_owned = owner.pending_meeting_cleanup_session_id.is_some()
        || owner.active_meeting_session_id.is_some();
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
    if runtime.pending_meeting_cleanup_session_id.is_some()
        || runtime.active_meeting_session_id.is_some()
    {
        return false;
    }

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
                        "Python helper cleanup is unverified or belongs to a different session. This Stop will not terminate unowned worker resources.",
                        "helper_cancel_rejected_unattributed_or_other_session_cleanup",
                        runtime.generation_token,
                    );
                }
                if !stop_child(&mut runtime) {
                    return set_blocked(
                        &mut runtime,
                        "The same Meeting session still has unverified helper cleanup. Retry Stop after its owned process and I/O threads finish.",
                        "helper_bridge:cleanup_incomplete",
                    );
                }
                // Only the matching Meeting Stop can release pending
                // attribution after a verified helper resource cleanup.
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

            if runtime.active_meeting_session_id.as_deref() == Some(session_id) {
                runtime.generation_token = runtime.generation_token.saturating_add(1);
                invalidate_required_outbound_ai_readiness();
                // stop_child records the exact session before any active
                // request metadata can be cleared on an incomplete cleanup.
                if !stop_child(&mut runtime) {
                    return set_blocked(
                        &mut runtime,
                        "Meeting helper Stop could not verify process/stdin/stdout/stderr cleanup. Retry Stop before releasing this session.",
                        "helper_bridge:cleanup_incomplete",
                    );
                }
                // Only the matching Meeting Stop can release pending
                // attribution after a verified helper resource cleanup.
                runtime.pending_meeting_cleanup_session_id = None;
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
            "The Python helper is owned by startup or another request. Meeting Stop retained its session for Retry Stop; helper cleanup is not yet verified.",
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

// Worker status probes can import large AI dependencies. Keep them off
// the native UI event thread while the canonical bridge serializes requests.
#[tauri::command]
pub async fn helper_bridge_worker_status() -> Result<HelperBridgeWorkerResponse, String> {
    tauri::async_runtime::spawn_blocking(|| send_worker_task("status", json!({})))
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
        assert!(stop_child(&mut owner), "do not discard an unjoined helper");
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
        assert!(owner.child.is_none());
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
        assert!(runtime().lock().expect("helper owner").child.is_none());
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
        assert!(runtime().lock().expect("helper status").child.is_none());

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
