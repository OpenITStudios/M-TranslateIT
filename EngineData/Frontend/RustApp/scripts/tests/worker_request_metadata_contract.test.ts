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

test("native synthesis borrows text and actor authority through admission and returns owned PCM", () => {
  assert.match(transport, /Synthesize \{ text: &\x27a str, actor_token: &\x27a str, samples: &\x27a mut Option<Vec<f32>> \}/);
  assert.match(native, /is_voice && \(payload\.get\("text"\)\.is_some\(\) \|\| payload\.get\("expected_actor_token"\)\.is_some\(\)\)/);
  assert.match(native, /actor_token\.expect\("checked token"\), preparing, \(\*text\)\.to_owned\(\), deadline/);
  assert.match(native, /token\.is_empty\(\) \|\| token\.len\(\) > 512/);
  assert.match(native, /\*\*slot = Some\(samples\)/);
  for (const caller of [read("meeting_session/outbound_pipeline.rs"), read("helper_bridge/functional_probe.rs")]) {
    const call = caller.slice(caller.indexOf("synthesize_native_voice(json!({"));
    assert.ok(call.startsWith("synthesize_native_voice(json!({"));
    const metadata = call.slice(0, call.indexOf("}),"));
    assert.doesNotMatch(metadata, /"text":|"expected_actor_token":/);
    assert.match(call, /\}\), &?translated_(?:text|fixture), &actor_token\)/);
  }
});

test("native scope fences observe borrowed session identity without building diagnostics", () => {
  const state = readFileSync(new URL("../../src-tauri/src/engine/runtime_state.rs", import.meta.url), "utf8");
  assert.match(state, /Some\(observe\(guard\.as_ref\(\)\)\)/);
  assert.doesNotMatch(native + policy, /latest_runtime_session_state\(\)/);
  assert.equal((native.match(/!scope_is_current\(payload, &request_scope\)/g) ?? []).length, 4);
  assert.match(native, /== Some\(\(expected\.generation, expected\.session_id\.as_deref\(\)\)\)/);
  assert.match(native, /\}\)\.unwrap_or\(false\)/);
  assert.match(transport, /let request_id = permit\.request_id\(\);/);
});
