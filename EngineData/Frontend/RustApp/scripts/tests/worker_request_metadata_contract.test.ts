import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const read = (path: string) => readFileSync(new URL("../../src-tauri/src/commands/" + path, import.meta.url), "utf8");
const policy = read("helper_bridge/request_policy.rs");
const transport = read("helper_bridge/transport.rs");
const native = read("helper_bridge/transport/native.rs");
const startup = read("helper_bridge.rs");

test("native requests retain scheduler identity, exact Meeting scope and a bounded deadline", () => {
  assert.match(transport, /acquire_scoped_helper_task_permit\(priority, wait_generation\)/);
  assert.match(transport, /permit\.request_id\(\)/);
  assert.match(native, /checked_add\(Duration::from_millis\(ms\)\)/);
  assert.match(native, /scope\(payload\)/);
  assert.match(native, /body\["request_id"\] = json!\(request_id\)/);
  assert.match(native, /body\["scheduler_priority"\] = json!\(priority\.label\(\)\)/);
  for (const field of ["meeting_generation","meeting_session_id","meeting_lane","meeting_start_prepare"]) {
    assert.ok(policy.includes(field));
  }
  assert.doesNotMatch(startup + transport, /Command::new|resolve_worker_python_command|begin_worker_stdin_write|begin_worker_stdout_read/);
});
