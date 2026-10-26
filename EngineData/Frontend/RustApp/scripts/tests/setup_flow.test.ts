import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  persistedSetupCheckpoint,
  safeSetupResumeStep,
  setupCheckpoint,
  setupStateNeedsResume,
} from "../../src/app/runtime/setupFlow.ts";

test("maps legacy five-step checkpoints into the compact four-step flow", () => {
  assert.equal(setupCheckpoint(1), 1);
  assert.equal(setupCheckpoint(2), 2);
  assert.equal(setupCheckpoint(3), 3);
  assert.equal(setupCheckpoint(4), 3);
  assert.equal(setupCheckpoint(5), 4);
  assert.equal(setupCheckpoint(99), 4);
});

test("writes compact step four using the legacy-safe persisted checkpoint", () => {
  assert.equal(persistedSetupCheckpoint(1), 1);
  assert.equal(persistedSetupCheckpoint(2), 2);
  assert.equal(persistedSetupCheckpoint(3), 3);
  assert.equal(persistedSetupCheckpoint(4), 5);
  assert.equal(setupCheckpoint(persistedSetupCheckpoint(4)), 4);
});

test("resume returns to the first unmet required setup owner", () => {
  assert.equal(
    safeSetupResumeStep(4, { microphoneReady: false, meetingRouteReady: false }),
    2,
  );
  assert.equal(
    safeSetupResumeStep(4, { microphoneReady: true, meetingRouteReady: false }),
    3,
  );
  assert.equal(
    safeSetupResumeStep(4, { microphoneReady: true, meetingRouteReady: true }),
    4,
  );
});

test("only deferred setup is treated as resumable", () => {
  assert.equal(setupStateNeedsResume("deferred"), true);
  assert.equal(setupStateNeedsResume("new"), false);
  assert.equal(setupStateNeedsResume("completed"), false);
});

const firstSetup = readFileSync(new URL("../../src/pages/FirstSetup.svelte", import.meta.url), "utf8");
const setupNavigation = readFileSync(new URL("../../src/components/setup/SetupNavigation.svelte", import.meta.url), "utf8");
const appShell = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");

test("setup exposes only Meeting setup and honest deferral", () => {
  assert.doesNotMatch(firstSetup, /openTextInstead|onOpenText|Translate text|destination/);
  assert.doesNotMatch(setupNavigation, /onOpenText|Translate text/);
  assert.match(setupNavigation, /Set up meeting translation/);
  assert.match(setupNavigation, /Set Up Later/);
  assert.match(firstSetup, /persistSetupFact\("deferred", step\)/);
});

test("finishing or deferring setup returns to Meeting without claiming readiness", () => {
  const start = appShell.indexOf("async function finishFirstSetup(");
  const end = appShell.indexOf("async function openMyVoiceFromSetup(", start);
  const entry = appShell.slice(start, end);
  assert.match(entry, /route = "meeting"/);
  assert.match(entry, /await refreshSnapshot/);
  assert.doesNotMatch(entry, /destination|text/);
});
