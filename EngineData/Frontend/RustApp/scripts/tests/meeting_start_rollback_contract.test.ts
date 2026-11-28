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

test("initial Start uses its own validated route and still rechecks after AI and native resource acquisition", () => {
  const lifecycle = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/lifecycle.rs", import.meta.url), "utf8");
  const preflight = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/preflight.rs", import.meta.url), "utf8");
  const start = lifecycle.slice(lifecycle.indexOf("fn start_meeting_impl("), lifecycle.indexOf("pub(super) fn stop_meeting_translation_impl("));
  const initial = start.indexOf("let (preflight, prepared_route) = build_preflight_with_route();");
  const endpoint = start.indexOf("prepared_route.selected_output_device.as_deref()");
  const claim = start.indexOf("let starting = begin_application_meeting_session()");
  const bind = start.indexOf("bind_prepared_virtual_mic_route_to_generation(generation)");
  const probe = start.indexOf("prepare_required_outbound_ai_runtime(generation)");
  const afterAI = start.indexOf("let ai_preflight = build_preflight();");
  const native = start.indexOf("probe_prepared_meeting_output_device_functionally(output_device, generation)");
  const final = start.indexOf("let prepared_preflight = build_preflight();");
  assert.ok(initial >= 0 && initial < endpoint && endpoint < claim && claim < bind);
  assert.ok(bind < probe && probe < afterAI && afterAI < native && native < final);
  assert.doesNotMatch(start, /get_virtual_mic_route_selection\(/);
  assert.match(preflight, /build_preflight_with_route\(\)\.0/);
  assert.equal((preflight.match(/let route = get_virtual_mic_route_selection\(\)/g) ?? []).length, 1);
  assert.match(preflight, /let cleanup_pending = transient_stop_cleanup_pending\(\)/);
  assert.match(preflight, /if cleanup_pending \{\s*start_blockers.push/);
  assert.match(preflight, /summary: if cleanup_pending/);
});

test("idle Start observes unscoped Worker capability and reuses Live preflight", () => {
  const preflight = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/preflight.rs", import.meta.url), "utf8");
  const start = lifecycle.slice(lifecycle.indexOf("fn start_meeting_impl("), lifecycle.indexOf("pub(super) fn stop_meeting_translation_impl("));
  const refresh = start.indexOf('let refreshed = send_helper_worker_task("status", json!({}));');
  const claim = start.indexOf("let starting = begin_application_meeting_session()");
  assert.ok(refresh >= 0 && refresh < claim);
  assert.match(start.slice(refresh, claim), /if !idle_worker_status_admitted\(&refreshed\)/);
  assert.doesNotMatch(start.slice(0, claim), /"meeting_start_prepare": true/);
  assert.match(start.slice(0, refresh), /status: status_for_report\(current\)/);
  assert.match(preflight, /fn status_for_report\(report: RuntimeSessionStateReport\)[\s\S]*?preflight_for_report\(&report, build_preflight\)/);
});
