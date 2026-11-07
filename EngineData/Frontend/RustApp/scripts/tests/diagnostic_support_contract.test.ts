import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const support = readFileSync(new URL("../../src-tauri/src/commands/diagnostic_support.rs", import.meta.url), "utf8");

test("support bundle is explicit local export with privacy whitelist", () => {
  assert.match(support, /SavedProject|user_saved_dir/);
  assert.match(support, /contains_transcript.*false/);
  assert.match(support, /contains_audio.*false/);
  assert.match(support, /contains_voice_reference.*false/);
  assert.doesNotMatch(support, /"source_text"\s*:|"translated_text"\s*:|"turns"\s*:/);
});

test("support bundle omits raw local identifiers", () => {
  assert.doesNotMatch(support, /stderr_log_path/);
  assert.doesNotMatch(support, /selected_output_device/);
  assert.doesNotMatch(support, /selected_input_device/);
  assert.doesNotMatch(support, /"session_id"\s*:/);
  assert.match(support, /sanitize_diagnostic_text/);
});

test("capture health distinguishes native errors and PCM loss without exporting raw callback text", () => {
  assert.match(support, /"native_callback_error_count": microphone_capture\.callback_error_count/);
  assert.match(support, /"pcm_drop_count": microphone_capture\.pcm_drop_count/);
  assert.match(support, /"native_callback_error_count": incoming_capture\.callback_error_count/);
  assert.match(support, /"pcm_drop_count": incoming_capture\.pcm_drop_count/);
  assert.doesNotMatch(support, /"latest_callback_error"\s*:|"device_name"\s*:/);
});


test("support bundle snapshot does not mutate incident history while observing guards", () => {
  assert.match(support, /without_runtime_incident_recording/);
});

test("support bundle records only numeric stage timing, not private speech timestamps", () => {
  assert.match(support, /"timing_ms": meeting\.outbound\.timing\.as_ref\(\)/);
  for (const field of ["speech_boundary", "finalization", "queue", "audio_prepare",
    "asr", "translation", "tts", "playback_queue", "delivery", "outbound_latency",
    "estimated_speech_onset_to_first_audio", "estimated_speech_end_to_first_audio",
    "asr_scheduler_wait", "translation_scheduler_wait", "tts_scheduler_wait",
    "output_stream_build", "output_stream_start_to_first_audio"]) {
    assert.ok(support.includes('"' + field + '": timing.'), field);
  }
  assert.doesNotMatch(support, /"finalized_unix_ms":|"first_playback_unix_ms":/);
});

test("capture callback contention evidence contains aggregates, not private speech", () => {
  const producer = readFileSync(
    new URL("../../src-tauri/src/engine/audio/finalized_utterance.rs", import.meta.url), "utf8",
  );
  assert.match(producer, /pub fn finalized_callback_lock_metrics\(\)/);
  assert.match(producer, /callback_lock_wait_max_ns\.fetch_max/);
  assert.match(support, /"capture_callback_lock"/);
  for (const field of ["outbound_observations", "outbound_max_wait_us",
    "incoming_observations", "incoming_max_wait_us"]) {
    assert.ok(support.includes('"' + field + '": capture_lock.'), field);
  }
  assert.doesNotMatch(support, /"audio_samples"\s*:|"source_text"\s*:/);
});

test("support export projects a bounded incident issue map without claiming native root cause", () => {
  assert.match(support, /"issue_map"/);
  assert.match(support, /"scope": "RECENT_BOUNDED_INCIDENTS"/);
  assert.match(support, /"coverage": incidents\.coverage/);
  assert.match(support, /"areas": &incidents\.issue_areas/);
  assert.match(support, /"total_failure_frequency_measured": false/);
  assert.match(support, /"native_root_cause_verified": false/);
});

test("support bundle includes optional build attribution without claiming release acceptance", () => {
  const builder = readFileSync(new URL("../../src-tauri/build.rs", import.meta.url), "utf8");
  assert.match(builder, /TRANSLATEIT_EXPECTED_SHA/);
  assert.match(builder, /TRANSLATEIT_BUILD_SOURCE_SHA/);
  assert.match(builder, /--porcelain/);
  assert.match(support, /"source_sha": option_env!\("TRANSLATEIT_BUILD_SOURCE_SHA"\)/);
  assert.match(support, /"source_identity_scope": if option_env!/);
  assert.match(support, /"source_sha_is_signed_attestation": false/);
  assert.match(support, /"legacy_issue_build_attribution": "UNKNOWN"/);
});
