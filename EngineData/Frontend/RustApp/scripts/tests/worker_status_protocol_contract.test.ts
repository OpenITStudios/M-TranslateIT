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
    /refreshWorker\s*\?\s*\(helper\.state === "ready"\s*\? await runtimeApi\.helperBridgeWorkerStatus\(\)/,
  );
  assert.match(
    productFacade,
    /export async function loadProductRuntimeSnapshot\([\s\S]*?return mapApplicationSnapshotToProduct\(application\);/,
  );
  assert.match(
    applicationController,
    /async refresh\(initialApplication\?: ApplicationSnapshot\)[\s\S]*?loadProductRuntimeSnapshot\(initialApplication\)/,
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

test("functional preparation refreshes capability inside the bind permit without a separate status task", () => {
  const native = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge/transport/native.rs", import.meta.url), "utf8");
  const probe = readFileSync(new URL("../../src-tauri/src/commands/helper_bridge/functional_probe.rs", import.meta.url), "utf8");
  const bind = native.slice(native.indexOf("let reference = if is_bind {"), native.indexOf("if host.native_inference.is_none()"));
  const refresh = bind.indexOf("project_candidate_status(&mut host, false)");
  const gate = bind.indexOf("if !host.provider_ready");
  const previous = bind.indexOf("let previous_device");
  const close = bind.indexOf("host.release_idle_native_inference()");
  const reference = bind.indexOf("selected_native_reference(&paths)");
  const initialScope = bind.indexOf("!scope_is_current(&request_scope)");
  const meetingOwner = bind.indexOf("host.native_meeting_session_id().is_some()");
  const scopeBeforeRelease = bind.indexOf("!scope_is_current(&request_scope)", reference);
  const scopeAfterRelease = bind.indexOf("!scope_is_current(&request_scope)", close);
  const open = bind.indexOf("NativeInferenceOwner::open_voice_candidate(");
  const compare = bind.indexOf("observed != owner.owner.device_id()");
  const retain = bind.indexOf("host.native_inference = Some(owner)");
  const reject = bind.indexOf("if device_changed");
  assert.ok(refresh >= 0 && refresh < gate && gate < initialScope
    && initialScope < meetingOwner && meetingOwner < reference
    && reference < scopeBeforeRelease && scopeBeforeRelease < previous && previous < close
    && close < scopeAfterRelease && scopeAfterRelease < open && open < compare
    && compare < retain && retain < reject);
  assert.match(bind.slice(reference, scopeBeforeRelease), /Err\(blocker\) => return fail\(blocker,/);
  assert.match(bind.slice(close, scopeAfterRelease), /return fail\("native_voice:owner_busy"/);
  assert.match(bind.slice(reject), /host\.provider_ready = false;[\s\S]*invalidate_required_outbound_ai_readiness\(\);[\s\S]*return fail\("native_inference:cuda_device_changed"/);
  assert.doesNotMatch(probe, /send_worker_task\(\s*"status"/);
  assert.match(probe, /response\.state == "ready" && response\.generation_token != 0/);
  assert.match(probe, /let generation_token = preparation\.generation_token/);
  for (const stage of ["translation", "voice", "asr_inference"]) {
    assert.match(probe, new RegExp("same_probe_inference\\(&" + stage + ", generation_token, native_device\\)"));
  }
  assert.match(probe, /current\.generation_token != generation_token/);
});
