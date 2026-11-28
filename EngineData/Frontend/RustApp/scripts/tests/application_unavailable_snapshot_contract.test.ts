import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const facade = readFileSync(
  new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url),
  "utf8",
);
const api = readFileSync(
  new URL("../../src/app/bridge/applicationRuntimeApi.ts", import.meta.url),
  "utf8",
);

test("unavailable ApplicationSnapshot is rejected before product settings mapping", () => {
  assert.match(api, /revision: 0/);
  assert.match(api, /lifecycle: "unavailable"/);

  const start = facade.indexOf("export function requireApplicationSnapshot");
  const guardBody = facade.slice(start, facade.indexOf("export function shouldBootstrapLocalRuntime", start));
  assert.match(guardBody, /application\.revision <= 0 \|\| application\.lifecycle === "unavailable"/);
  assert.match(guardBody, /throw new Error\("TranslateIT application runtime is unavailable\."\)/);
  const mapper = facade.slice(facade.indexOf("async function mapApplicationSnapshotToProduct"));
  assert.match(mapper, /requireApplicationSnapshot\(application\);\s*const settings = application\.settings;/);
  assert.doesNotMatch(facade, /knownSettings/);
});

test("explicit bootstrap validates its replacement receipt before mapping", () => {
  const start = facade.indexOf("export async function loadProductRuntimeSnapshot(");
  const body = facade.slice(start, facade.indexOf("export async function loadProductRuntimeSnapshotFromApplication", start));
  assert.match(body, /initialApplication \?\? await applicationRuntimeApi\.getSnapshot\(\)/);
  assert.match(body, /requireApplicationSnapshot\(application\);\s*if \(shouldBootstrapLocalRuntime\(application\)\)/);
  assert.match(body, /application = \(await prepareLocalRuntime\(application\.revision\)\)\.snapshot;/);
  const preparation = facade.slice(facade.indexOf("export async function prepareLocalRuntime("), facade.indexOf("async function mapApplicationSnapshotToProduct("));
  assert.match(preparation, /dispatchIntent\("ensure_runtime_ready"\);\s*requireApplicationSnapshot\(outcome\.snapshot\);\s*if \(outcome\.snapshot\.revision <= initialRevision\)/);
  assert.match(preparation, /throw new Error\("TranslateIT runtime preparation returned a stale snapshot\."\)/);
  assert.match(preparation, /return outcome;/);
  assert.match(body, /return mapApplicationSnapshotToProduct\(application\)/);
});

test("failed product intent returns a fail-closed Meeting status, never an empty IPC DTO", () => {
  const fallback = readFileSync(
    new URL("../../src/app/runtime/meetingBridgeFallback.ts", import.meta.url),
    "utf8",
  );
  const mapper = readFileSync(
    new URL("../../src/app/bridge/productMeetingState.ts", import.meta.url),
    "utf8",
  );
  const consumer = readFileSync(
    new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url),
    "utf8",
  );
  const app = readFileSync(
    new URL("../../src/App.svelte", import.meta.url),
    "utf8",
  );

  assert.match(api, /import \{ meetingSessionStatusFallback \} from "\.\.\/runtime\/meetingBridgeFallback";/);
  assert.match(api, /meeting: meetingSessionStatusFallback\("Application runtime is unavailable\."\)/);
  assert.doesNotMatch(api, /meeting:\s*\{\}\s+as\s+MeetingSessionStatus/);
  assert.doesNotMatch(api, /settings:\s*\{\}\s+as\s+RuntimeSettings/);
  assert.doesNotMatch(api, /helper:\s*\{\}\s+as\s+HelperBridgeStatus/);
  assert.match(api, /settings: defaultSettings\(\)/);
  for (const field of [
    "cuda_ready", "provider_ready", "functional_outbound_ready",
    "functional_outbound_verified_unix_ms", "generation_token",
    "active_task", "active_request_id", "active_meeting_generation",
    "active_meeting_session_id", "active_meeting_lane", "runtime_claim",
  ]) {
    assert.ok(api.includes(`${field}:`), `unavailable helper missing ${field}`);
  }
  assert.match(api, /state: "frontend_bridge_error"/);
  assert.match(api, /last_error: "frontend_bridge_unavailable"/);
  assert.match(api, /snapshot: unavailableSnapshot\(\)/);
  assert.match(consumer, /const status = result\.snapshot\.meeting/);
  assert.match(mapper, /const unavailable = meetingBridgeUnavailable\(status\)/);
  assert.match(mapper, /const canStart = !unavailable/);
  assert.match(mapper, /const canStop = !unavailable/);
  assert.match(fallback, /lifecycle: "unavailable"/);
  assert.match(fallback, /has_session: true/);
  assert.match(fallback, /runtime_claim: "frontend_bridge_unavailable"/);
  assert.match(fallback, /ready_for_start: false/);
  assert.match(fallback, /start_eligible: false/);
  assert.match(app, /result\.meeting\.lifecycle === "unavailable" \|\| result\.state === "frontend_bridge_error"/);
  assert.match(app, /Meeting status is unknown/);
});

test("helper fallback preserves every canonical Rust/TypeScript field without claiming readiness", () => {
  const rust = readFileSync(
    new URL("../../src-tauri/src/commands/helper_bridge_runtime.rs", import.meta.url),
    "utf8",
  );
  const types = readFileSync(
    new URL("../../src/app/shared/types.ts", import.meta.url),
    "utf8",
  );
  const rustBlock = rust.match(/pub struct HelperBridgeStatus\s*\{([\s\S]*?)\n\}/)?.[1];
  const tsBlock = types.match(/export type HelperBridgeStatus\s*=\s*\{([\s\S]*?)\n\};/)?.[1];
  const fallbackBlock = api.match(/^    helper: \{([\s\S]*?)^    \},/m)?.[1];
  assert.ok(rustBlock);
  assert.ok(tsBlock);
  assert.ok(fallbackBlock);

  const rustFields = [...rustBlock.matchAll(/^\s*pub\s+([a-z_][a-z_0-9]*)\s*:/gm)]
    .map((match) => match[1]).sort();
  const tsFields = [...tsBlock.matchAll(/^  ([a-z_][a-z_0-9]*)\s*:/gm)]
    .map((match) => match[1]).sort();
  const fallbackFields = [...fallbackBlock.matchAll(/^      ([a-z_][a-z_0-9]*)\s*:/gm)]
    .map((match) => match[1]).sort();

  assert.deepEqual(tsFields, rustFields);
  assert.deepEqual(fallbackFields, rustFields);
  assert.match(fallbackBlock, /cuda_ready: false/);
  assert.match(fallbackBlock, /provider_ready: false/);
  assert.match(fallbackBlock, /functional_outbound_ready: false/);
  assert.match(fallbackBlock, /state: "frontend_bridge_error"/);
  assert.match(fallbackBlock, /runtime_claim: "frontend_bridge_unavailable"/);
});
