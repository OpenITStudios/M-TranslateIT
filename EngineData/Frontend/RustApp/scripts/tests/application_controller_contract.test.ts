import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const controller = readFileSync(
  new URL("../../src/app/runtime/applicationController.ts", import.meta.url),
  "utf8",
);
const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");

test("application controller owns the product runtime snapshot", () => {
  assert.match(controller, /snapshot: ProductRuntimeSnapshot \| null/);
  assert.match(controller, /async refresh\(initialApplication\?: ApplicationSnapshot\)/);
  assert.match(controller, /async refreshFromApplication/);
  assert.match(controller, /loadProductRuntimeSnapshotFromApplication/);
  assert.match(controller, /applySettings\(settings: RuntimeSettings\)/);
  assert.match(controller, /applyMeetingSession/);
  assert.match(controller, /ownershipChanged/);
  assert.match(controller, /previousOwner/);
  assert.match(controller, /previousLocked/);
});

test("App shell consumes one controller snapshot instead of subsystem copies", () => {
  assert.match(app, /createApplicationController\(defaultSettings\(\)\)/);
  assert.match(app, /const snapshot = \$derived\(applicationState\.snapshot\)/);
  assert.equal(app.includes("let runtimeSettings = $state"), false);
  assert.equal(app.includes("let helperStatus = $state"), false);
  assert.equal(app.includes("let workerStatus = $state"), false);
  assert.equal(app.includes("let inputStatus = $state"), false);
  assert.equal(app.includes("let approvedVoiceReady = $state"), false);
});

test("controller owns setup state transitions", () => {
  assert.match(controller, /meeting_setup_state === "new"/);
  assert.match(controller, /setupRequired/);
  assert.equal(app.includes("let setupRequired = $state"), false);
});

const architectureVerifier = readFileSync(new URL("../validate_application_runtime_architecture.mjs", import.meta.url), "utf8");
const rustApplicationContract = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/contract.rs", import.meta.url), "utf8");
const rustApplicationLifecycle = readFileSync(new URL("../../src-tauri/src/commands/application_runtime/lifecycle.rs", import.meta.url), "utf8");
const productRuntimeFacade = readFileSync(new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url), "utf8");

test("ApplicationRuntime source boundaries reject UI dependencies without matching status strings", () => {
  assert.match(rustApplicationLifecycle, /frontend_bridge_error/);
  assert.match(architectureVerifier, /frontend::/);
  assert.doesNotMatch(architectureVerifier, /WindowBuilder\|@tauri\|frontend\|svelte/);
});

test("WorkerSummary is validated within its own type, not unrelated audio readiness", () => {
  assert.match(rustApplicationContract, /pub struct WorkerSummary/);
  assert.match(rustApplicationContract, /pub process_ready: bool/);
  assert.match(rustApplicationContract, /pub struct AudioSummary/);
  assert.match(rustApplicationContract, /pub ready: bool/);
  assert.match(architectureVerifier, /const workerSummaryFields/);
});

test("read-only worker diagnostics are not mistaken for lifecycle mutation", () => {
  assert.match(productRuntimeFacade, /runtimeApi\.helperBridgeWorkerStatus\(\)/);
  assert.doesNotMatch(architectureVerifier, /"runtimeApi\.helperBridgeWorkerStatus\("/);
});


test("new setup commits the native sequence without a discarded worker projection", () => {
  const start = controller.indexOf("async refreshFromApplication(");
  const body = controller.slice(start, controller.indexOf("applySettings(settings:", start));
  assert.match(body, /requireApplicationSnapshot\(application\)/);
  assert.match(body, /application\.revision <= state\.applicationRevision/);
  const setup = body.indexOf('application.settings.meeting_setup_state === "new"');
  const commit = body.indexOf("return commitSnapshot({", setup);
  const map = body.indexOf("loadProductRuntimeSnapshotFromApplication(", commit);
  assert.ok(setup >= 0 && commit > setup && map > commit);
  assert.match(body.slice(commit, map), /applicationRevision: application\.revision,\s*settings: application\.settings/);
  assert.match(controller, /applicationRevision: next\.applicationRevision/);
  assert.match(controller, /initialApplication\.revision < state\.applicationRevision/);
});
