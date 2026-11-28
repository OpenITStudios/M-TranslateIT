import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const utterance = readFileSync(new URL("../../src-tauri/src/engine/audio/finalized_utterance.rs", import.meta.url), "utf8");
const consumer = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/consumer_runtime.rs", import.meta.url), "utf8");
const timing = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/session_state.rs", import.meta.url), "utf8");
const meeting = readFileSync(new URL("../../src-tauri/src/commands/meeting_session.rs", import.meta.url), "utf8");
const outbound = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/outbound_pipeline.rs", import.meta.url), "utf8");
const native = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge/transport/native.rs", import.meta.url), "utf8");
const runtime = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge_runtime.rs", import.meta.url), "utf8");
const owner = readFileSync(new URL("../../../../../tools/translation_quality/native_cuda/src/owner.rs", import.meta.url), "utf8");

// The old Python speed-factor feature is retired. This suite protects current
// source timing and selected-voice admission, not acoustic prosody parity.
test("native outbound preserves measured source speech duration for timing", () => {
  assert.match(utterance, /speech_duration_ms: u64/);
  assert.match(utterance, /duration_ms\(speech_end, state\.sample_rate_hz\)/);
  assert.match(utterance, /speech_duration_ms: u64::from\(speech_duration_ms\)/);
  assert.match(consumer, /timing_context_from_utterance\(&utterance, queue_ms, 0\)/);
  assert.match(timing, /captured_speech_span_ms: utterance\.speech_duration_ms/);
  assert.match(outbound, /let speech_duration_ms = utterance\.speech_duration_ms/);
  assert.match(outbound, /turn\.worker_metadata\(Some\(speech_duration_ms\)\)/);
  assert.match(meeting, /metadata\["source_speech_duration_ms"\] = serde_json::json!\(duration\)/);
});

test("native selected voice receives only translated text and the bound actor", () => {
  assert.match(outbound, /synthesize_native_voice\(\s*turn\.worker_metadata\(None\), translated_text, &actor_token,/);
  assert.match(native, /owner\.begin_bound_voice\(request_scope\.generation, request_scope\.session_id\.as_deref\(\),\s*actor_token\.expect\("checked token"\), preparing, \(\*text\)\.to_owned\(\), deadline\)/);
  assert.match(runtime, /self\.owner\.begin\(InferenceInput::Voice \{ text, binding: selected\.binding \}, deadline\)/);
  assert.match(owner, /Voice \{ text: String, binding: VoiceBinding \}/);
  assert.doesNotMatch(outbound, /"source_text":|speed_factor/);
});
