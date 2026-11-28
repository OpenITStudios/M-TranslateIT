import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const rust = readFileSync(
  new URL("../../src-tauri/src/commands/helper_bridge_runtime.rs", import.meta.url),
  "utf8",
);
const frontend = readFileSync(
  new URL("../../src/app/bridge/workerCapabilities.ts", import.meta.url),
  "utf8",
);

const readinessKeys = [
  "asr",
  "translation_id_en",
  "translation_en_id",
  "voice_actor_tts",
] as const;

test("Rust owns all native ASR, MT and voice capability status", () => {
  for (const key of readinessKeys) {
    assert.match(frontend, new RegExp(`readiness\\["${key}"\\]`), `TypeScript missing ${key}`);
  }

  for (const key of readinessKeys) {
    assert.match(rust, new RegExp(`"${key}"\\s*:`), `Rust missing ${key}`);
  }
  assert.match(rust, /fn candidate_status_payload\(/);
  assert.match(rust, /"voice_actor_tts": voice_ready/);
  assert.doesNotMatch(rust, /fn meeting_compute_preflight_admitted\(/);
});

test("worker preflight stage remains the capability response discriminator", () => {
  assert.match(frontend, /payload\["stage"\] === "local_realtime_worker_preflight"/);
  assert.match(rust, /stage == "local_realtime_worker_preflight"/);
});

test("frontend diagnostic loaded-state fields exist in Rust projected status", () => {
  for (const key of [
    "asr",
    "asr_device",
    "asr_compute_type",
    "asr_model_id",
    "translation_directions",
    "voice_actor",
    "voice_actor_device",
  ]) {
    assert.match(rust, new RegExp(`"${key}"\\s*:`), `Rust loaded status missing ${key}`);
  }
});

const diagnosticsView = readFileSync(
  new URL("../../src/components/settings/SettingsDiagnostics.svelte", import.meta.url),
  "utf8",
);

test("GPU diagnostics use existing worker status and do not alter readiness", () => {
  assert.match(frontend, /gpu\["torch_allocator"\]/);
  assert.match(frontend, /payload\["stage"\] === "local_realtime_worker_preflight"/);
  assert.match(frontend, /Number\.isSafeInteger/);
  assert.match(frontend, /allocated > reserved/);
  assert.match(frontend, /peakAllocated < allocated/);
  assert.match(frontend, /free > total/);
  assert.match(diagnosticsView, /workerDiagnostics\.torchMemoryDisplay/);
  assert.match(diagnosticsView, /workerDiagnostics\.deviceMemoryDisplay/);
  assert.match(diagnosticsView, /Read-only snapshot from Check Again/);
  assert.doesNotMatch(diagnosticsView, /setInterval|setTimeout/);
});

const productFacade = readFileSync(
  new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url),
  "utf8",
);
const applicationController = readFileSync(
  new URL("../../src/app/runtime/applicationController.ts", import.meta.url),
  "utf8",
);
const rustBridge = readFileSync(
  new URL("../../src-tauri/src/commands/helper_bridge/transport.rs", import.meta.url),
  "utf8",
);

test("worker status and GPU diagnostics are scoped to the same helper generation", () => {
  assert.match(rustBridge, /generation_token: runtime\.generation_token/);
  assert.match(productFacade, /observedWorkerStatus\?\.task === "status"/);
  assert.match(productFacade, /helper\.state === "ready"/);
  assert.match(
    productFacade,
    /observedWorkerStatus\.generation_token === helper\.generation_token/,
  );
  assert.match(
    productFacade,
    /const workerStatus = [\s\S]*?\? observedWorkerStatus\s*:\s*null;/,
  );
  assert.match(
    productFacade,
    /mapProductReadiness\(\{[\s\S]*?helper,[\s\S]*?workerStatus,[\s\S]*?inputStatus,/,
  );
  assert.match(productFacade, /return \{[\s\S]*?workerStatus,[\s\S]*?inputStatus,/);
});

test("explicit diagnostics refresh is not served from the event-reuse cache", () => {
  // A manual status refresh uses the standalone snapshot path (no previous/reason);
  // event-only reuse is conditional and cannot make it an unconditional cache.
  assert.match(productFacade, /const refreshWorker = !previous/);
  assert.match(productFacade, /!WORKER_REUSE_SAFE_REASONS\.has\(reason \?\? ""\)/);
  assert.match(
    productFacade,
    /refreshWorker\s*\?\s*\(helper\.state === "ready"\s*\? runtimeApi\.helperBridgeWorkerStatus\(\)/,
  );
  assert.match(
    productFacade,
    /export async function loadProductRuntimeSnapshot\([\s\S]*?return mapApplicationSnapshotToProduct\([\s\S]*?true,\s*\);/,
  );
  assert.match(
    applicationController,
    /async refresh\(knownSettings\?: RuntimeSettings\)[\s\S]*?loadProductRuntimeSnapshot\(knownSettings\)/,
  );
});

test("candidate readiness updates runtime directly and emits JSON only for explicit status", () => {
  const project = rust.slice(rust.indexOf("pub(super) fn project_candidate_status("), rust.indexOf("pub fn apply_worker_response("));
  assert.equal((project.match(/ProjectPaths::discover\(\)/g) ?? []).length, 1);
  assert.match(project, /if admitted \{\s*let paths = .*ProjectPaths::discover\(\)/);
  assert.match(project, /NativeInferenceOwner::assets_present_at\(&paths\)/);
  assert.match(project, /selected_native_identity\(&paths\)/);
  assert.match(project, /include_payload\.then\(\|\| candidate_status_payload/);
  assert.match(project, /runtime\.provider_ready = readiness\.ready/);
  assert.doesNotMatch(project, /worker_bool\(&projected|worker_text\(&projected/);
  const startup = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge.rs", import.meta.url), "utf8");
  const native = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge/transport/native.rs", import.meta.url), "utf8");
  assert.match(startup, /project_candidate_status\(&mut runtime, false\)/);
  assert.match(native, /project_candidate_status\(&mut host, true\)/);
});
