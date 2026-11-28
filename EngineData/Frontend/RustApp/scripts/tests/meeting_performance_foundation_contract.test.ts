import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

// Source contracts only. Execution, CUDA and receiver latency require the
// exact-revision Windows meeting baseline, not these assertions.
const source = (relative: string) =>
  readFileSync(new URL(`../../${relative}`, import.meta.url), "utf8");

const scheduler = source("src-tauri/src/commands/helper_bridge_runtime/scheduler.rs");
const transport = source("src-tauri/src/commands/helper_bridge/transport.rs");
const outbound = source("src-tauri/src/commands/meeting_session/outbound_pipeline.rs");
const timing = source("src-tauri/src/commands/meeting_session/session_state.rs");
const output = source("src-tauri/src/engine/audio/meeting_output_runtime.rs");
const playback = source("src-tauri/src/commands/meeting_session/playback_runtime.rs");
const diagnostics = source("src-tauri/src/commands/diagnostic_support.rs");
const vad = source("src-tauri/src/engine/audio/vad.rs");
const nativeVoice = source("src-tauri/src/commands/helper_bridge/transport/native.rs");

test("only one inference job is admitted; wait timing is measured by Rust scheduler", () => {
  assert.match(scheduler, /active: bool/);
  assert.match(scheduler, /if state\.active \{/);
  assert.match(scheduler, /pub fn scheduler_wait_ms\(&self\) -> u64/);
  assert.match(scheduler, /scheduler_wait_ms: wait_started\.elapsed\(\)/);
  assert.match(transport, /json!\(scheduler_wait_ms\)/);
  for (const key of ["asr_scheduler_wait_ms", "translation_scheduler_wait_ms", "tts_scheduler_wait_ms"]) {
    assert.match(outbound, new RegExp(key));
  }
});

test("audio-boundary latency estimates are labeled, numeric, and do not change VAD thresholds", () => {
  assert.match(timing, /estimated_end_to_first_audio_ms/);
  assert.match(timing, /estimated_onset_to_first_audio_ms/);
  assert.match(timing, /saturating_add\(timing\.metrics\.captured_speech_span_ms\)/);
  assert.match(vad, /minimum_speech_duration_ms: 140/);
});

test("CPAL build timing is isolated from worker processing; no persistent stream behavior was introduced", () => {
  assert.match(output, /let stream_build_started_at = Instant::now\(\)/);
  assert.match(output, /stream_build_ms: Some\(stream_build_ms\)/);
  assert.match(output, /fn stream_start_to_first_audio_ms\(/);
  assert.match(playback, /timing\.metrics\.output_stream_build_ms = route\.stream_build_ms/);
  assert.match(output, /drop\(stream\)/);
  assert.match(nativeVoice, /InferenceOutput::Voice\(/);
});

test("support bundle contains only numeric derived timing, no audio or transcript", () => {
  for (const field of ["estimated_speech_onset_to_first_audio", "asr_scheduler_wait",
    "translation_scheduler_wait", "tts_scheduler_wait", "output_stream_build"]) {
    assert.match(diagnostics, new RegExp(`"${field}": timing\\.`), field);
  }
  assert.doesNotMatch(diagnostics, /"source_text"\s*:|"translated_text"\s*:|"session_id"\s*:/);
  assert.doesNotMatch(diagnostics, /"finalized_unix_ms"\s*:|"first_playback_unix_ms"\s*:/);
});

test("Meeting worker attempt attribution keeps recovery distinct and old archives readable", () => {
  const meeting = source("src-tauri/src/commands/meeting_session.rs");
  const dto = source("src/app/bridge/runtimeApi.ts");
  const ui = source("src/components/settings/MeetingPerformanceDiagnostics.svelte");
  assert.match(transport, /"worker_attempt_timing"\.to_string\(\)/);
  assert.match(transport, /"scheduler_wait_total_ms": wait/);
  assert.doesNotMatch(transport, /struct WorkerAttemptObservation|checked_add\(self\.retry_scheduler_wait_ms/);
  assert.match(meeting, /pub struct MeetingWorkerAttemptTiming/);
  assert.match(meeting, /#\[serde\(default\)\]\s+pub asr_worker_attempt_timing/);
  assert.match(meeting, /get\("scheduler_wait_total_ms"\)/);
  assert.match(outbound, /timing\.metrics\.asr_worker_attempt_timing = super::worker_attempt_timing\(&asr\)/);
  assert.match(outbound, /timing\.metrics\.translation_worker_attempt_timing = super::worker_attempt_timing\(&translation\)/);
  assert.match(outbound, /timing\.metrics\.tts_worker_attempt_timing = super::worker_attempt_timing\(&tts\)/);
  assert.match(dto, /export type MeetingWorkerAttemptTiming/);
  for (const field of ["asr_worker_attempt", "translation_worker_attempt", "tts_worker_attempt"]) {
    assert.match(diagnostics, new RegExp(`"${field}": timing\\.`));
  }
  assert.match(ui, /stageMs - admissionWaitMs - recovery/);
  assert.match(ui, /inclusive initial\/retry totals are not ranked twice/);
});

test("Meeting native-only transport never clones or replays a request", () => {
  const send = transport.slice(transport.indexOf("fn send_worker_task_with_input("),
    transport.indexOf("#[cfg(test)]\nmod scheduler_failure_receipt_tests"));
  assert.equal([...send.matchAll(/send_worker_task_inner\(/g)].length, 1);
  assert.doesNotMatch(send, /payload\.clone\(|retry_payload|recover_live_meeting_helper_transport/);
  assert.match(transport, /"recovery_ms": null/);
  assert.match(transport, /"retry_attempt_ms": null/);
  assert.match(send, /add_worker_attempt_observation\(&mut response, initial_attempt_ms\)/);
  assert.match(send, /clear_meeting_outbound_pipeline\(generation\)/);
});

test("finalized Meeting PCM enters the existing scheduler and only native inference", () => {
  const consumers = source("src-tauri/src/commands/meeting_session/consumer_runtime.rs");
  const queue = source("src-tauri/src/commands/meeting_session/incoming_deferred.rs");
  const incoming = source("src-tauri/src/commands/meeting_session/incoming_pipeline.rs");
  assert.doesNotMatch(consumers, /write_finalized_.*_wav/);
  assert.match(queue, /utterance: FinalizedMeetingUtterance/);
  assert.doesNotMatch(queue, /audio_path: String|retain_failed_wav_cleanup/);
  assert.match(incoming, /let \(asr, deferred_utterance\) = send_helper_worker_pcm_task\(/);
  assert.match(outbound, /let \(asr, _\) = send_helper_worker_pcm_task\(/);
  assert.match(transport, /utterance\.into_inner\(\)\.and_then\(\|owned\| retain_deferred_pcm\(&response, owned\)\)/);
  assert.match(incoming, /let Some\(utterance\) = deferred_utterance else/);
  assert.doesNotMatch(incoming, /utterance\.clone\(|drop\(utterance\.frame\)/);
  assert.doesNotMatch(outbound, /utterance\.clone\(|drop\(utterance\.frame\)/);
  const inner = transport.indexOf("fn send_worker_task_inner(");
  const permit = transport.indexOf("acquire_scoped_helper_task_permit(priority, wait_generation, deferred_deadline)", inner);
  const defer = transport.indexOf("return incoming_deferred_response", permit);
  const nativeDispatch = transport.indexOf("return native::dispatch(", defer);
  const metadata = transport.indexOf("return native::control(", nativeDispatch);
  assert.ok(inner >= 0 && permit > inner && defer > permit && nativeDispatch > defer && metadata > nativeDispatch);
  assert.doesNotMatch(transport, /TemporaryAsrWav|write_finalized_.*_wav/);
  assert.match(transport.slice(nativeDispatch, metadata), /"native_inference:platform_unsupported"/);
  assert.doesNotMatch(transport, /begin_worker_stdin_write|begin_worker_stdout_read/);
  assert.doesNotMatch(transport, /dispatch_if_owned|retain_payload_for_live_retry|recover_live_meeting_helper_transport/);
  const native = source("src-tauri/src/commands/helper_bridge/transport/native.rs");
  assert.match(native, /permit: &HelperTaskPermit/);
  assert.doesNotMatch(native, /acquire_helper_task_permit\(|\.samples\.clone\(|write_finalized_.*_wav/);
  assert.match(native, /pcm\.utterance\.borrow_mut\(\)\.take\(\)/);
  assert.match(native, /condition_asr_pcm\(&mut samples, pcm\.noise_suppression_enabled\)/);
  assert.match(native, /drop\(host\);[\s\S]*?let result = pending\.wait_output\(\);/);
  assert.match(native, /host\.generation_token != bridge_generation/);
  assert.match(outbound, /asr_wall_ms\.saturating_sub\(timing\.metrics\.audio_prepare_ms\)/);
});

test("deferred incoming uses producer notification and age deadline, not retry polling", () => {
  const consumer = source("src-tauri/src/commands/meeting_session/consumer_runtime.rs");
  const producer = source("src-tauri/src/engine/audio/finalized_utterance.rs");
  const policy = source("src-tauri/src/commands/helper_bridge/request_policy.rs");
  const deferred = source("src-tauri/src/commands/meeting_session/incoming_deferred.rs");
  assert.match(consumer, /wait_for_incoming_work\(/);
  assert.doesNotMatch(consumer, /DEFERRED_INCOMING_RETRY_WAIT/);
  assert.match(policy, /compare_exchange\([\s\S]*?wake_incoming_consumer\(\)/);
  assert.match(producer, /fn wake_incoming_consumer\([\s\S]*?sync\.state\.lock\(\)[\s\S]*?sync\.ready\.notify_all\(\)/);
  assert.match(deferred, /MAX_DEFERRED_INCOMING_AGE: Duration = Duration::from_secs\(20\)/);
  assert.match(deferred, /front\.expires_at\.saturating_duration_since\(now\)/);
  assert.match(deferred, /now >= front\.expires_at/);
  assert.match(consumer, /deferred_incoming_wait_budget\(Instant::now\(\)\)/);
  assert.match(deferred, /if outbound_idle \{ guard\.pop_front\(\) \} else \{ None \}/);
});

test("playback capacity waits for receiver and cancellation notifications", () => {
  assert.match(playback, /mpsc::sync_channel::<PreparedPlaybackJob>\(PLAYBACK_QUEUE_CAPACITY\)/);
  assert.doesNotMatch(playback, /ENQUEUE_RETRY_DELAY/);
  assert.match(playback, /while let Ok\(mut job\) = receiver\.recv\(\) \{\s*wake_playback_waiters\(\)/);
  assert.match(playback, /wake\.1\.wait_timeout\(guard, remaining\)/);
  assert.match(playback, /runtime_generation_matches_epoch\(generation\)/);
  const state = source("src-tauri/src/engine/runtime_state.rs");
  assert.match(state, /crate::commands::meeting_session::wake_playback_waiters\(\)/);
});

test("native output cancellation wakes the existing completion channel", () => {
  assert.match(output, /control\.cancel_requested\.store\(true, Ordering::Release\);\s*\/\/[^\n]*\n\s*let _ = control\.completion_tx\.try_send\(Duration::ZERO\)/);
  assert.match(output, /wait_for_output_tail\(&completion_rx, tail,/);
  assert.match(output, /!runtime_generation_matches_epoch\(self\.generation\)/);
  assert.match(output, /release_output_stream\(stream, generation\)/);
});

test("outbound priority cannot survive an early utterance return", () => {
  const policy = source("src-tauri/src/commands/helper_bridge/request_policy.rs");
  const scope = outbound.slice(outbound.indexOf("pub(super) fn process_outbound_pcm("));
  assert.ok(scope.indexOf("OutboundPipelineRelease::new(generation)") >= 0);
  assert.ok(scope.indexOf("OutboundPipelineRelease::new(generation)") < scope.indexOf("send_helper_worker_pcm_task("));
  assert.match(policy, /impl Drop for OutboundPipelineRelease \{[\s\S]*?clear_meeting_outbound_pipeline\(self\.0\)/);
  assert.match(policy, /current <= generation && runtime_generation_matches_epoch\(generation\)/);
});

test("deferred deadline survives dequeue and scheduler admission without changing inference deadline", () => {
  const deferred = source("src-tauri/src/commands/meeting_session/incoming_deferred.rs");
  const incoming = source("src-tauri/src/commands/meeting_session/incoming_pipeline.rs");
  const scheduler = source("src-tauri/src/commands/helper_bridge_runtime/scheduler.rs");
  const native = source("src-tauri/src/commands/helper_bridge/transport/native.rs");
  assert.match(deferred, /first_deadline\.unwrap_or_else\(\|\| now \+ MAX_DEFERRED_INCOMING_AGE\)/);
  assert.match(incoming, /deferred_deadline: Option<Instant>/);
  assert.match(incoming, /transcript\.as_ref\(\), &source_language, &target_language, deferred_deadline/);
  assert.equal((incoming.match(/Some\(expires_at\)/g) ?? []).length, 2);
  assert.match(scheduler, /remaining\.min\(budget\)/);
  assert.match(scheduler, /SCHEDULER_DEFERRED_EXPIRED/);
  const afterPermit = transport.indexOf("let response = (|| -> HelperBridgeWorkerResponse");
  const afterExpiry = transport.indexOf("if deferred_deadline_elapsed(deferred_deadline)", afterPermit);
  const nativeDispatch = transport.indexOf("return native::dispatch(", afterExpiry);
  assert.ok(afterPermit >= 0 && afterPermit < afterExpiry && afterExpiry < nativeDispatch);
  const lastExpiry = native.lastIndexOf("if deferred_deadline_elapsed(deferred_deadline)");
  const begin = native.indexOf("let pending_result = {", lastExpiry);
  assert.ok(lastExpiry >= 0 && lastExpiry < begin);
  assert.match(native, /owner\.owner\.begin\(input\.expect\("checked text or speech input"\), deadline\)/);
  assert.match(incoming, /record_deferred_expiration\(&translation\)[\s\S]*?finish_meeting_turn_translation\(/);
});

test("incoming translation owns initial and repeated deferral without renewing age or cloning queued text", () => {
  const incoming = source("src-tauri/src/commands/meeting_session/incoming_pipeline.rs");
  const mt = incoming.slice(incoming.indexOf("fn translate_and_commit_incoming_transcript("), incoming.indexOf("pub(super) fn drain_due_deferred_incoming("));
  assert.equal((incoming.match(/stage: DeferredIncomingStage::NeedsTranslation \{/g) ?? []).length, 1);
  assert.match(mt, /transcript: transcript\.into_owned\(\)/);
  assert.match(mt, /expires_at: deferred_expiration\(deferred_deadline, Instant::now\(\)\)/);
  assert.match(mt, /if deferred_deadline\.is_some\(\) \{\s*requeue_deferred_incoming_front\(job\)\s*\} else \{\s*enqueue_deferred_incoming\(job\)/);
  assert.match(incoming, /Cow::Borrowed\(transcript\)/);
  assert.match(incoming, /Cow::Owned\(transcript\)/);
  assert.doesNotMatch(incoming, /allow_defer/);
  assert.match(mt, /if !super::local_translated_meeting_route_admitted\(\) \{[\s\S]*?finish_meeting_turn_translation\([\s\S]*?"incoming", None/);
});

test("finalization checks only the required signal and clipping evidence before PCM handoff", () => {
  const producer = source("src-tauri/src/engine/audio/finalized_utterance.rs");
  const evidence = source("src-tauri/src/engine/audio/evidence.rs");
  const finalize = producer.slice(producer.indexOf("fn finalize_current_utterance("), producer.indexOf("fn adaptive_end_silence_ms("));
  const gate = finalize.indexOf("finalized_speech_rejected(&state.current_samples[..speech_end], state.profile.gate.max_clipping_ratio)");
  const transfer = finalize.indexOf("std::mem::take(&mut state.current_samples)");
  assert.ok(gate >= 0 && gate < transfer);
  assert.doesNotMatch(finalize, /AudioEvidenceReport::from_samples/);
  const check = evidence.slice(evidence.indexOf("pub(super) fn finalized_speech_rejected("), evidence.indexOf("pub(super) fn noise_suppression_threshold("));
  assert.match(check, /safe_sample\(sample\)\.abs\(\)/);
  assert.match(check, /level >= 0\.98/);
  assert.match(check, /!has_signal \|\| safe_ratio\(clipping_count, samples\.len\(\)\) > max_clipping_ratio/);
  assert.doesNotMatch(check, /frame_metrics\(|AudioEvidenceReport::from_samples/);
});

test("Meeting critical path borrows session authority without building diagnostic reports", () => {
  const meeting = source("src-tauri/src/commands/meeting_session.rs");
  const phase = meeting.slice(meeting.indexOf("fn generation_in_phase("), meeting.indexOf("fn process_authoritative_finalized_outbound_pcm("));
  assert.doesNotMatch(phase, /latest_runtime_session_state\(|runtime_generation_is_authoritative\(/);
  assert.match(phase, /runtime_generation_matches_epoch\(generation\)\s*&& observe_runtime_session/);
  assert.match(phase, /snapshot\.generation == generation\s*&& snapshot\.authority_active && snapshot\.phase == phase/);
  assert.match(phase, /generation_in_phase\(generation, "live"\)/);
  assert.match(phase, /generation_in_phase\(generation, "starting"\)/);
  assert.match(phase, /snapshot\.owner_id == APPLICATION_MEETING_OWNER_ID[\s\S]*?snapshot\.session_id == session_id[\s\S]*?snapshot\.authority_active[\s\S]*?snapshot\.phase == "live"/);
  const turns = source("src-tauri/src/commands/meeting_session/committed_turns.rs");
  const mutation = turns.slice(turns.indexOf("pub(super) fn commit_meeting_turn("), turns.indexOf("pub(super) fn interrupt_committed_turns_for_generation("));
  assert.doesNotMatch(mutation, /latest_runtime_session_state\(/);
  assert.equal((mutation.match(/observe_runtime_session\(\|snapshot\| live_turn_owner_matches/g) ?? []).length, 5);
  assert.match(mutation, /if delivery_state != "interrupted" \{\s*if !observe_runtime_session/);
  assert.match(mutation, /transcript_archive::queue_turn/);
  const status = source("src-tauri/src/commands/meeting_session/session_state.rs");
  assert.doesNotMatch(status, /latest_runtime_session_state\(/);
  assert.match(status, /observe_runtime_session\(\|snapshot\| outbound_status_update_allowed/);
  assert.match(status, /observe_runtime_session\(\|snapshot\| incoming_status_update_allowed/);
  assert.match(status, /status\.timing\.take\(\)/);
});
