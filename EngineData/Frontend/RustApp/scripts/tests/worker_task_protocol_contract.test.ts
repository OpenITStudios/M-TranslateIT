import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const rust = readFileSync(
  new URL("../../src-tauri/src/commands/helper_bridge_runtime.rs", import.meta.url),
  "utf8",
);
function rustDeadlineTasks(): string[] {
  const match = rust.match(
    /pub fn worker_response_deadline_ms\(task: &str\)[\s\S]*?match task \{([\s\S]*?)\n\s*_ =>/,
  );
  assert.ok(match, "Rust worker deadline router must exist");
  const values = new Set<string>();
  for (const arm of match[1].matchAll(/([^=]+)=>/g)) {
    for (const item of arm[1].matchAll(/"([a-z_][a-z_0-9]*)"/g)) values.add(item[1]);
  }
  return [...values].sort();
}

test("Rust deadlines cover the single native task set", () => {
  const native = ["asr_preload", "transcribe", "translate", "translation_preload"];
  const transport = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge/transport.rs", import.meta.url), "utf8");
  assert.deepEqual(rustDeadlineTasks(), [...native, "ping", "status", "voice_actor_preflight", "voice_actor_synthesize"].sort());
  assert.match(transport, /return native::dispatch\(/);
  assert.match(transport, /return native::control\(/);
  assert.doesNotMatch(transport, /begin_worker_stdin_write|begin_worker_stdout_read/);
});

test("unknown Rust worker tasks remain bounded by a fallback deadline", () => {
  assert.match(rust, /_ => WORKER_FALLBACK_RESPONSE_DEADLINE_MS/);
});

test("native runtime has no legacy pipe framing or worker process", () => {
  assert.doesNotMatch(rust, /ChildStdin|ChildStdout|read_worker_response|begin_worker_stdin_write/);
});
