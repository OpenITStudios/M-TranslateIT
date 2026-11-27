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
  assert.match(boot, /applicationController\.refreshFromApplication\(\s*initialSnapshot,\s*"initial_shell",/);
  assert.equal((boot.match(/applicationRuntimeApi\.getSnapshot\(\)/g) ?? []).length, 1);
  assert.doesNotMatch(boot, /runtimeApi\.loadSettings\(/);
  assert.match(boot, /applicationController\.applySettings\(initialSnapshot\.settings\)/);
  assert.doesNotMatch(boot, /await refreshSnapshot\(/);
  const shellReady = boot.indexOf("booting = false;");
  assert.ok(shellReady >= 0 && shellReady < boot.indexOf('dispatchIntent("ensure_runtime_ready")'));
  assert.match(boot, /initialSnapshot\?\.helper\.state === "not_started"/);
  assert.match(boot, /applicationController\.read\(\)\.snapshot\?\.helper\?\.state === "not_started"/);
  assert.match(boot, /if \(outcome\.snapshot\.revision > 0 && outcome\.snapshot\.lifecycle !== "unavailable"\)/);
  assert.match(boot, /refreshFromApplicationEvent\(outcome\.snapshot, outcome\.intent\)/);
  assert.match(boot, /if \(!outcome\.ok\) \{\s*setKnownNotice\(outcome\.message, "attention"\)/);
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

test("native Meeting and Mic Test receipts reuse the exact application sequence", () => {
  const meeting = app.slice(app.indexOf("async function handleMeetingAction("), app.indexOf("async function runSetupAction("));
  const mic = app.slice(app.indexOf("async function toggleMicTest("), app.indexOf("async function pollMeeting("));
  assert.match(facade, /snapshot: result\.snapshot,\s*reason: result\.intent/);
  assert.match(meeting, /result\.snapshot\.revision <= 0 \|\| result\.snapshot\.lifecycle === "unavailable"/);
  assert.match(meeting, /refreshFromApplication\(\s*result\.snapshot, result\.reason/);
  assert.match(meeting, /current\.applicationRevision !== result\.snapshot\.revision/);
  assert.doesNotMatch(meeting, /applyMeetingStatus\(result\.status|void refreshMeetingLibrary\(/);
  assert.match(mic, /refreshFromApplication\(\s*result\.snapshot, result\.intent/);
  assert.match(mic, /result\.snapshot\.revision <= 0 \|\| result\.snapshot\.lifecycle === "unavailable"/);
});
