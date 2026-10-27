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

test("Start retains lifecycle ownership through activation and rollback before Stop", () => {
  const start = lifecycle.indexOf("fn start_meeting_impl(");
  const claim = lifecycle.indexOf("let starting = begin_application_meeting_session();", start);
  const commit = lifecycle.indexOf("let committed = commit_application_meeting_session_live(", claim);
  const finish = lifecycle.indexOf("pub(super) fn stop_meeting_translation_impl()", commit);
  const stopRevoke = lifecycle.indexOf("let revoked = revoke_runtime_session_authority(", finish);
  const cleanup = lifecycle.indexOf("original_voice_pcm::stop(generation);", stopRevoke);
  assert.ok(start > 0 && claim > start && commit > claim && finish > commit
    && stopRevoke > finish && cleanup > stopRevoke);
  const opening = lifecycle.slice(start, claim);
  assert.match(opening, /let _transition = match meeting_lifecycle_gate\(\)\.try_lock\(\)/);
  assert.ok(opening.indexOf("let _transition =") < opening.indexOf("let current = latest_runtime_session_state();"),
    "the transaction guard must precede all readiness and helper mutations");
  assert.doesNotMatch(lifecycle.slice(claim, finish), /let _transition =/,
    "a shadowed claim-only guard would release lifecycle ownership prematurely");
  assert.match(lifecycle.slice(claim, finish), /rollback_starting_meeting_resources\(generation, &session_id\)/);
  assert.match(lifecycle.slice(finish, stopRevoke), /meeting_lifecycle_gate\(\)\.try_lock\(\)/);
  assert.match(lifecycle.slice(stopRevoke, cleanup), /stop_revoked_own_generation\(&revoked, generation, &session_id\)/);
});

test("Retry Stop owns WAV sweeping until consumers and playback have exited", () => {
  const rollback = lifecycle.slice(
    lifecycle.indexOf("fn rollback_starting_meeting_resources("),
    lifecycle.indexOf("fn clear_transcript_only_after_retention("),
  );
  assert.match(rollback, /let wav_ok = outbound\.ok\s*&& cleanup_finalized_meeting_session_wavs\(session_id\)\.is_ok\(\)/);
  assert.match(rollback, /let tts_ok = output_cancelled && helper\.ok && outbound\.ok && playback\.ok\s*&& cleanup_meeting_tts_for_session\(session_id, generation\)\.is_ok\(\)/);

  const stop = lifecycle.slice(
    lifecycle.indexOf("pub(super) fn stop_meeting_translation_impl()"),
    lifecycle.indexOf("pub(super) fn meeting_cleanup_complete("),
  );
  const consumers = stop.indexOf("let incoming_cleanup = stop_meeting_incoming_consumer(&session_id);");
  const finalized = stop.indexOf("let finalized_audio_cleanup = if outbound_cleanup.ok && incoming_cleanup.ok");
  const tts = stop.indexOf("let tts_cache_cleanup = if output_cancelled && helper_cancel.ok");
  assert.ok(consumers > 0 && finalized > consumers && tts > finalized,
    "all file sweeps must follow bounded consumer Stop checks");
  assert.match(stop.slice(finalized, tts), /finalized_utterance_cleanup:consumer_exit_unconfirmed/);
  assert.match(stop.slice(tts), /meeting_tts_cleanup:producer_or_playback_exit_unconfirmed/);
  assert.match(stop.slice(tts), /finalized_audio_cleanup\.is_ok\(\) && tts_cache_cleanup\.is_ok\(\)/,
    "skipped file cleanup must mark Stop incomplete");
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
