import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const reader = readFileSync(
  new URL("../../src/app/runtime/meetingReconcileReader.ts", import.meta.url),
  "utf8",
);
const controller = readFileSync(
  new URL("../../src/app/runtime/meetingLiveController.ts", import.meta.url),
  "utf8",
);

test("reconciliation uses one read and rejects non-Meeting owners before presenting turns", () => {
  assert.match(reader, /status\.owner_id !== APPLICATION_MEETING_OWNER_ID/);
  assert.equal((reader.match(/runtimeApi\.getMeetingCommittedTurns\(/g) ?? []).length, 1);
  assert.doesNotMatch(reader, /getMeetingSessionStatus\(/);
  const backend = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/committed_turns.rs", import.meta.url), "utf8");
  const read = backend.slice(backend.indexOf("pub(super) fn current_committed_turn_snapshot("));
  assert.ok(read.indexOf("session.owner_id != APPLICATION_MEETING_OWNER_ID") < read.indexOf("store.read_live("));
  assert.ok(read.indexOf("status.generation != Some(session.generation)") < read.indexOf("store.read_live("));
});

test("newer action/application snapshot invalidates stale Meeting-only reads", () => {
  const app = readFileSync(
    new URL("../../src/App.svelte", import.meta.url),
    "utf8",
  );
  assert.match(app, /if \(!result\.applied\) return;[\s\S]*meetingViewState = meetingLiveController\.invalidate\(\)/);
  assert.match(app, /const actionControllerRevision = applicationController\.read\(\)\.revision/);
  assert.match(app, /if \(applicationController\.read\(\)\.revision !== actionControllerRevision\)/);
  assert.match(app, /const pollControllerRevision = applicationController\.read\(\)\.revision/);
  assert.match(app, /if \(applicationController\.read\(\)\.revision !== pollControllerRevision\)/);
  assert.match(controller, /callbacks\.onStatus\(result\.status\);\s*\/\/[^\n]*[\s\S]*?if \(revision !== state\.revision\) return publish\(\);\s*const overlayRevision/);
});

test("session-scoped transcript fallback never reuses foreign or failed turns", () => {
  assert.match(reader, /currentTurns\?\.ok && currentTurns\.has_session/);
  assert.match(reader, /currentTurns\.session_id === status\.session_id \? currentTurns : null/);
  assert.match(reader, /nextTurns\.live_revision === retainedTurns\.live_revision/);
  assert.match(reader, /!nextTurns\.has_session \|\| nextTurns\.session_id !== status\.session_id/);
  assert.match(reader, /turns: retainedTurns,/);
  assert.match(reader, /if \(!unchanged\) return \{ status, turns: nextTurns/);
  assert.doesNotMatch(reader, /currentTurns \?\? nextTurns/);
  assert.match(controller, /publishLatestMeetingOverlay\(\s*result\.turns,/);
});

test("event and fallback share the authoritative revision read without timestamp heuristics", () => {
  assert.match(reader, /currentTurns\.live_revision \?\? null/);
  assert.doesNotMatch(reader + controller, /forceTurns|transcriptStatusKey/);
  assert.doesNotMatch(reader, /updated_unix_ms|utterance_sequence/);
  const bridge = readFileSync(new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url), "utf8");
  assert.match(bridge, /\{ reconcile: knownRevision === undefined \? null : \{ known_revision: knownRevision \} \}/);
});

test("saved transcript DTO stays flat and live-only metadata belongs to its read", () => {
  const bridge = readFileSync(new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url), "utf8");
  const backend = readFileSync(new URL("../../src-tauri/src/commands/meeting_session.rs", import.meta.url), "utf8");
  const saved = bridge.match(/export type MeetingCommittedTurnsSnapshot\s*=\s*\{([^{}]*)\};/)?.[1];
  assert.ok(saved);
  assert.doesNotMatch(saved, /live_revision|unchanged|status:/);
  assert.match(bridge, /MeetingLiveTurnsSnapshot = MeetingCommittedTurnsSnapshot & \{\s*live_revision: string \| null;/);
  assert.match(bridge, /MeetingLiveTurnsRead = MeetingLiveTurnsSnapshot & \{\s*unchanged: boolean;\s*status\?: MeetingSessionStatus;/);
  assert.match(backend, /#\[serde\(flatten\)\]\s*pub snapshot: MeetingCommittedTurnsSnapshot,/);
  assert.match(backend, /pub live_revision: Option<String>,\s*pub unchanged: bool,\s*#\[serde\(skip_serializing_if = "Option::is_none"\)\]\s*pub status: Option<MeetingSessionStatus>,/);
  assert.match(backend, /get_meeting_committed_turns\(reconcile: Option<MeetingLiveReadRequest>\)/);
  assert.match(backend, /struct MeetingLiveReadRequest \{\s*known_revision: Option<String>,/);
  assert.match(bridge, /live_revision: null,\s*unchanged: false,/);
});

test("the superseded standalone Meeting status IPC is retired but the internal owner remains", () => {
  const bridge = readFileSync(new URL("../../src/app/bridge/runtimeApi.ts", import.meta.url), "utf8");
  const registry = readFileSync(new URL("../../src-tauri/src/commands/registry.rs", import.meta.url), "utf8");
  const backend = readFileSync(new URL("../../src-tauri/src/commands/meeting_session.rs", import.meta.url), "utf8");
  assert.doesNotMatch(bridge, /getMeetingSessionStatus|get_meeting_session_status/);
  assert.doesNotMatch(registry, /::get_meeting_session_status,/);
  assert.doesNotMatch(backend, /#\[tauri::command\]\s*pub fn get_meeting_session_status/);
  assert.match(backend, /pub fn get_meeting_session_status\(\) -> MeetingSessionStatus \{\s*current_status\(\)/);
  assert.match(registry, /::get_meeting_committed_turns,/);
});
