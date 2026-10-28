import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const main = readFileSync(new URL("../../src-tauri/src/main.rs", import.meta.url), "utf8");
const bootstrap = readFileSync(new URL("../../src-tauri/src/app_bootstrap.rs", import.meta.url), "utf8");
const guard = readFileSync(new URL("../../src-tauri/src/app_bootstrap/single_instance.rs", import.meta.url), "utf8");

test("Windows single-instance admission precedes every Tauri setup/runtime owner", () => {
  const admission = main.indexOf("single_instance::acquire()");
  const builder = main.indexOf("tauri::Builder::default()");
  assert.ok(admission > 0 && builder > admission);
  assert.match(main, /let _single_instance = match app_bootstrap::single_instance::acquire\(\)/);
  assert.match(main, /InstanceAdmission::Primary\(guard\) => guard/);
  assert.match(main, /InstanceAdmission::Existing => return/);
  assert.match(main, /show_startup_error\(&error\)/);
  assert.ok(main.indexOf("app.run(") > builder, "guard must live through event loop");
  assert.match(bootstrap, /#\[cfg\(target_os = "windows"\)\]\s*pub\(crate\) mod single_instance;/);
  assert.match(bootstrap, /begin_startup_recovery/);
});

test("OS-owned mutex prevents duplicate startup without a stale file or extra service", () => {
  assert.match(guard, /CreateMutexW\(/);
  assert.match(guard, /GetLastError\(\) \} == ERROR_ALREADY_EXISTS/);
  assert.match(guard, /if mutex\.is_null\(\) \{\s*return Err\(io::Error::last_os_error\(\)\)/);
  assert.match(guard, /impl Drop for PrimaryInstance[\s\S]*CloseHandle\(self\.mutex\)/);
  assert.match(guard, /if existed \{[\s\S]*CloseHandle\(mutex\)[\s\S]*restore_existing_main\(\)[\s\S]*InstanceAdmission::Existing/);
  assert.match(guard, /current_exe\(\)/);
  assert.match(guard, /canonicalize\(process\.exe\(\)\?\)/);
  assert.match(guard, /EnumWindows\(/);
  assert.match(guard, /GetWindowTextW\(/);
  assert.match(guard, /ShowWindow\(search\.found, SW_RESTORE\)/);
  assert.match(guard, /SetForegroundWindow\(search\.found\)/);
  assert.doesNotMatch(guard, /create_dir|write\(|remove_file|thread::spawn|sleep\(/);
});
