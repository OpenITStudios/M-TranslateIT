import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  appNotice,
  activeMeetingIssue,
  visibleAppNotice,
} from "../../src/app/runtime/applicationNotice.ts";
import type { ProductRuntimeSnapshot } from "../../src/app/bridge/runtimeProductTypes.ts";

function meeting(
  lifecycle: string,
  extra: Partial<ProductRuntimeSnapshot["meeting"]> = {},
  ownerKind = "meeting",
): ProductRuntimeSnapshot {
  return {
    meeting: {
      lifecycle,
      hasSession: lifecycle !== "idle",
      live: lifecycle === "live",
      outboundStage: "idle",
      ...extra,
    },
    resources: { owner_kind: ownerKind },
  } as unknown as ProductRuntimeSnapshot;
}

test("free-form messages remain neutral; uncertainty is explicit, not a confirmed fault", () => {
  assert.deepEqual(appNotice("Could not confirm the action"), {
    message: "Could not confirm the action",
    kind: "info",
  });
  assert.equal(appNotice("The action could not be verified", "unverified").kind, "unverified");
  assert.equal(appNotice("Meeting started", "success").kind, "success");
});

test("cleanup is native-owned and cannot be overwritten by routine status updates", () => {
  const blocked = meeting("cleanup_incomplete", { hasSession: false });
  const info = appNotice("Ready", "success");
  const nativeIssue = activeMeetingIssue(blocked);
  assert.equal(nativeIssue?.kind, "attention");
  assert.match(nativeIssue?.message ?? "", /Retry Stop/);
  assert.deepEqual(visibleAppNotice(info, blocked, null, null), nativeIssue);
  assert.equal(activeMeetingIssue(meeting("idle", { hasSession: false }, "none")), null);
  assert.deepEqual(visibleAppNotice(info, meeting("idle", { hasSession: false }, "none"), null, null), info);
});

test("uncertain Meeting ownership is never presented as a confirmed failure or clean stop", () => {
  assert.equal(activeMeetingIssue(meeting("unavailable", { hasSession: true }))?.kind, "unverified");
  assert.equal(activeMeetingIssue(meeting("ownership_unknown"))?.kind, "unverified");
  assert.equal(activeMeetingIssue(meeting("unavailable", { hasSession: false }, "none")), null);
  assert.equal(activeMeetingIssue(meeting("runtime_conflict"))?.kind, "attention");
  assert.equal(activeMeetingIssue(meeting("live", { outboundStage: "attention_needed" }))?.kind, "attention");
  const original = {
    ...meeting("live", { outboundStage: "attention_needed" }),
    meetingSession: { audio_source: "original" },
  } as unknown as ProductRuntimeSnapshot;
  assert.equal(activeMeetingIssue(original), null);
});

test("active health warning survives unrelated refresh, and clears only after its owner does", () => {
  const idle = meeting("idle", { hasSession: false }, "none");
  const info = appNotice("Diagnostics refreshed", "success");
  const health = appNotice("The microphone route needs attention.", "attention");
  const longSession = appNotice("Session health could not be checked.", "unverified");
  assert.deepEqual(visibleAppNotice(info, idle, health, longSession), health);
  assert.deepEqual(visibleAppNotice(info, idle, null, longSession), longSession);
  assert.deepEqual(visibleAppNotice(info, idle, null, null), info);
  assert.equal(visibleAppNotice(appNotice("Action failed", "error"), idle, health, null).kind, "error");
});

test("App shell and header keep one notice surface with scoped live-region semantics", () => {
  const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
  const header = readFileSync(new URL("../../src/components/layout/AppHeader.svelte", import.meta.url), "utf8");
  assert.match(app, /visibleAppNotice\(notice, snapshot, liveHealthNotice, longSessionNotice\)/);
  assert.match(app, /notice=\{displayedNotice\}/);
  assert.match(header, /notice: AppNotice/);
  assert.match(header, /role="alert" aria-atomic="true"/);
  assert.match(header, /role="status" aria-live="polite" aria-atomic="true"/);
  assert.doesNotMatch(app, /message\.includes\("error"\)/);
});
