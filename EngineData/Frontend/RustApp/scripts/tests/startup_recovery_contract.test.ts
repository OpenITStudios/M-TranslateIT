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
  assert.match(helper, /if runtime\.last_error\.as_deref\(\) == Some\("helper_bridge:cleanup_incomplete"\)/);
  assert.match(helper, /if !stop_child\(&mut runtime\) \{[\s\S]*?"Meeting helper Stop has not verified/);
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
