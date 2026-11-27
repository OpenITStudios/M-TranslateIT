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
    clear_meeting_outbound_pipeline, incoming_session_is_eligible,
    inject_request_metadata,
    mark_meeting_outbound_pipeline, meeting_generation, meeting_lane,
    meeting_outbound_pipeline_active, meeting_session_id, task_priority,
};
use super::{
    incoming_deferred_for_required_outbound,
    worker_message, HelperBridgeWorkerResponse,
};
use super::super::helper_bridge_runtime::{
    acquire_helper_task_permit, apply_worker_response, clear_active_request, set_blocked,
    await_worker_stdin_write, await_worker_stdout_read,
    begin_worker_stdin_write, begin_worker_stdout_read,
    finish_received_stdin_write, finish_received_stdout_read,
    validate_worker_response_contract,
    runtime, runtime_claim, stop_child, unix_ms,
    worker_response_deadline_for_priority,
    HelperBridgeRuntime, HelperTaskPriority, WorkerDeadline,
};

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

#[derive(Clone, Copy)]
struct MeetingPcmInput<'a> {
    utterance: &'a RefCell<Option<FinalizedMeetingUtterance>>,
    noise_suppression_enabled: bool,
    prepare_ms: &'a Cell<u64>,
}

impl MeetingPcmInput<'_> {
    fn matches(&self, task: &str, payload: &Value) -> bool {
        let slot = self.utterance.borrow();
        let Some(u) = slot.as_ref() else { return false; };
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

// PCM stays in Rust ownership, never in the public worker JSON.
enum NativeVoiceIo<'a> {
    Bind,
    Synthesize(&'a mut Option<Vec<f32>>),
    ProbeSpeech(Vec<f32>),
}

fn send_worker_task_inner(task: &str, mut payload: Value, pcm: Option<MeetingPcmInput<'_>>,
                          voice: Option<NativeVoiceIo<'_>>) -> HelperBridgeWorkerResponse {
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

    // ASR and MT never cross Python IPC, including on preparation errors.
    if voice.is_some() || matches!(task, "asr_preload" | "translation_preload" | "transcribe" | "translate") {
        #[cfg(windows)]
        return native::dispatch(task, &mut payload, pcm, voice, priority, &permit, response_deadline_ms);
        #[cfg(not(windows))]
        return standalone_blocked_response(
            task, &request_id, priority, "error", "native_inference:platform_unsupported",
            "Native CUDA translation requires Windows.",
        );
    }
    // Only control/status and existing voice operations may enter IPC.
    if pcm.is_some() || !matches!(task, "ping" | "status" | "voice_actor_preflight" | "voice_actor_synthesize") {
        return standalone_blocked_response(
            task, &request_id, priority, "error", "helper_bridge:unsupported_task",
            "Only voice and control tasks may enter the Python worker.",
        );
    }

    let deadline = WorkerDeadline::new(response_deadline_ms);
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
                &mut runtime, stdin, payload, deadline,
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

    let stdin = match await_worker_stdin_write(stdin_receiver, deadline) {
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
            return response;
        }
    };

    let (mut worker_response, stdout) = match await_worker_stdout_read(
        stdout_receiver,
        deadline,
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
            return response;
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

            let ok = if task == "status" {
                super::super::helper_bridge_runtime::project_native_status(&mut runtime, &mut worker_response)
            } else {
                apply_worker_response(&mut runtime, &worker_response)
            };
            clear_active_request(&mut runtime, &request_id);
            runtime.updated_unix_ms = unix_ms();
            let message = worker_message(task, &worker_response);
            let mut response = response_with_runtime(
                ok,
                task,
                &request_id,
                priority,
                message,
                worker_response,
                &runtime,
            );
            if task == "status" {
                response.runtime_claim = super::NATIVE_STATUS_CLAIM.into();
            }
            response
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

pub(super) fn send_worker_task(task: &str, payload: Value) -> HelperBridgeWorkerResponse {
    send_worker_task_with_input(task, payload, None, None)
}

pub(super) fn bind_native_voice(payload: Value) -> HelperBridgeWorkerResponse {
    send_worker_task_with_input("voice_actor_preflight", payload, None, Some(NativeVoiceIo::Bind))
}

pub(super) fn synthesize_native_voice(payload: Value) -> (HelperBridgeWorkerResponse, Option<Vec<f32>>) {
    let mut samples = None;
    let response = send_worker_task_with_input("voice_actor_synthesize", payload, None,
        Some(NativeVoiceIo::Synthesize(&mut samples)));
    if !response.ok { samples = None; }
    (response, samples)
}

pub(super) fn transcribe_native_voice_probe(payload: Value, samples: Vec<f32>) -> HelperBridgeWorkerResponse {
    send_worker_task_with_input("transcribe", payload, None, Some(NativeVoiceIo::ProbeSpeech(samples)))
}

pub(super) fn send_worker_pcm_task(
    payload: Value,
    utterance: FinalizedMeetingUtterance,
    noise_suppression_enabled: bool,
) -> (HelperBridgeWorkerResponse, Option<FinalizedMeetingUtterance>) {
    let rejection = finalized_asr_frame_rejection(&utterance);
    let utterance = RefCell::new(Some(utterance));
    let prepare_ms = Cell::new(0);
    let input = MeetingPcmInput { utterance: &utterance, noise_suppression_enabled, prepare_ms: &prepare_ms };
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
        send_worker_task_with_input("transcribe", payload, Some(input), None)
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

fn send_worker_task_with_input(task: &str, payload: Value, pcm: Option<MeetingPcmInput<'_>>,
                               voice: Option<NativeVoiceIo<'_>>) -> HelperBridgeWorkerResponse {
    let priority = task_priority(task, &payload);
    let outbound_generation = if priority == HelperTaskPriority::MeetingOutbound
        && meeting_lane(&payload).as_deref() == Some("you")
    {
        meeting_generation(&payload)
    } else {
        None
    };
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
    let mut response = send_worker_task_inner(task, payload, pcm, voice);
    let initial_attempt_ms = observed_elapsed_ms(first_started)
        .saturating_sub(pcm.map_or(0, |input| input.prepare_ms.get()));

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

    if failed_required_outbound_task_invalidates_cache(task, priority, &response) {
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
        let input = MeetingPcmInput { utterance: &utterance, noise_suppression_enabled: false, prepare_ms: &prepare_ms };
        assert!(input.matches("transcribe", &payload));
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
