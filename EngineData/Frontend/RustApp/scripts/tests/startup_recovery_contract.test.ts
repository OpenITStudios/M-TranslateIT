import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const recovery = readFileSync(new URL("../../src-tauri/src/commands/startup_recovery.rs", import.meta.url), "utf8");
const bootstrap = readFileSync(new URL("../../src-tauri/src/app_bootstrap.rs", import.meta.url), "utf8");
const main = readFileSync(new URL("../../src-tauri/src/main.rs", import.meta.url), "utf8");
const shutdown = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/shutdown.rs", import.meta.url), "utf8");
const helper = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge.rs", import.meta.url), "utf8");
const helperRuntime = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge_runtime.rs", import.meta.url), "utf8");
const transport = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge/transport.rs", import.meta.url), "utf8");
const meetingLifecycle = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/lifecycle.rs", import.meta.url), "utf8");
const helperCancellationTests = readFileSync(
  new URL("../../src-tauri/src/commands/helper_bridge_contract_tests.rs", import.meta.url),
  "utf8",
);
const applicationLifecycle = readFileSync(
  new URL("../../src-tauri/src/commands/application_runtime/lifecycle.rs", import.meta.url),
  "utf8",
);
const resources = readFileSync(
  new URL("../../src-tauri/src/commands/application_runtime/resources.rs", import.meta.url),
  "utf8",
);
const closePolicy = readFileSync(
  new URL("../../src/app/runtime/closePolicy.ts", import.meta.url),
  "utf8",
);
const closeController = readFileSync(
  new URL("../../src/app/runtime/closeController.ts", import.meta.url),
  "utf8",
);
const meetingSound = readFileSync(
  new URL("../../src-tauri/src/engine/audio/meeting_sound_capture.rs", import.meta.url),
  "utf8",
);
const meetingConsumers = readFileSync(
  new URL("../../src-tauri/src/commands/meeting_session/consumer_runtime.rs", import.meta.url),
  "utf8",
);
const meetingPlayback = readFileSync(
  new URL("../../src-tauri/src/commands/meeting_session/playback_runtime.rs", import.meta.url),
  "utf8",
);

test("startup recovery never resumes Meeting and cleans only bounded cache namespaces", () => {
  assert.match(bootstrap, /begin_startup_recovery/);
  assert.match(recovery, /audio_segments/);
  assert.match(recovery, /meeting_tts/);
  assert.match(recovery, /helper_functional_readiness/);
  assert.doesNotMatch(recovery, /start_meeting_translation|begin_application_meeting_session/);
  assert.doesNotMatch(recovery, /remove_dir_all\([^)]*user_cache_dir/);
});

