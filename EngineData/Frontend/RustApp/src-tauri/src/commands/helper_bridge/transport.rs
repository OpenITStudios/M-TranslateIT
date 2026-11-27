use serde_json::{json, Value};
use std::cell::Cell;
use std::time::Instant;

use crate::engine::audio::finalized_utterance::FinalizedMeetingUtterance;
use crate::engine::audio::live_segment_writer::{
    finalized_asr_frame_rejection, remove_finalized_meeting_utterance_wav,
    write_finalized_incoming_utterance_wav_with_noise_suppression,
    write_finalized_outbound_utterance_wav_with_noise_suppression,
};

use crate::engine::runtime_state::runtime_generation_is_authoritative;

use super::functional_readiness::{
    failed_required_outbound_task_invalidates_cache, invalidate_required_outbound_ai_readiness,
};
use super::request_policy::{
    clear_meeting_outbound_pipeline, incoming_session_generation, incoming_session_is_eligible,
    inject_request_metadata,
    live_outbound_generation_is_authoritative, live_outbound_stage_retry_safe,
    mark_meeting_outbound_pipeline, meeting_generation, meeting_lane,
    meeting_outbound_pipeline_active, meeting_session_id, task_priority,
};
use super::{
    get_helper_bridge_status, prepare_required_outbound_ai_runtime, start_helper_bridge_internal,
    worker_message, worker_text, HelperBridgeWorkerResponse,
};
use super::super::helper_bridge_runtime::{
    acquire_helper_task_permit, apply_worker_response, clear_active_request, set_blocked,
    await_worker_stdin_write, await_worker_stdout_read,
    begin_worker_stdin_write, begin_worker_stdout_read,
    finish_received_stdin_write, finish_received_stdout_read,
    validate_worker_response_contract,
    runtime, runtime_claim, stop_child, unix_ms,
    worker_response_deadline_for_priority, write_worker_request_with_deadline,
    HelperBridgeActionResult, HelperBridgeRuntime, HelperTaskPriority,
};

pub(super) fn helper_transport_failure(response: &HelperBridgeWorkerResponse) -> bool {
    if response.ok {
        return false;
    }
    let blocker = worker_text(&response.worker_response_json, "blocker").unwrap_or_default();
    blocker.starts_with("helper_bridge:")
        && (blocker.contains("_write_failed:") || blocker.contains("_read_failed:"))
}

fn response_with_runtime(
    ok: bool,
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
    message: String,
    worker_response: Value,
    runtime: &HelperBridgeRuntime,
) -> HelperBridgeWorkerResponse {
    HelperBridgeWorkerResponse {
        ok,
        state: runtime.state.clone(),
        task: task.to_string(),
        request_id: request_id.to_string(),
        scheduler_priority: priority.label().to_string(),
        message,
        generation_token: runtime.generation_token,
        runtime_claim: runtime_claim(runtime),
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

fn incoming_deferred_response(task: &str, request_id: &str) -> HelperBridgeWorkerResponse {
    standalone_blocked_response(
        task,
        request_id,
        HelperTaskPriority::MeetingIncoming,
        "deferred",
        "helper_scheduler:incoming_deferred_for_outbound",
        "Optional incoming Meeting work yielded before execution because required outbound translation currently owns the helper pipeline.",
    )
}

fn blocked_response_from_runtime(
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
    message: &str,
    runtime: &HelperBridgeRuntime,
) -> HelperBridgeWorkerResponse {
    response_with_runtime(
        false,
        task,
        request_id,
        priority,
        message.to_string(),
        json!({
            "ok": false,
            "stage": task,
            "request_id": request_id,
            "scheduler_priority": priority.label(),
            "blocker": runtime
                .last_error
                .clone()
                .unwrap_or_else(|| "helper_bridge:not_ready".to_string()),
            "note": message,
        }),
        runtime,
    )
}

fn stale_meeting_request(
    task: &str,
    request_id: &str,
    priority: HelperTaskPriority,
    generation: Option<u64>,
    session_id: Option<&str>,
    lane: Option<&str>,
) -> Option<HelperBridgeWorkerResponse> {
    if generation
        .map(|value| !runtime_generation_is_authoritative(value))
        .unwrap_or(false)
    {
        return Some(standalone_blocked_response(
            task,
            request_id,
            priority,
            "stale_generation",
            "helper_scheduler:meeting_generation_not_authoritative",
            "Queued outbound Meeting work was discarded because its generation no longer owns output authority.",
        ));
    }

    if lane == Some("incoming")
        && session_id
            .map(|value| !incoming_session_is_eligible(value))
            .unwrap_or(true)
    {
        return Some(standalone_blocked_response(
            task,
            request_id,
            priority,
            "stale_session",
            "helper_scheduler:incoming_meeting_session_not_eligible",
            "Queued incoming Meeting work was discarded because its application Meeting session is no longer eligible for incoming promotion.",
        ));
    }
    None
}

fn recover_incoming_transport_failure_before_permit_release(
    session_id: Option<&str>,
    mut response: HelperBridgeWorkerResponse,
) -> HelperBridgeWorkerResponse {
    if !helper_transport_failure(&response) {
        return response;
    }
    let Some(generation) = session_id.and_then(incoming_session_generation) else {
        return response;
    };

    // This runs while the failed MeetingIncoming request still owns the scheduler
    // permit. Restoring the one shared worker before that permit is released prevents
    // a concurrently waiting required outbound request from observing the worker as
    // stopped. The stale incoming event itself is never retried.
    let recovery = start_helper_bridge_internal(false, Some(generation));
    response.state = recovery.state.clone();
    response.generation_token = recovery.generation_token;
    if recovery.ok {
        response.runtime_claim =
            "meeting_incoming_transport_recovered_same_worker_event_not_retried".to_string();
        response.message = format!(
            "{} The same helper worker was restored before releasing incoming scheduler ownership; this stale incoming event was not retried.",
            response.message
        );
    } else {
        response.runtime_claim = "meeting_incoming_transport_recovery_failed".to_string();
        response.message = format!(
            "{} The helper worker could not be restored after the optional incoming transport failure: {}",
            response.message, recovery.message
        );
    }
    response
}

#[derive(Clone, Copy)]
struct MeetingPcmInput<'a> {
    utterance: &'a FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
    prepare_ms: &'a Cell<u64>,
}

impl MeetingPcmInput<'_> {
    fn matches(&self, task: &str, payload: &Value) -> bool {
        let u = self.utterance;
        task == "transcribe"
            && payload.get("audio_path").is_none()
            && payload.get("meeting_session_id").and_then(Value::as_str) == Some(u.session_id.as_str())
            && payload.get("meeting_lane").and_then(Value::as_str) == Some(u.lane.as_str())
            && payload.get("meeting_sequence").and_then(Value::as_u64) == Some(u.sequence)
            && payload.get("utterance_id").and_then(Value::as_u64) == Some(u.utterance_id)
            && meeting_generation(payload) == u.generation
            && matches!(u.lane.as_str(), "you" | "incoming")
            && ((u.lane == "you") == u.generation.is_some())
    }
}

