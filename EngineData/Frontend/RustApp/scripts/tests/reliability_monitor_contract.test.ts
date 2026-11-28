import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const monitor = readFileSync(new URL("../../src/app/runtime/reliabilityMonitor.ts", import.meta.url), "utf8");
const app = readFileSync(new URL("../../src/App.svelte", import.meta.url), "utf8");
const schedule = readFileSync(new URL("../../src/app/runtime/meetingReconcileSchedule.ts", import.meta.url), "utf8");
const runtimeEvents = readFileSync(
  new URL("../../src-tauri/src/engine/runtime_events.rs", import.meta.url),
  "utf8",
);
const committedTurns = readFileSync(
  new URL("../../src-tauri/src/commands/meeting_session/committed_turns.rs", import.meta.url),
  "utf8",
);

test("live reliability monitor is Meeting-owner scoped and advisory only", () => {
  assert.match(app, /snapshot\?\.resources\.owner_kind === "meeting"/);
  assert.match(app, /startMeetingRuntimeMonitors/);
  assert.match(monitor, /getMeetingReliabilitySnapshot/);
  assert.doesNotMatch(monitor, /getRuntimeWatchdogStatus\(/);
  assert.doesNotMatch(monitor, /getDeviceLossGuardStatus\(/);
  assert.doesNotMatch(monitor, /getLongSessionHealthStatus/);
  assert.doesNotMatch(monitor, /stopMeetingTranslation|startMeetingTranslation|selectAudioDevice|startHelperBridge/);
});

test("live reliability uses one aggregated IPC and stays low frequency", () => {
  assert.match(monitor, /getMeetingReliabilitySnapshot/);
  assert.match(monitor, /function liveReliabilityNotice\(\{ watchdog, devices \}/);
  assert.doesNotMatch(monitor, /getLongSessionHealthStatus/);

  assert.match(monitor, /LIVE_RELIABILITY_POLL_MS = 8_000/);
  assert.doesNotMatch(monitor, /LONG_SESSION_POLL_MS|longSessionInFlight|longSessionTimer/);
  assert.match(monitor, /if \(key === lastKeys\[channel\]\) return/);
  assert.match(monitor, /disposed \|\| inFlight/);
  assert.equal((monitor.match(/await getMeetingReliabilitySnapshot\(\)/g) ?? []).length, 1);
});

test("Meeting freshness is event-first with slow reconciliation fallback", () => {
  assert.match(monitor, /MEETING_RUNTIME_EVENT = "translateit:\/\/meeting-runtime"/);
  assert.match(monitor, /listen<MeetingRuntimeEvent>/);
  assert.match(monitor, /MEETING_EVENT_COALESCE_MS = 80/);
  assert.match(monitor, /MEETING_RECONCILIATION_POLL_MS = 10_000/);
  assert.match(monitor, /createMeetingReconcileSchedule\(/);
  assert.match(monitor, /schedule\.request\(\)/);
  assert.match(monitor, /schedule\.dispose\(\)/);
  assert.doesNotMatch(monitor, /setInterval\([^;]*MEETING_RECONCILIATION_POLL_MS/);
  assert.match(schedule, /if \(disposed \|\| pending\) return/);
  assert.match(schedule, /if \(inFlight\) return/);
  assert.match(schedule, /arm\(pending \? eventWindowMs : fallbackMs\)/);
  assert.doesNotMatch(monitor, /forceTurns|pollMeeting\((?:true|false)\)/);
  assert.doesNotMatch(monitor, /setInterval\([^)]*1_200/);
});

test("backend publishes lightweight Meeting change tokens from authoritative turn state", () => {
  assert.match(runtimeEvents, /MEETING_RUNTIME_EVENT.*translateit:\/\/meeting-runtime/);
  assert.match(runtimeEvents, /revision/);
  assert.match(runtimeEvents, /session_id/);
  assert.match(runtimeEvents, /sequence/);
  assert.match(committedTurns, /emit_meeting_runtime_event\("turn_committed"/);
  assert.match(committedTurns, /emit_meeting_runtime_event\("delivery_state_changed"/);
  assert.doesNotMatch(runtimeEvents, /MeetingCommittedTurnsSnapshot|translated_text|source_text/);
});

test("long-session health is advisory and change-deduplicated", () => {
  assert.match(monitor, /pressure\.state !== "attention" \|\| pressure\.warning_count === 0/);
  assert.match(monitor, /lastKeys\[channel\]/);
  assert.match(monitor, /publish\("session", sessionPressureNotice\(observation\.pressure\)\)/);
  assert.doesNotMatch(monitor, /clearDeferred|cleanup.*temp|replay|restart/i);
});

test("completed reliability reads cannot notify after Meeting monitor disposal", () => {
  assert.match(monitor, /const observation = await getMeetingReliabilitySnapshot\(\);\s*\/\/[^\n]*\n\s*if \(disposed\) return;/);
  assert.match(monitor, /return \(\) => \{\s*disposed = true;\s*window.clearInterval\(timer\);/);
});

test("Meeting monitor lifetime is stable across status-only snapshot refreshes", () => {
  assert.ok(app.includes("const meetingMonitorsActive = $derived("));
  assert.ok(app.includes("const meetingReliabilityEnabled = $derived("));
  assert.match(app, /\$effect\(\(\) => \{\s*if \(!meetingMonitorsActive\)/);
  assert.match(app, /startMeetingRuntimeMonitors\(\s*pollMeeting,\s*updateHealthNotice,\s*meetingReliabilityEnabled/);
  assert.ok(monitor.includes('key: "reliability_unavailable"'));
  assert.ok(monitor.includes('key: "long_session_unavailable"'));
  assert.ok(app.includes("Couldn't refresh the meeting display"));
});

test("reliability warnings have typed severity and clear on their own healthy observation", () => {
  assert.match(monitor, /type ReliabilityNoticeCallback = \(channel: ReliabilityChannel, notice: ReliabilityNotice \| null\)/);
  assert.match(monitor, /const key = notice\?\.key \?\? ""/);
  assert.match(monitor, /onNotice\(channel, notice\)/);
  assert.match(monitor, /if \(!pressure \|\| pressure\.state === "unavailable"\)/);
  assert.match(monitor, /kind: "unverified"/);
  assert.match(monitor, /kind: "attention"/);
  assert.match(app, /liveHealthNotice = null;/);
  assert.match(app, /longSessionNotice = null;/);
});


test("combined reliability reuses fresh Meeting and device observations without a cache", () => {
  const commands = new URL("../../src-tauri/src/commands/", import.meta.url);
  const combined = readFileSync(new URL("reliability_snapshot.rs", commands), "utf8");
  const watchdog = readFileSync(new URL("runtime_watchdog.rs", commands), "utf8");
  const guard = readFileSync(new URL("device_loss_guard.rs", commands), "utf8");
  const route = readFileSync(new URL("virtual_mic_route.rs", commands), "utf8");
  assert.equal((combined.match(/get_meeting_session_status\(\)/g) ?? []).length, 1);
  assert.match(combined, /runtime_watchdog_for_meeting\(&meeting\)/);
  assert.match(combined, /device_loss_guard_for_meeting\(&meeting\)/);
  assert.match(combined, /session_pressure_for_meeting\(&meeting\)/);
  assert.match(combined, /record_pressure_if_needed\(&pressure\)/);
  assert.doesNotMatch(combined, /get_long_session_health_status|meeting_temp_file_count|get_recent_runtime_incidents/);
  assert.match(watchdog, /let meeting = get_meeting_session_status\(\);\s*runtime_watchdog_for_meeting\(&meeting\)/);
  assert.match(guard, /let meeting = get_meeting_session_status\(\);\s*device_loss_guard_for_meeting\(&meeting\)/);
  assert.equal((guard.match(/list_audio_devices\(\)/g) ?? []).length, 1);
  assert.match(guard, /get_virtual_mic_route_selection_with_devices\(&devices\)/);
  const reusedRoute = route.slice(route.indexOf("pub(super) fn get_virtual_mic_route_selection_with_devices("));
  assert.doesNotMatch(reusedRoute.slice(0, reusedRoute.indexOf("#[tauri::command]")), /list_audio_devices\(/);
  assert.match(route, /get_virtual_mic_route_selection_with_devices\(&list_audio_devices\(\)\)/);
  assert.equal((route.match(/list_audio_devices\(\)/g) ?? []).length, 2);
});
