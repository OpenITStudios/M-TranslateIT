import assert from "node:assert/strict";
import test from "node:test";

import { resolveMeetingVoiceGate } from "../../src/app/runtime/meetingVoiceGate.ts";
import { mapProductReadiness } from "../../src/app/bridge/productReadinessState.ts";
import type { HelperBridgeStatus } from "../../src/app/shared/types.ts";
test("busy helper lock is Checking, not a fabricated Ready or worker failure", () => {
  const pending = mapProductReadiness({
    helper: {
      state: "checking",
      runtime_claim: "helper_bridge:status_lock_busy",
      message: "Worker status could not be read during an owned task.",
      generation_token: 0,
      provider_ready: false,
      cuda_ready: false,
      functional_outbound_ready: false,
      last_error: null,
    } as HelperBridgeStatus,
    inputStatus: null,
    meetingSession: null,
    approvedVoiceReady: null,
  });
  assert.equal(pending.level, "checking");
  assert.equal(pending.meetingReady, false);
  assert.equal(pending.asrReady, false);
  assert.equal(pending.translationIdEnReady, false);
  assert.equal(pending.translationEnIdReady, false);
  assert.equal(pending.ttsReady, false);
  assert.equal(pending.helperStatus, "Checking local AI worker");
  assert.match(pending.summary, /not yet verified/);
  assert.match(pending.nextAction, /must be confirmed/);
  assert.equal(pending.blockers.some((entry) => entry.includes("lock_poisoned")), false);
});

test("preflight-ready Meeting stays Setup Needed until a Meeting voice is selected", () => {
  const result = resolveMeetingVoiceGate({
    live: false,
    preflightReady: true,
    selectedVoiceReady: false,
  });
  assert.equal(result.meetingReady, false);
  assert.equal(result.status, "Setup Needed");
  assert.equal(result.blocker, "meeting_voice:not_selected");
  assert.match(result.nextAction ?? "", /Choose a Meeting voice/);
  assert.doesNotMatch(result.summary ?? "", /My Voice isn't ready/);
});

test("unknown Meeting voice readiness remains Checking instead of claiming Ready", () => {
  const result = resolveMeetingVoiceGate({
    live: false,
    preflightReady: true,
    selectedVoiceReady: null,
  });
  assert.equal(result.meetingReady, false);
  assert.equal(result.status, "Checking");
  assert.equal(result.blocker, null);
  assert.match(result.nextAction ?? "", /Checking the selected Meeting voice/);
});

test("selected Meeting voice completes preflight readiness", () => {
  const result = resolveMeetingVoiceGate({
    live: false,
    preflightReady: true,
    selectedVoiceReady: true,
  });
  assert.equal(result.meetingReady, true);
  assert.equal(result.status, "Ready");
  assert.equal(result.blocker, null);
});

test("an authoritative live Meeting remains ready if a later voice probe is unavailable", () => {
  const result = resolveMeetingVoiceGate({
    live: true,
    preflightReady: true,
    selectedVoiceReady: null,
  });
  assert.equal(result.meetingReady, true);
  assert.equal(result.blocker, null);
});

test("voice state does not hide or duplicate an unmet preflight owner", () => {
  const result = resolveMeetingVoiceGate({
    live: false,
    preflightReady: false,
    selectedVoiceReady: false,
  });
  assert.equal(result.meetingReady, false);
  assert.equal(result.status, "Setup Needed");
  assert.equal(result.blocker, null);
  assert.equal(result.nextAction, null);
});
