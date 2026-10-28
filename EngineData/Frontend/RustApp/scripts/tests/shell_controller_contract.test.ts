import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
const meeting = readFileSync(
  new URL("../../src/app/runtime/meetingLiveController.ts", import.meta.url),
  "utf8",
);
const close = readFileSync(
  new URL("../../src/app/runtime/closeController.ts", import.meta.url),
  "utf8",
);
const facade = readFileSync(
  new URL("../../src/app/bridge/runtimeProductFacade.ts", import.meta.url),
  "utf8",
);

test("Meeting live coordination is owned outside App.svelte", () => {
  assert.match(app, /createMeetingLiveController\(\)/);
  assert.match(meeting, /readMeetingReconciliation/);
  assert.match(meeting, /publishLatestMeetingOverlay/);
  assert.equal(app.includes("let meetingPollInFlight"), false);
  assert.equal(app.includes("let lastTranscriptStatusKey"), false);
  assert.equal(app.includes("let lastOverlayMeetingRevision"), false);
});

test("close flow coordination is owned outside App.svelte", () => {
  assert.match(app, /createCloseController\(\)/);
  assert.match(close, /resolveNativeCloseVerdict/);
  assert.match(close, /stopAndResolveNativeClose/);
  for (const stale of [
    "closeDialogOpen",
    "closeDialogTitle",
    "closeDialogMessage",
    "closeDialogAction",
  ]) {
    assert.equal(app.includes(stale), false, stale);
  }
  assert.match(app, /onOpenChange=\{\(open\) =>/);
});


test("returning startup renders the native shell before cold worker initialization", () => {
  const onMountStart = app.indexOf("  onMount(() => {");
  const bootStart = app.indexOf("    const boot = async () => {", onMountStart);
  const bootEnd = app.indexOf("    const installCloseGuard = async () => {", bootStart);
  assert.ok(onMountStart >= 0 && bootStart > onMountStart && bootEnd > bootStart);
  const boot = app.slice(bootStart, bootEnd);
  assert.match(boot, /await applicationRuntimeApi\.getSnapshot\(\)/);
  assert.match(boot, /applicationController\.refreshFromApplication\(\s*initialSnapshot,\s*"initial_shell",\s*loadedSettings/);
  assert.doesNotMatch(boot, /await refreshSnapshot\(/);
  assert.ok(boot.indexOf("booting = false;") < boot.indexOf('dispatchIntent("ensure_runtime_ready")'));
  assert.match(boot, /initialSnapshot\?\.helper\.state === "not_started"/);
  assert.match(boot, /applicationController\.read\(\)\.snapshot\?\.helper\?\.state === "not_started"/);
  assert.match(boot, /if \(outcome\.snapshot\.revision > 0 && outcome\.snapshot\.lifecycle !== "unavailable"\)/);
  assert.match(boot, /refreshFromApplicationEvent\(outcome\.snapshot, "startup_worker_result"\)/);
  assert.match(boot, /if \(!outcome\.ok\) \{\s*setNotice\(outcome\.message\)/);
  assert.doesNotMatch(boot, /setInterval|setTimeout|retry_blocked_worker/);
});

test("read-heavy My Voice build status is offloaded from native command dispatch", () => {
  const native = readFileSync(new URL("../../src-tauri/src/commands/voice_lab_build.rs", import.meta.url), "utf8");
  assert.match(native, /pub async fn get_voice_lab_build_status\(\) -> Result<VoiceLabBuildStatus, String>/);
  assert.match(native, /spawn_blocking\(current_status\)/);
  assert.match(native, /My Voice status collection was interrupted/);
  assert.match(native, /fn current_status\(\) -> VoiceLabBuildStatus/);
});

test("runtime product facade remains a thin compatibility surface", () => {
  assert.match(facade, /from "\.\/productAudioFacade"/);
  assert.doesNotMatch(facade, /productTranslationFacade|runProductTranslation/);
  assert.match(facade, /from "\.\/productSetupFacade"/);
  assert.doesNotMatch(facade, /async function probeProductAudioDevice/);
  assert.doesNotMatch(facade, /async function runProductTranslationAlternative/);
});
