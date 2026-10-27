import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const lifecycle = readFileSync(
  new URL("../../src-tauri/src/commands/meeting_session/lifecycle.rs", import.meta.url),
  "utf8",
);

test("pre-Live rollback has one complete resource owner", () => {
  assert.match(lifecycle, /fn rollback_starting_meeting_resources/);
  assert.match(lifecycle, /cancel_meeting_output_for_generation\(generation\)/);
  assert.match(lifecycle, /stop_live_capture_runtime\(\)/);
  assert.match(lifecycle, /stop_meeting_sound_capture_runtime\(\)/);
  assert.match(lifecycle, /cancel_helper_bridge_meeting_session\(session_id\)/);
  assert.match(lifecycle, /stop_meeting_outbound_consumer\(generation\)/);
  assert.match(lifecycle, /clear_prepared_virtual_mic_route_selection\(\)/);
  assert.match(lifecycle, /clear_runtime_session_if_generation\(generation\)/);
});

test("Start prepares output only after winning session and route ownership", () => {
  const start = lifecycle.indexOf("fn start_meeting_impl(");
  const claim = lifecycle.indexOf("let starting = begin_application_meeting_session();", start);
  const bound = lifecycle.indexOf("bind_prepared_virtual_mic_route_to_generation(generation)", claim);
  const reset = lifecycle.indexOf("clear_all_start_preflight();", bound);
  const prepare = lifecycle.indexOf("if let Err(blocker) = prepare_meeting_output_device(output_device)", reset);
  const source = lifecycle.indexOf("original_voice_pcm::register_translated(generation)", prepare);
  assert.ok(start > 0 && claim > start && bound > claim && reset > bound
    && prepare > reset && source > prepare, "session claim precedes every shared output mutation");
  const unowned = lifecycle.slice(start, bound);
  assert.doesNotMatch(unowned, /clear_all_start_preflight\(\)|clear_prepared_meeting_output_device\(\)|prepare_meeting_output_device\(output_device\)/,
    "a rejected Start must not mutate another Start's shared resources");
  const failure = lifecycle.slice(prepare, source);
  assert.match(failure, /revoke_runtime_session_authority\(/);
  assert.match(failure, /rollback_starting_meeting_resources\(generation, &session_id\)/);
  assert.match(failure, /rollback_completion_state\(\)/);
});

test("Meeting Stop and new Start claim share one fail-closed lifecycle gate", () => {
  const start = lifecycle.indexOf("fn start_meeting_impl(");
  const claim = lifecycle.indexOf("let starting = begin_application_meeting_session();", start);
  const stop = lifecycle.indexOf("pub(super) fn stop_meeting_translation_impl()");
  const revoke = lifecycle.indexOf("let revoked = revoke_runtime_session_authority(", stop);
  const cleanup = lifecycle.indexOf("original_voice_pcm::stop(generation);", revoke);
  assert.ok(start >= 0 && claim > start && stop > claim && revoke > stop && cleanup > revoke);
  assert.match(lifecycle.slice(start, claim), /meeting_lifecycle_gate\(\)\.try_lock\(\)/);
  assert.match(lifecycle.slice(stop, revoke), /meeting_lifecycle_gate\(\)\.try_lock\(\)/);
  assert.match(lifecycle.slice(revoke, cleanup), /stop_revoked_own_generation\(&revoked, generation, &session_id\)/);
});

test("Live commit failure uses the canonical rollback instead of a partial cleanup copy", () => {
  const start = lifecycle.lastIndexOf("let committed = commit_application_meeting_session_live");
  const end = lifecycle.indexOf("update_outbound_status(", start);
  const block = lifecycle.slice(start, end);
  assert.match(block, /rollback_starting_meeting_resources\(generation, &session_id\)/);
  assert.match(block, /Helper cleanup:/);
  assert.doesNotMatch(block, /stop_live_capture_runtime\(\)/);
  assert.doesNotMatch(block, /stop_meeting_outbound_consumer\(generation\)/);
});