// Transitional serialization only for an admitted legacy ASR call.
// The existing scoped Stop sweep verifies any file not removed immediately.
struct TemporaryAsrWav(String);
impl Drop for TemporaryAsrWav {
    fn drop(&mut self) { remove_finalized_meeting_utterance_wav(&self.0); }
}

fn send_worker_task_inner(task: &str, mut payload: Value, pcm: Option<MeetingPcmInput<'_>>) -> HelperBridgeWorkerResponse {
    let priority = task_priority(task, &payload);
    let meeting_generation = meeting_generation(&payload);
    let meeting_session_id = meeting_session_id(&payload);
    let meeting_lane = meeting_lane(&payload);
    let permit = match acquire_helper_task_permit(priority) {
        Ok(permit) => permit,
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
    let request_id = permit.request_id().to_string();
    let scheduler_wait_ms = permit.scheduler_wait_ms();
    let response_deadline_ms = worker_response_deadline_for_priority(task, priority);

    // All early exits below occur AFTER this request acquired its permit.
    // Preserve its measured wait even when no valid Python reply exists.
    let response = (|| -> HelperBridgeWorkerResponse {
    // An incoming request may have entered the scheduler before outbound claimed the
    // pipeline. Re-check after permit acquisition so queued optional work cannot slip
    // between required outbound ASR -> translation -> TTS stages.
    if priority == HelperTaskPriority::MeetingIncoming && meeting_outbound_pipeline_active() {
        return incoming_deferred_response(task, &request_id);
    }

    if let Some(response) = stale_meeting_request(
        task,
        &request_id,
        priority,
        meeting_generation,
        meeting_session_id.as_deref(),
        meeting_lane.as_deref(),
    ) {
        return response;
    }

    // Work rejected or deferred at admission does not create a WAV.
    // Native cutover will consume this same borrowed PCM at this boundary.
    let _temporary_asr_wav = if let Some(input) = pcm {
        let started = Instant::now();
        let write = if input.utterance.lane == "you" {
            write_finalized_outbound_utterance_wav_with_noise_suppression(
                input.utterance, input.noise_suppression_enabled)
        } else {
            write_finalized_incoming_utterance_wav_with_noise_suppression(
                input.utterance, input.noise_suppression_enabled)
        };
        input.prepare_ms.set(input.prepare_ms.get().saturating_add(observed_elapsed_ms(started)));
        if !write.ok {
            return standalone_blocked_response(
                task, &request_id, priority, "error", &write.blocker, &write.note,
            );
        }
        let Some(path) = write.audio_path else {
            return standalone_blocked_response(
                task, &request_id, priority, "error", "asr:finalized_audio_path_missing",
                "Finalized audio writer did not return an owned path.",
            );
        };
        let owned = TemporaryAsrWav(path);
        payload["audio_path"] = json!(&owned.0);
        // An outbound claim may arrive while preparing the legacy WAV.
        // Yield again before worker I/O; the scoped file is removed on return.
        if priority == HelperTaskPriority::MeetingIncoming && meeting_outbound_pipeline_active() {
            return incoming_deferred_response(task, &request_id);
        }
        if let Some(response) = stale_meeting_request(
            task, &request_id, priority, meeting_generation,
            meeting_session_id.as_deref(), meeting_lane.as_deref(),
        ) {
            return response;
        }
        Some(owned)
    } else {
        None
    };

    inject_request_metadata(&mut payload, task, &request_id, priority);

    let (stdin_receiver, stdout_receiver, bridge_generation) = match runtime().lock() {
        Ok(mut runtime) => {
            if runtime.child.is_none() || runtime.stdin.is_none() || runtime.stdout.is_none() {
                // A timed-out kill/stderr reader still owns resources even
                // after its request pipes are removed. Do not downgrade that
                // verified cleanup failure to an ordinary Stopped state.
                if runtime.last_error.as_deref() == Some("helper_bridge:cleanup_incomplete") {
                    return blocked_response_from_runtime(
                        task,
                        &request_id,
                        priority,
                        "Python helper cleanup is incomplete. Retry Stop before another worker request.",
                        &runtime,
                    );
                }
                runtime.state = "stopped".to_string();
                runtime.message = "Helper worker is not running. Start Helper first.".to_string();
                runtime.last_error = Some("helper_bridge:not_running".to_string());
                runtime.updated_unix_ms = unix_ms();
                return blocked_response_from_runtime(
                    task,
                    &request_id,
                    priority,
                    "Helper worker is not running. Start Helper first.",
                    &runtime,
                );
            }

            // Publish the reader JoinHandle inside the canonical mutex before
            // any blocking write or read. Meeting Stop can now find/reap that
            // exact stdout owner even while this task waits outside the lock.
            // An earlier timed-out reader must not be overwritten.
            if runtime.stdin_writer.is_some() || runtime.stdout_reader.is_some() {
                return blocked_response_from_runtime(
                    task,
                    &request_id,
                    priority,
                    "A previous Python stdin writer or stdout reader is still owned. Complete helper cleanup before another request.",
                    &runtime,
                );
            }
            let stdin = runtime.stdin.take().expect("stdin checked above");
            let stdout = runtime.stdout.take().expect("stdout checked above");
            let stdout_receiver = match begin_worker_stdout_read(&mut runtime, stdout) {
                Ok(receiver) => receiver,
                Err(error) => {
                    let cleaned = stop_child(&mut runtime);
                    runtime.generation_token = runtime.generation_token.saturating_add(1);
                    let blocker = if cleaned {
                        "helper_bridge:stdout_reader_admission_failed"
                    } else {
                        "helper_bridge:cleanup_incomplete"
                    };
                    let message = format!(
                        "Could not register the Python stdout read owner safely: {error}"
                    );
                    set_blocked(&mut runtime, &message, blocker);
                    return blocked_response_from_runtime(
                        task, &request_id, priority, &message, &runtime,
                    );
                }
            };
            // Register stdin ownership under this exact same native mutex
            // before releasing it. Stop can now find a backpressured writer
            // while this task waits on the original response deadline.
            let stdin_receiver = match begin_worker_stdin_write(
                &mut runtime, stdin, payload, response_deadline_ms,
            ) {
                Ok(receiver) => receiver,
                Err(error) => {
                    let cleaned = stop_child(&mut runtime);
                    runtime.generation_token = runtime.generation_token.saturating_add(1);
                    let blocker = if cleaned {
                        "helper_bridge:stdin_writer_admission_failed"
                    } else {
                        "helper_bridge:cleanup_incomplete"
                    };
                    let message = format!(
                        "Could not register the Python stdin write owner safely: {error}"
                    );
                    set_blocked(&mut runtime, &message, blocker);
                    return blocked_response_from_runtime(
                        task, &request_id, priority, &message, &runtime,
                    );
                }
            };
            let bridge_generation = runtime.generation_token;
            runtime.active_task = Some(task.to_string());
            runtime.active_request_id = Some(request_id.clone());
            runtime.active_meeting_generation = meeting_generation;
            runtime.active_meeting_session_id = meeting_session_id.clone();
            runtime.active_meeting_lane = meeting_lane.clone();
            runtime.updated_unix_ms = unix_ms();
            (stdin_receiver, stdout_receiver, bridge_generation)
        }
        Err(_) => {
            return standalone_blocked_response(
                task,
                &request_id,
                priority,
                "error",
                "helper_bridge:lock_poisoned",
                "Helper bridge state lock is poisoned.",
            )
        }
    };

    let stdin = match await_worker_stdin_write(stdin_receiver, response_deadline_ms) {
        Ok(stdin) => stdin,
        Err(error) => {
            let response = match runtime().lock() {
                Ok(mut runtime) => {
                    if runtime.generation_token == bridge_generation {
                        let cleaned = stop_child(&mut runtime);
                        runtime.generation_token = runtime.generation_token.saturating_add(1);
                        runtime.state = if cleaned { "stopped" } else { "blocked" }.to_string();
                        runtime.message = if cleaned {
                            format!("Failed to write {task} request to Python helper worker: {error}")
                        } else {
                            format!("Failed to write {task} request, and Python helper cleanup remains incomplete: {error}")
                        };
                        runtime.last_error = Some(if cleaned {
                            format!("helper_bridge:{task}_write_failed:{error}")
                        } else {
                            "helper_bridge:cleanup_incomplete".to_string()
                        });
                        clear_active_request(&mut runtime, &request_id);
                        runtime.provider_ready = false;
                        runtime.cuda_ready = false;
                        runtime.updated_unix_ms = unix_ms();
                    }
                    let message = runtime.message.clone();
                    blocked_response_from_runtime(task, &request_id, priority, &message, &runtime)
                }
                Err(_) => standalone_blocked_response(
                    task,
                    &request_id,
                    priority,
                    "error",
                    "helper_bridge:lock_poisoned_after_write_failure",
                    "Helper request write failed and bridge state could not be recovered.",
                ),
            };
            return if priority == HelperTaskPriority::MeetingIncoming {
                recover_incoming_transport_failure_before_permit_release(
                    meeting_session_id.as_deref(),
                    response,
                )
            } else {
                response
            };
        }
    };

    let (mut worker_response, stdout) = match await_worker_stdout_read(
        stdout_receiver,
        response_deadline_ms,
    )
    .and_then(|(response, stdout)| {
        validate_worker_response_contract(task, &request_id, &response)?;
        Ok((response, stdout))
    }) {
        Ok(value) => value,
        Err(error) => {
            let response = match runtime().lock() {
                Ok(mut runtime) => {
                    if runtime.generation_token == bridge_generation {
                        let cleaned = stop_child(&mut runtime);
                        runtime.generation_token = runtime.generation_token.saturating_add(1);
                        runtime.state = if cleaned { "stopped" } else { "blocked" }.to_string();
                        runtime.message = if cleaned {
                            format!(
                                "Failed to read {task} response from Python helper worker before deadline: {error}"
                            )
                        } else {
                            format!(
                                "Failed to read {task} response, and Python helper cleanup remains incomplete: {error}"
                            )
                        };
                        runtime.last_error = Some(if cleaned {
                            format!("helper_bridge:{task}_read_failed:{error}")
                        } else {
                            "helper_bridge:cleanup_incomplete".to_string()
                        });
                        clear_active_request(&mut runtime, &request_id);
                        runtime.provider_ready = false;
                        runtime.cuda_ready = false;
                        runtime.updated_unix_ms = unix_ms();
                    }
                    let message = runtime.message.clone();
                    blocked_response_from_runtime(task, &request_id, priority, &message, &runtime)
                }
                Err(_) => standalone_blocked_response(
                    task,
                    &request_id,
                    priority,
                    "error",
                    "helper_bridge:lock_poisoned_after_read_failure",
                    "Helper response failed and bridge state could not be recovered safely.",
                ),
            };
            return if priority == HelperTaskPriority::MeetingIncoming {
                recover_incoming_transport_failure_before_permit_release(
                    meeting_session_id.as_deref(),
                    response,
                )
            } else {
                response
            };
        }
    };

    if let Some(object) = worker_response.as_object_mut() {
        object.insert("request_id".to_string(), json!(request_id));
        object.insert("scheduler_priority".to_string(), json!(priority.label()));
        // Rust-measured admission delay, not model inference time or worker-supplied truth.
        object.insert("scheduler_wait_ms".to_string(), json!(scheduler_wait_ms));
    }

    match runtime().lock() {
        Ok(mut runtime) => {
            let bridge_still_owns_process =
                runtime.generation_token == bridge_generation && runtime.child.is_some();
            if !bridge_still_owns_process {
                clear_active_request(&mut runtime, &request_id);
                return blocked_response_from_runtime(
                    task,
                    &request_id,
                    priority,
                    "Helper result was discarded because the worker process was cancelled or replaced while the request was running.",
                    &runtime,
                );
            }

            // Both channels have returned their original owned pipes.
            // Join the registered writer AND reader before exposing either
            // pipe for another request. A concurrent Stop wins the generation
            // check above and prevents stale completion from resurrecting them.
            let writer_joined = finish_received_stdin_write(&mut runtime);
            let reader_joined = finish_received_stdout_read(&mut runtime);
            if !writer_joined || !reader_joined {
                let cleaned = stop_child(&mut runtime);
                runtime.generation_token = runtime.generation_token.saturating_add(1);
                let blocker = if !cleaned {
                    "helper_bridge:cleanup_incomplete"
                } else if !writer_joined {
                    "helper_bridge:stdin_writer_join_failed"
                } else {
                    "helper_bridge:stdout_reader_join_failed"
                };
                let message = "Python worker I/O thread completion could not be verified; the response was not committed.";
                set_blocked(&mut runtime, message, blocker);
                return blocked_response_from_runtime(
                    task, &request_id, priority, message, &runtime,
                );
            }

            runtime.stdin = Some(stdin);
            runtime.stdout = Some(stdout);

            if let Some(response) = stale_meeting_request(
                task,
                &request_id,
                priority,
                meeting_generation,
                meeting_session_id.as_deref(),
                meeting_lane.as_deref(),
            ) {
                clear_active_request(&mut runtime, &request_id);
                runtime.message = response.message.clone();
                runtime.updated_unix_ms = unix_ms();
                return response_with_runtime(
                    false,
                    task,
                    &request_id,
                    priority,
                    response.message,
                    response.worker_response_json,
                    &runtime,
                );
            }

            let ok = apply_worker_response(&mut runtime, &worker_response);
            clear_active_request(&mut runtime, &request_id);
            runtime.updated_unix_ms = unix_ms();
            let message = worker_message(task, &worker_response);
            response_with_runtime(
                ok,
                task,
                &request_id,
                priority,
                message,
                worker_response,
                &runtime,
            )
        }
        Err(_) => standalone_blocked_response(
            task,
            &request_id,
            priority,
            "error",
            "helper_bridge:lock_poisoned_after_worker_response",
            "Worker response was received, but bridge state could not be updated safely.",
        ),
    }
    })();
    attach_admitted_wait_to_failure(response, scheduler_wait_ms)
}

fn recover_live_meeting_helper_transport(
    generation: u64,
    retain_outbound_pipeline: bool,
) -> HelperBridgeActionResult {
    if !live_outbound_generation_is_authoritative(generation) {
        return HelperBridgeActionResult {
            ok: false,
            state: "stale_generation".to_string(),
            message: "Live Meeting helper recovery was skipped because the outbound generation no longer owns Live authority."
                .to_string(),
            generation_token: get_helper_bridge_status().generation_token,
            runtime_claim: "meeting_live_helper_recovery_skipped_stale_generation".to_string(),
        };
    }

    let _permit = match acquire_helper_task_permit(HelperTaskPriority::MeetingOutbound) {
        Ok(permit) => permit,
        Err(error) => {
            return HelperBridgeActionResult {
                ok: false,
                state: "error".to_string(),
                message: format!(
                    "Live Meeting helper recovery could not acquire outbound scheduler priority: {error}"
                ),
                generation_token: get_helper_bridge_status().generation_token,
                runtime_claim: "meeting_live_helper_recovery_scheduler_unavailable".to_string(),
            }
        }
    };

    if !live_outbound_generation_is_authoritative(generation) {
        return HelperBridgeActionResult {
            ok: false,
            state: "stale_generation".to_string(),
            message: "Live Meeting helper recovery stopped before restart because the outbound generation was revoked while waiting for the scheduler."
                .to_string(),
            generation_token: get_helper_bridge_status().generation_token,
            runtime_claim: "meeting_live_helper_recovery_skipped_after_scheduler_wait".to_string(),
        };
    }

    // The scheduler preflight is not atomic with helper mutex acquisition.
    // Pin the Live Meeting generation again under the helper mutex before
    // any restart side effects, not only at this caller's earlier checks.
    let mut recovery = start_helper_bridge_internal(false, Some(generation));
    if recovery.ok && live_outbound_generation_is_authoritative(generation) {
        if retain_outbound_pipeline {
            mark_meeting_outbound_pipeline(generation);
        }
        recovery.message = "The same canonical helper worker was restarted after a Live Meeting transport failure."
            .to_string();
        recovery.runtime_claim = "meeting_live_helper_transport_recovered_same_worker".to_string();
        return recovery;
    }

    clear_meeting_outbound_pipeline(generation);
    if recovery.ok {
        recovery.ok = false;
        recovery.state = "stale_generation".to_string();
        recovery.message = "The helper worker restarted, but the Meeting generation was revoked before the failed stage could be retried."
            .to_string();
        recovery.runtime_claim =
            "meeting_live_helper_recovery_completed_after_generation_revoke".to_string();
    }
    recovery
}

fn retain_payload_for_live_retry(
    task: &str,
    priority: HelperTaskPriority,
    outbound_generation: Option<u64>,
) -> bool {
    priority == HelperTaskPriority::MeetingOutbound
        && outbound_generation.is_some()
        && live_outbound_stage_retry_safe(task)
}

// One request's observed wall-clock partitions. Attempts include their own
// permit waits, but exclude separately measured PCM preparation. Recovery
// includes the worker restart and functional re-proof.
// This is transient measurement, not another scheduler, retry owner, or cache.
#[derive(Clone, Copy)]
struct WorkerAttemptObservation {
    initial_attempt_ms: u64,
    recovery_ms: Option<u64>,
    retry_attempt_ms: Option<u64>,
    initial_scheduler_wait_ms: Option<u64>,
    retry_scheduler_wait_ms: Option<u64>,
}

impl WorkerAttemptObservation {
    fn scheduler_wait_total_ms(&self) -> Option<u64> {
        if self.retry_attempt_ms.is_some() {
            self.initial_scheduler_wait_ms?
                .checked_add(self.retry_scheduler_wait_ms?)
        } else {
            self.initial_scheduler_wait_ms
        }
    }
}

fn observed_elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn response_scheduler_wait_ms(response: &HelperBridgeWorkerResponse) -> Option<u64> {
    response.worker_response_json.get("scheduler_wait_ms").and_then(Value::as_u64)
}

// Rust owns these measurements; never trust a worker-provided summary and
// never turn missing/invalid response data into invented numeric evidence.
fn add_worker_attempt_observation(response: &mut HelperBridgeWorkerResponse, timing: WorkerAttemptObservation) {
    let Some(object) = response.worker_response_json.as_object_mut() else { return };
    object.insert("worker_attempt_timing".to_string(), json!({
        "initial_attempt_ms": timing.initial_attempt_ms,
        "recovery_ms": timing.recovery_ms,
        "retry_attempt_ms": timing.retry_attempt_ms,
        "initial_scheduler_wait_ms": timing.initial_scheduler_wait_ms,
        "retry_scheduler_wait_ms": timing.retry_scheduler_wait_ms,
        "scheduler_wait_total_ms": timing.scheduler_wait_total_ms(),
    }));
}

pub(super) fn send_worker_task(task: &str, payload: Value) -> HelperBridgeWorkerResponse {
    send_worker_task_with_input(task, payload, None)
}

pub(super) fn send_worker_pcm_task(
    payload: Value,
    utterance: &FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
) -> HelperBridgeWorkerResponse {
    let prepare_ms = Cell::new(0);
    let input = MeetingPcmInput { utterance, noise_suppression_enabled, prepare_ms: &prepare_ms };
    let mut response = if !input.matches("transcribe", &payload) {
        standalone_blocked_response(
            "transcribe", "invalid-meeting-pcm", task_priority("transcribe", &payload),
            "error", "asr:finalized_pcm_contract_invalid",
            "Finalized PCM Meeting event ownership is invalid.",
        )
    } else if let Some(rejection) = finalized_asr_frame_rejection(utterance) {
        standalone_blocked_response(
            "transcribe", "invalid-meeting-pcm", task_priority("transcribe", &payload),
            "error", &rejection.blocker, &rejection.note,
        )
    } else {
        send_worker_task_with_input("transcribe", payload, Some(input))
    };
    if let Some(object) = response.worker_response_json.as_object_mut() {
        object.insert("audio_prepare_ms".into(), json!(prepare_ms.get()));
    }
    response
}

fn send_worker_task_with_input(task: &str, payload: Value, pcm: Option<MeetingPcmInput<'_>>) -> HelperBridgeWorkerResponse {
    let priority = task_priority(task, &payload);
    let outbound_generation = if priority == HelperTaskPriority::MeetingOutbound
        && meeting_lane(&payload).as_deref() == Some("you")
    {
        meeting_generation(&payload)
    } else {
        None
    };
    let mut retry_payload = retain_payload_for_live_retry(task, priority, outbound_generation)
        .then(|| payload.clone());

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
    let mut response = send_worker_task_inner(task, payload, pcm);
    let mut timing = WorkerAttemptObservation {
        initial_attempt_ms: observed_elapsed_ms(first_started)
            .saturating_sub(pcm.map_or(0, |input| input.prepare_ms.get())),
        recovery_ms: None,
        retry_attempt_ms: None,
        initial_scheduler_wait_ms: response_scheduler_wait_ms(&response),
        retry_scheduler_wait_ms: None,
    };

    if let Some(generation) = outbound_generation {
        if helper_transport_failure(&response)
            && live_outbound_generation_is_authoritative(generation)
        {
            let retry_current_stage = live_outbound_stage_retry_safe(task);
            let recovery_started = Instant::now();
            let recovery = recover_live_meeting_helper_transport(generation, retry_current_stage);
            if recovery.ok && live_outbound_generation_is_authoritative(generation) {
                // Restarting the worker intentionally invalidates functional readiness
                // and the generation-bound actor token. Re-prove the exact required
                // outbound path before retrying a side-effect-safe stage or claiming
                // the recovered worker is usable for later utterances.
                let functional_reproof = prepare_required_outbound_ai_runtime(generation);
                // Re-proof is the final recovery step. Retry has its own
                // separately measured wall interval and must not inflate
                // recovery_ms or be subtracted twice in Meeting Diagnostics.
                timing.recovery_ms = Some(observed_elapsed_ms(recovery_started));
                match functional_reproof {
                    Ok(()) if retry_current_stage => {
                        // Only retry-safe Live outbound stages retain a payload clone.
                        // ASR and translation have no Meeting playback side effect, so
                        // the same finalized input may be attempted exactly once after
                        // transport recovery + generation-bound functional re-proof.
                        // The retry itself is not recursive.
                        if let Some(retry_payload) = retry_payload.take() {
                            let retry_started = Instant::now();
                            let prepared_before_retry = pcm.map_or(0, |input| input.prepare_ms.get());
                            response = send_worker_task_inner(task, retry_payload, pcm);
                            let retry_prepare_ms = pcm.map_or(0, |input| input.prepare_ms.get())
                                .saturating_sub(prepared_before_retry);
                            timing.retry_attempt_ms = Some(observed_elapsed_ms(retry_started)
                                .saturating_sub(retry_prepare_ms));
                            timing.retry_scheduler_wait_ms = response_scheduler_wait_ms(&response);
                        } else {
                            clear_meeting_outbound_pipeline(generation);
                            response.ok = false;
                            response.state = "error".to_string();
                            response.runtime_claim =
                                "meeting_live_helper_retry_payload_contract_missing".to_string();
                            response.message =
                                "Live Meeting helper recovery completed, but the retry-safe payload contract was unavailable."
                                    .to_string();
                        }
                    }
                    Ok(()) => {
                        // Synthesis can have uncertain child-process/file state after a
                        // transport failure. The worker and actor binding are restored
                        // for subsequent utterances, but this phrase is not synthesized
                        // again automatically.
                        response.state = recovery.state;
                        response.generation_token = recovery.generation_token;
                        response.runtime_claim =
                            "meeting_live_helper_recovered_current_stage_not_retried".to_string();
                        response.message = format!(
                            "{} The helper worker and Meeting voice authority were restored for subsequent utterances; this synthesis stage was not retried.",
                            response.message
                        );
                    }
                    Err(stage) => {
                        clear_meeting_outbound_pipeline(generation);
                        let current = get_helper_bridge_status();
                        response.state = current.state;
                        response.generation_token = current.generation_token;
                        response.runtime_claim =
                            "meeting_live_helper_recovery_functional_reproof_failed".to_string();
                        response.message = format!(
                            "{} The helper process restarted, but required outbound functional re-proof failed at {stage}. Stop and start Translation before producing more voice output.",
                            response.message
                        );
                    }
                }
            }
            // A failed restart or revoked generation skips functional re-proof.
            // Preserve the failed recovery interval, but do not invent retry
            // work or overwrite the already-closed successful recovery timer.
            if timing.recovery_ms.is_none() {
                timing.recovery_ms = Some(observed_elapsed_ms(recovery_started));
            }
        }

        if task == "voice_actor_synthesize"
            || !response.ok
            || !runtime_generation_is_authoritative(generation)
        {
            clear_meeting_outbound_pipeline(generation);
        }
    }

    if failed_required_outbound_task_invalidates_cache(task, priority, &response) {
        invalidate_required_outbound_ai_readiness();
    }
    if outbound_generation.is_some() {
        add_worker_attempt_observation(&mut response, timing);
    }
    response
}

#[cfg(test)]
mod request_payload_retention_tests {
    use super::retain_payload_for_live_retry;
    use super::super::super::helper_bridge_runtime::HelperTaskPriority;

    #[test]
    fn only_retry_safe_live_outbound_stages_retain_payload_clone() {
        assert!(retain_payload_for_live_retry(
            "transcribe",
            HelperTaskPriority::MeetingOutbound,
            Some(7),
        ));
        assert!(retain_payload_for_live_retry(
            "translate",
            HelperTaskPriority::MeetingOutbound,
            Some(7),
        ));

        assert!(!retain_payload_for_live_retry(
            "voice_actor_synthesize",
            HelperTaskPriority::MeetingOutbound,
            Some(7),
        ));
        assert!(!retain_payload_for_live_retry(
            "status",
            HelperTaskPriority::MeetingOutbound,
            Some(7),
        ));
        assert!(!retain_payload_for_live_retry(
            "translate",
            HelperTaskPriority::Text,
            None,
        ));
        assert!(!retain_payload_for_live_retry(
            "transcribe",
            HelperTaskPriority::MeetingIncoming,
            None,
        ));
    }
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
        standalone_blocked_response, WorkerAttemptObservation};
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
    fn first_attempt_without_recovery_preserves_original_wait() {
        let mut response = fake_response(Some(13));
        add_worker_attempt_observation(&mut response, WorkerAttemptObservation {
            initial_attempt_ms: 80, recovery_ms: None, retry_attempt_ms: None,
            initial_scheduler_wait_ms: response_scheduler_wait_ms(&fake_response(Some(13))),
            retry_scheduler_wait_ms: None,
        });
        let v = &response.worker_response_json;
        assert_eq!(v["scheduler_wait_ms"], 13);
        assert_eq!(v["worker_attempt_timing"]["initial_attempt_ms"], 80);
        assert_eq!(v["worker_attempt_timing"]["scheduler_wait_total_ms"], 13);
        assert!(v["worker_attempt_timing"]["recovery_ms"].is_null());
    }

    #[test]
    fn recovery_retry_waits_sum_but_attempt_wall_times_stay_distinct() {
        let mut response = fake_response(Some(7));
        add_worker_attempt_observation(&mut response, WorkerAttemptObservation {
            initial_attempt_ms: 140, recovery_ms: Some(250), retry_attempt_ms: Some(110),
            initial_scheduler_wait_ms: Some(18), retry_scheduler_wait_ms: Some(7),
        });
        let v = &response.worker_response_json;
        assert_eq!(v["scheduler_wait_ms"], 7);
        let timing = &v["worker_attempt_timing"];
        assert_eq!(timing["scheduler_wait_total_ms"], 25);
        assert_eq!(timing["initial_attempt_ms"], 140);
        assert_eq!(timing["recovery_ms"], 250);
        assert_eq!(timing["retry_attempt_ms"], 110);
    }

    #[test]
    fn incomplete_retry_wait_and_unadmitted_attempt_never_claim_total_wait() {
        let mut response = fake_response(None);
        add_worker_attempt_observation(&mut response, WorkerAttemptObservation {
            initial_attempt_ms: 140, recovery_ms: Some(250), retry_attempt_ms: Some(110),
            initial_scheduler_wait_ms: Some(18), retry_scheduler_wait_ms: None,
        });
        let v = &response.worker_response_json;
        assert!(v["worker_attempt_timing"]["scheduler_wait_total_ms"].is_null());
        let no_permit = WorkerAttemptObservation {
            initial_attempt_ms: 1, recovery_ms: None, retry_attempt_ms: None,
            initial_scheduler_wait_ms: None, retry_scheduler_wait_ms: None,
        };
        assert_eq!(no_permit.scheduler_wait_total_ms(), None);
    }

    #[test]
    fn malformed_response_does_not_become_a_trusted_measurement() {
        let mut response = fake_response(None);
        response.worker_response_json = json!("non-object");
        add_worker_attempt_observation(&mut response, WorkerAttemptObservation {
            initial_attempt_ms: 1, recovery_ms: None, retry_attempt_ms: None,
            initial_scheduler_wait_ms: None, retry_scheduler_wait_ms: None,
        });
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
        let mut utterance = FinalizedMeetingUtterance {
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
        let input = MeetingPcmInput { utterance: &utterance, noise_suppression_enabled: false, prepare_ms: &prepare_ms };
        assert!(input.matches("transcribe", &payload));
        assert!(finalized_asr_frame_rejection(&utterance).is_none());
        for (key, value) in [
            ("meeting_session_id", json!("another")),
            ("meeting_lane", json!("incoming")),
            ("meeting_generation", json!(6)),
            ("meeting_sequence", json!(10)),
            ("utterance_id", json!(9)),
            ("audio_path", json!("unowned.wav")),
        ] {
            let mut invalid = payload.clone();
            invalid[key] = value;
            assert!(!input.matches("transcribe", &invalid), "{key}");
        }
        assert!(!input.matches("translate", &payload));
        utterance.frame.samples.truncate(4_799);
        assert_eq!(finalized_asr_frame_rejection(&utterance).unwrap().blocker,
                   "finalized_utterance_writer:segment_too_short");
    }
}
