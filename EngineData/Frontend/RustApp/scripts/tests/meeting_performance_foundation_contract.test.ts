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
const actor = readFileSync(new URL("../../../../Backend/LocalWorker/WorkerRuntime/voice_lab_gpt_sovits.py", import.meta.url), "utf8");

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
  assert.match(actor, /"streaming_mode": False/);
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
  const permit = transport.indexOf("acquire_helper_task_permit(priority)", inner);
  const defer = transport.indexOf("return incoming_deferred_response", permit);
  const nativeDispatch = transport.indexOf("return native::dispatch(", defer);
  const metadata = transport.indexOf("inject_request_metadata", nativeDispatch);
  assert.ok(inner >= 0 && permit > inner && defer > permit && nativeDispatch > defer && metadata > nativeDispatch);
  assert.doesNotMatch(transport, /TemporaryAsrWav|write_finalized_.*_wav/);
  assert.match(transport.slice(nativeDispatch, metadata), /"native_inference:platform_unsupported"/);
  assert.match(transport.slice(nativeDispatch, metadata), /!matches!\(task, "ping" \| "status" \| "voice_actor_preflight" \| "voice_actor_synthesize"\)/);
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
