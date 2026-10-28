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