test("active previous process prevents destructive stale cleanup without trusting reused PIDs", () => {
  assert.match(recovery, /marker_process_is_alive/);
  assert.match(recovery, /process\.start_time\(\)/);
  assert.match(recovery, /marker_matches_process_start/);
  assert.match(recovery, /let Some\(marker\) = read_marker[\s\S]*stale_found = true/);
  assert.match(recovery, /another_instance_detected = live_other_found/);
  assert.match(recovery, /Shared Meeting cache was left untouched|did not modify the other process's Meeting cache/);
});

test("clean app exit remains fail-closed and clears the marker after helper shutdown", () => {
  assert.match(main, /application_runtime::prepare_for_app_exit\(\)/);
  assert.match(shutdown, /if !result\.ok \|\| result\.status\.has_session/);
  assert.match(shutdown, /finish_app_exit\(\s*&latest_runtime_session_state\(\),\s*super::super::meeting_session::meeting_stop_cleanup_pending\(\),\s*super::super::helper_bridge::shutdown_helper_bridge_for_app_exit,\s*super::super::startup_recovery::mark_clean_shutdown,\s*\)/);
  assert.match(shutdown, /shutdown_helper\(\) && mark_clean\(\)/);
  assert.ok(shutdown.indexOf("mark_clean_shutdown") > shutdown.indexOf("shutdown_helper_bridge_for_app_exit"));
});

test("helper cleanup keeps unjoined worker resources and clean-exit marker is fail-closed", () => {
  const core = helperRuntime.slice(helperRuntime.indexOf("fn stop_child_with_budget("), helperRuntime.indexOf("#[cfg(test)]"));
  assert.match(core, /child\.try_wait\(\)/);
  assert.match(core, /child\.kill\(\)/);
  assert.match(core, /logger\.is_finished\(\)/);
  assert.match(core, /runtime\.stderr_logger\.take\(\)/);
  assert.match(core, /if started\.elapsed\(\) >= budget \{\s*return false/);
  assert.doesNotMatch(core, /child\.wait\(\)/);
  assert.match(helper, /pub\(crate\) fn shutdown_helper_bridge_for_app_exit\(\) -> bool \{[\s\S]*?runtime\(\)\.try_lock\(\)/);
  assert.match(helper, /if !stop_child\(&mut runtime\) \{[\s\S]*?"helper_bridge:cleanup_incomplete",[\s\S]*?return false/);
  assert.match(helper, /let cleanup_unverified = runtime\.last_error\.as_deref\(\)\s*== Some\("helper_bridge:cleanup_incomplete"\)/);
  assert.match(helper, /if !stop_child\(&mut runtime\) \{[\s\S]*?"Meeting helper Stop could not verify process/);
  assert.match(transport, /let cleaned = stop_child\(&mut runtime\);/);
  assert.match(transport, /runtime\.state = if cleaned \{ "stopped" \} else \{ "blocked" \}/);
  assert.match(meetingLifecycle, /helper_cancel\.ok/);
  assert.match(shutdown, /shutdown_helper\(\) && mark_clean\(\)/);
});


test("timed-out Python stdout reader remains owned until bounded cleanup joins it", () => {
  assert.match(helperRuntime, /pub stdout_reader: Option<thread::JoinHandle<\(\)>>/);
  assert.match(helperRuntime, /fn begin_worker_stdout_read<R: BufRead \+ Send \+ 'static>/);
  assert.match(helperRuntime, /let reader = thread::Builder::new\(\)/);
  assert.match(helperRuntime, /runtime\.stdout_reader = Some\(reader\)/);
  assert.match(helperRuntime, /worker:stdout_reader_spawn_failed/);
  assert.match(helperRuntime, /fn await_worker_stdout_read<R>/);
  assert.match(helperRuntime, /Err\(RecvTimeoutError::Timeout\)/);
  assert.match(helperRuntime, /fn finish_received_stdout_read\(/);
  assert.match(helperRuntime, /if result\.is_ok\(\) && !finish_received_stdout_read\(runtime\)/);

  const stop = helperRuntime.slice(
    helperRuntime.indexOf("fn stop_child_with_budget("),
    helperRuntime.indexOf("#[cfg(test)]"),
  );
  assert.match(stop, /runtime\.stdout_reader\.as_ref\(\)\.is_some_and\(\|reader\| reader\.is_finished\(\)\)/);
  assert.match(stop, /runtime\.stdout_reader\.take\(\)/);
  assert.match(stop, /runtime\.stdin_writer\.is_none\(\)\s*&&\s*runtime\.stdout_reader\.is_none\(\)\s*&&\s*runtime\.stderr_logger\.is_none\(\)/);
  assert.match(stop, /if started\.elapsed\(\) >= budget \{\s*return false/);

  assert.match(helper, /read_worker_response_direct_with_deadline\(&mut runtime, stdout, ping_deadline_ms\)/);
  assert.match(helper, /read_worker_response_direct_with_deadline\(\s*&mut runtime,\s*stdout,\s*status_deadline_ms,?\s*\)/);
  const registered = transport.indexOf("begin_worker_stdout_read(&mut runtime, stdout)");
  const writerRegistered = transport.indexOf("begin_worker_stdin_write(");
  const awaitedWrite = transport.indexOf("await_worker_stdin_write(stdin_receiver, response_deadline_ms)");
  const awaited = transport.indexOf("await_worker_stdout_read(");
  const writerJoined = transport.indexOf("finish_received_stdin_write(&mut runtime)");
  const joined = transport.indexOf("finish_received_stdout_read(&mut runtime)");
  const stdinReused = transport.indexOf("runtime.stdin = Some(stdin)");
  const reused = transport.indexOf("runtime.stdout = Some(stdout)");
  assert.ok(registered > 0 && registered < writerRegistered, "reader and writer must register under one mutex");
  assert.ok(writerRegistered < awaitedWrite && awaitedWrite < awaited, "stdin deadline precedes stdout deadline");
  assert.ok(awaited < writerJoined && writerJoined < stdinReused, "write join must precede pipe reuse");
  assert.ok(awaited < joined && joined < reused, "read join must precede pipe reuse");
  assert.match(transport, /if !bridge_still_owns_process \{/);
  assert.match(helperRuntime, /fn timed_out_stdout_reader_remains_owned_until_explicit_retry_stop\(\)/);
  assert.match(helperRuntime, /fn successful_worker_response_returns_original_stdout_after_join\(\)/);
});

test("worker stdin write timeout is enforced on an owned thread without losing cleanup authority", () => {
  assert.match(helperRuntime, /pub stdin_writer: Option<thread::JoinHandle<\(\)>>/);
  assert.match(helperRuntime, /fn begin_worker_stdin_write<W: Write \+ Send \+ 'static>/);
  assert.match(helperRuntime, /let writer = thread::Builder::new\(\)/);
  assert.match(helperRuntime, /runtime\.stdin_writer = Some\(writer\)/);
  assert.match(helperRuntime, /worker:stdin_writer_spawn_failed/);
  assert.match(helperRuntime, /fn await_worker_stdin_write<W>/);
  assert.match(helperRuntime, /request_write_deadline_exceeded:/);
  assert.match(helperRuntime, /fn finish_received_stdin_write\(/);
  assert.match(helperRuntime, /fn write_worker_request_direct_with_deadline\(/);
  assert.match(helperRuntime, /if result\.is_ok\(\) && !finish_received_stdin_write\(runtime\)/);

  const cleanup = helperRuntime.slice(
    helperRuntime.indexOf("fn stop_child_with_budget("),
    helperRuntime.indexOf("#[cfg(test)]"),
  );
  assert.match(cleanup, /runtime\.stdin_writer\.as_ref\(\)\.is_some_and\(\|writer\| writer\.is_finished\(\)\)/);
  assert.match(cleanup, /runtime\.stdin_writer\.take\(\)/);
  assert.match(cleanup, /runtime\.stdin_writer\.is_none\(\)/);
  assert.match(cleanup, /runtime\.stdout_reader\.is_none\(\)/);
  assert.match(cleanup, /runtime\.stderr_logger\.is_none\(\)/);

  assert.match(helper, /write_worker_request_direct_with_deadline\([\s\S]*?"command": "ping", "request_id": STARTUP_PING_REQUEST_ID/);
  assert.match(helper, /write_worker_request_direct_with_deadline\([\s\S]*?"command": "status", "request_id": STARTUP_STATUS_REQUEST_ID/);
  assert.match(helper, /"helper_bridge:status_write_failed"/);
  assert.doesNotMatch(helper, /write_worker_request_with_deadline\(/);

  const writerRegistered = transport.indexOf("begin_worker_stdin_write(");
  const timeoutWait = transport.indexOf("await_worker_stdin_write(stdin_receiver, response_deadline_ms)");
  const writerJoined = transport.indexOf("finish_received_stdin_write(&mut runtime)");
  const reused = transport.indexOf("runtime.stdin = Some(stdin)");
  assert.ok(writerRegistered > 0 && writerRegistered < timeoutWait);
  assert.ok(timeoutWait < writerJoined && writerJoined < reused);
  assert.match(transport, /if !bridge_still_owns_process \{/);
  assert.match(transport, /"helper_bridge:stdin_writer_join_failed"/);
  assert.match(helperRuntime, /fn stdin_backpressure_timeout_keeps_writer_owned_until_retry_cleanup\(\)/);
  assert.match(helperRuntime, /fn completed_stdin_write_retains_original_pipe_and_deadline_payload\(\)/);
});

test("I/O thread admission and retries cannot erase failed join evidence", () => {
  assert.match(helperRuntime, /pub io_thread_join_failed: bool/);
  assert.match(helperRuntime, /io_thread_join_failed: false/);
  assert.match(helperRuntime, /runtime\.io_thread_join_failed \|= !joined;/);
  assert.match(helperRuntime, /runtime\.io_thread_join_failed \|= !thread_joins_ok;/);
  assert.match(helperRuntime, /if runtime\.io_thread_join_failed \{\s*return false;/);
  assert.match(helperRuntime, /fn panicked_io_thread_join_cannot_be_forgotten_by_retry_stop\(\)/);

  assert.match(helperRuntime, /pub fn spawn_stderr_logger\([\s\S]*?\) -> Result<thread::JoinHandle<\(\)>, String>/);
  assert.match(helperRuntime, /worker:stderr_logger_spawn_failed/);
  assert.match(helperRuntime, /worker:stdout_reader_spawn_failed/);
  assert.match(helper, /let logger = match spawn_stderr_logger\(stderr, stderr_log_path\.clone\(\)\)/);
  assert.match(helper, /"helper_bridge:stderr_logger_spawn_failed"/);
  assert.match(helper, /return fail_start_after_spawn\(\s*&mut runtime,[\s\S]*?"helper_bridge:stderr_logger_spawn_failed"/);
});


test("worker restart admission is native, generation-scoped, and updater-safe", () => {
  const internal = helper.slice(
    helper.indexOf("fn start_helper_bridge_internal("),
    helper.indexOf("pub fn start_helper_bridge()"),
  );
  const publicStart = helper.slice(
    helper.indexOf("pub fn start_helper_bridge()"),
    helper.indexOf("pub(crate) fn shutdown_helper_bridge_for_app_exit()"),
  );
  assert.match(helper, /fn public_worker_start_has_idle_audio_owner\(\) -> bool/);
  assert.match(helper, /latest_runtime_session_state\(\)/);
  assert.match(helper, /state\.blocker == "runtime_session:no_active_session"/);
  assert.match(publicStart, /let _update_admission = match admit_app_runtime_operation\(\)/);
  assert.match(publicStart, /if !public_worker_start_has_idle_audio_owner\(\)/);
  assert.match(publicStart, /start_helper_bridge_internal\(true, None\)/);
  assert.match(internal, /if clear_outbound_pipeline && !public_worker_start_has_idle_audio_owner\(\)/);
  assert.match(internal, /if !worker_restart_generation_allowed\(recovery_generation\)/);
  assert.match(internal, /helper_restart_denied_stale_meeting_generation/);
  assert.match(internal, /helper_restart_denied_after_cleanup_generation_revoke/);

  const recheck = internal.indexOf("helper_restart_denied_after_cleanup_generation_revoke");
  const markerReset = internal.indexOf("clear_any_meeting_outbound_pipeline();");
  const spawn = internal.indexOf("let child = match command.spawn()");
  assert.ok(recheck > 0 && markerReset > recheck && spawn > markerReset,
    "a revoked Meeting generation cannot clear live admission or spawn a replacement worker");
  assert.match(transport, /start_helper_bridge_internal\(false, Some\(generation\)\)/);
  assert.match(helper, /fn direct_helper_start_cannot_restart_while_mic_test_owns_audio\(\)/);
  assert.match(helper, /fn update_installer_reservation_blocks_direct_helper_start\(\)/);
  assert.match(helper, /fn live_recovery_requires_current_generation_even_inside_helper_lock\(\)/);
});

test("Meeting Stop helper cancellation is bounded and only its matching retry may reap retained resources", () => {
  const cancel = helper.slice(
    helper.indexOf("pub fn cancel_helper_bridge_meeting_session("),
    helper.indexOf("// Worker status probes"),
  );
  assert.match(cancel, /if session_id\.is_empty\(\) \{/);
  assert.match(cancel, /clear_meeting_outbound_pipeline_for_session\(session_id\)/);
  assert.match(cancel, /match runtime\(\)\.try_lock\(\)/);
  assert.match(cancel, /Err\(TryLockError::WouldBlock\)/);
  assert.match(cancel, /helper_cancel_deferred_owned_mutex/);
  assert.match(cancel, /Err\(TryLockError::Poisoned\(_\)\)/);
  assert.doesNotMatch(cancel, /runtime\(\)\.lock\(\)/);
  assert.doesNotMatch(cancel, /clear_any_meeting_outbound_pipeline/);

  const scopedRelease = cancel.indexOf("runtime.pending_meeting_cleanup_session_id.as_deref() != Some(session_id)");
  const retryCleanup = cancel.indexOf("if !stop_child(&mut runtime)");
  const activeMatch = cancel.indexOf("runtime.active_meeting_session_id.as_deref() == Some(session_id)");
  assert.ok(scopedRelease > 0 && scopedRelease < retryCleanup && retryCleanup < activeMatch,
    "Retry Stop may not reap an unrelated helper before exact session attribution");
  assert.match(cancel, /helper_cancel_rejected_unattributed_or_other_session_cleanup/);
  assert.match(cancel, /helper_bridge:cleanup_completed/);
  assert.match(cancel, /helper_bridge:meeting_session_hard_cancelled/);

  const cleanup = helperRuntime.slice(
    helperRuntime.indexOf("fn stop_child_with_budget("),
    helperRuntime.indexOf("#[cfg(test)]"),
  );
  assert.match(helperRuntime, /pub pending_meeting_cleanup_session_id: Option<String>/);
  assert.match(cleanup, /runtime\.pending_meeting_cleanup_session_id = runtime\.active_meeting_session_id\.clone\(\)/);
  assert.doesNotMatch(cleanup, /runtime\.pending_meeting_cleanup_session_id = None;/);
  assert.match(helper, /owner\.pending_meeting_cleanup_session_id = None;/);
  assert.match(meetingLifecycle, /let helper_cancel = cancel_helper_bridge_meeting_session\(&session_id\)/);
  assert.match(meetingLifecycle, /helper_cancel\.ok/);
  assert.match(meetingLifecycle, /mark_runtime_session_cleanup_incomplete/);
  for (const fixture of [
    "meeting_stop_defers_if_canonical_helper_mutex_is_busy_without_clearing_owner",
    "retry_stop_reaps_only_the_exact_session_that_owned_incomplete_cleanup",
    "unattributed_incomplete_helper_cleanup_cannot_be_claimed_by_meeting_stop",
    "incomplete_helper_cleanup_retains_meeting_owner_after_active_request_clears",
  ]) {
    assert.ok(helperCancellationTests.includes("fn " + fixture)
      || helperRuntime.includes("fn " + fixture), fixture + " must remain covered");
  }
});

test("no-snapshot Meeting orphan must be reconciled before native Stop and Close can succeed", () => {
  assert.match(helper, /pub\(crate\) fn meeting_helper_orphan_cleanup_pending\(\) -> bool/);
  assert.match(helper, /pub\(crate\) fn finish_orphaned_meeting_helper_without_session\(\) -> Result<\(\), &'static str>/);
  assert.match(helper, /helper_orphan:owner_busy/);
  assert.match(helper, /helper_orphan:cleanup_incomplete/);
  assert.match(helper, /let meeting_owned = owner\.pending_meeting_cleanup_session_id\.is_some\(\)/);
  assert.match(helper, /if !meeting_owned \{/);
  assert.match(helper, /helper_bridge:orphan_meeting_cleanup_verified/);

  const helperShutdown = helper.slice(
    helper.indexOf("pub(crate) fn shutdown_helper_bridge_for_app_exit()"),
    helper.indexOf("pub fn cancel_helper_bridge_meeting_session("),
  );
  assert.match(helperShutdown, /runtime\.pending_meeting_cleanup_session_id\.is_some\(\)/);
  assert.match(helperShutdown, /runtime\.active_meeting_session_id\.is_some\(\)/);

  const stop = meetingLifecycle.slice(
    meetingLifecycle.indexOf("pub(super) fn stop_meeting_translation_impl()"),
    meetingLifecycle.indexOf("pub(super) fn meeting_cleanup_complete("),
  );
  const orphanCleanup = stop.indexOf("finish_orphaned_meeting_helper_without_session()");
  const orphanFailed = stop.indexOf("if orphan_helper_cleanup.is_err()");
  const alreadyStopped = stop.indexOf('state: "already_stopped".to_string()');
  assert.ok(orphanCleanup >= 0 && orphanCleanup < orphanFailed && orphanFailed < alreadyStopped);
  assert.match(stop, /STOP_CLEANUP_PENDING\.store\(true, Ordering::Release\)/);
  assert.match(meetingLifecycle, /meeting_helper_orphan_cleanup_pending\(\)/);
  assert.match(meetingLifecycle, /inactive_writer_may_need_stop\(\)/);
  assert.match(meetingLifecycle, /inactive_meeting_sound_capture_may_need_stop\(\)/);
  assert.match(meetingSound, /pub fn inactive_meeting_sound_capture_may_need_stop\(\) -> bool/);
  assert.match(meetingSound, /store\.try_lock\(\)\.map\(\|guard\| guard\.is_some\(\)\)\.unwrap_or\(true\)/);

  assert.match(resources, /let orphan_cleanup = !meeting\.has_session && meeting\.lifecycle == "cleanup_incomplete"/);
  assert.match(applicationLifecycle, /if meeting\.lifecycle == "cleanup_incomplete" \{/);
  assert.match(closePolicy, /snapshot\.lifecycle === "cleanup_incomplete"/);
  assert.match(closePolicy, /snapshot\.lifecycle === "checking"/);
  assert.match(closeController, /verdict\.orphanCleanup/);
  assert.match(shutdown, /meeting_stop_cleanup_pending\(\)/);
  assert.match(shutdown, /shutdown_helper\(\) && mark_clean\(\)/);
  assert.match(helperRuntime, /pub pending_meeting_cleanup_session_id: Option<String>/);
  assert.match(helper, /let meeting_cleanup_pending =\s*runtime\.pending_meeting_cleanup_session_id\.is_some\(\)/);
  assert.match(helper, /helper_start_denied_orphan_meeting_cleanup/);
  assert.match(helper, /helper_bridge:stale_generation_during_startup/);
  const workerStart = helper.slice(
    helper.indexOf("fn start_helper_bridge_internal("),
    helper.indexOf("pub fn start_helper_bridge()"),
  );
  const lateGenerationCheck = workerStart.lastIndexOf("if !worker_restart_generation_allowed(recovery_generation)");
  const handoff = workerStart.indexOf("runtime.pending_meeting_cleanup_session_id = None;");
  const workerReady = workerStart.indexOf('runtime.state = "ready".to_string();');
  assert.ok(lateGenerationCheck >= 0 && handoff > lateGenerationCheck && workerReady > handoff,
    "a revoked Live generation during Python cold startup may not clear orphan attribution or report Ready");
  assert.match(helperRuntime, /reaping I\/O cannot release the owning Meeting's Stop transaction/);

  for (const fixture of [
    "orphaned_meeting_helper_blocks_native_close_until_no_snapshot_retry_stop",
    "no_snapshot_meeting_stop_does_not_terminate_standalone_worker",
    "no_snapshot_orphan_cleanup_never_waits_for_busy_helper_lock",
    "orphan_helper_cleanup_does_not_take_over_active_mic_test",
    "public_worker_restart_cannot_erase_orphaned_meeting_attribution",
  ]) {
    assert.ok(helperCancellationTests.includes("fn " + fixture), fixture);
  }
});

test("orphan Meeting consumers and playback keep exact native ownership through Retry Stop and Close", () => {
  assert.match(meetingLifecycle, /pub\(super\) fn verified_idle_for_orphan_cleanup\(\) -> bool/);
  assert.match(meetingLifecycle, /inactive_meeting_consumers_may_need_stop\(\)/);
  assert.match(meetingLifecycle, /inactive_meeting_playback_may_need_stop\(\)/);

  for (const [source, readName, finishName] of [
    [meetingConsumers, "inactive_meeting_consumers_may_need_stop", "finish_orphaned_meeting_consumers_without_session"],
    [meetingPlayback, "inactive_meeting_playback_may_need_stop", "finish_orphaned_meeting_playback_without_session"],
  ] as const) {
    assert.ok(source.includes("pub(super) fn " + readName + "() -> bool"));
    assert.ok(source.includes("pub(super) fn " + finishName + "() -> Result<(), String>"));
    assert.match(source, /join_failed: bool/);
    assert.match(source, /active\.join_failed = true;/);
    assert.match(source, /owner\.is_some\(\)\)\.unwrap_or\(true\)/);
    assert.match(source, /super::lifecycle::verified_idle_for_orphan_cleanup\(\)/);
  }
  assert.match(meetingConsumers, /outbound\.map\(stop_meeting_outbound_consumer\)/);
  assert.match(meetingConsumers, /incoming\.as_deref\(\)\.map\(stop_meeting_incoming_consumer\)/);
  assert.match(meetingConsumers, /outbound_consumer_store\(\)\.try_lock\(\)/);
  assert.match(meetingConsumers, /if !producer_cleared \{/);
  assert.match(meetingConsumers, /if !producer_cleared \|\| deferred_cleanup\.is_err\(\)/);
  assert.match(meetingConsumers, /active\.thread\.take\(\)\.is_some_and\(\|handle\| handle\.join\(\)\.is_err\(\)\)/);
  assert.match(meetingConsumers, /incoming_consumer_store\(\)\.try_lock\(\)/);
  assert.match(meetingPlayback, /playback_runtime_store\(\)\.try_lock\(\)/);
  assert.match(meetingPlayback, /stop_meeting_playback_runtime\(generation\)/);

  const idleStop = meetingLifecycle.slice(
    meetingLifecycle.indexOf("let Some(snapshot) = current.snapshot.as_ref() else {"),
    meetingLifecycle.indexOf("let generation = snapshot.generation;"),
  );
  const consumers = idleStop.indexOf("let orphan_consumers_cleanup = finish_orphaned_meeting_consumers_without_session()");
  const playback = idleStop.indexOf("let orphan_playback_cleanup = finish_orphaned_meeting_playback_without_session()");
  const deferred = idleStop.indexOf("let deferred_cleanup = if orphan_consumers_cleanup.is_ok()");
  const completed = idleStop.indexOf('state: "already_stopped".to_string()');
  assert.ok(consumers >= 0 && playback > consumers && deferred > playback && completed > deferred);
  assert.match(idleStop, /if orphan_consumers_cleanup\.is_ok\(\) && orphan_playback_cleanup\.is_ok\(\)/);
  assert.match(idleStop, /\|\| orphan_consumers_cleanup\.is_err\(\)/);
  assert.match(idleStop, /\|\| orphan_playback_cleanup\.is_err\(\)/);
  assert.match(idleStop, /STOP_CLEANUP_PENDING\.store\(true, Ordering::Release\)/);
  assert.match(meetingLifecycle, /let outbound_cleanup = stop_meeting_outbound_consumer\(generation\)/);
  assert.match(meetingLifecycle, /let playback_cleanup = stop_meeting_playback_runtime\(generation\)/);
  assert.match(meetingLifecycle, /let incoming_cleanup = stop_meeting_incoming_consumer\(&session_id\)/);
  assert.match(shutdown, /meeting_stop_cleanup_pending\(\)/);
  assert.match(shutdown, /shutdown_helper\(\) && mark_clean\(\)/);
  assert.match(closePolicy, /snapshot\.lifecycle === "cleanup_incomplete"/);

  for (const name of [
    "failed_outbound_join_cannot_be_forgotten_by_retry_stop",
    "already_joined_outbound_consumer_can_retry_scoped_producer_cleanup",
    "no_snapshot_stop_reaps_only_the_old_outbound_consumer_generation",
    "live_mic_test_prevents_orphan_consumer_cleanup",
    "panicked_playback_join_remains_owned_across_retry_stop",
    "no_snapshot_stop_reaps_only_the_old_playback_generation",
  ]) {
    assert.ok(meetingConsumers.includes("fn " + name + "()")
      || meetingPlayback.includes("fn " + name + "()"), name);
  }
});
