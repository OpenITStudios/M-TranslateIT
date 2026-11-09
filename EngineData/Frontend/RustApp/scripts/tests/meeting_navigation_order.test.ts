import assert from "node:assert/strict";
import test from "node:test";
import { pinnedFirst, recentMeetings } from "../../src/components/layout/meetingNavigationOrder.ts";

type Sample = { session_id: string; pinned: boolean; project_id?: string | null };
const sessions: Sample[] = [
  { session_id: "meeting_011", pinned: false, project_id: "project_a" },
  { session_id: "meeting_010", pinned: true, project_id: "project_b" },
  { session_id: "meeting_009", pinned: false },
  { session_id: "meeting_008", pinned: false, project_id: "project_a" },
  { session_id: "meeting_007", pinned: true, project_id: "project_a" },
  { session_id: "meeting_006", pinned: false },
  { session_id: "meeting_005", pinned: false },
  { session_id: "meeting_004", pinned: false },
  { session_id: "meeting_003", pinned: true },
  { session_id: "meeting_002", pinned: false },
  { session_id: "meeting_001", pinned: false },
];
const ids = (items: Sample[]) => items.map(s => s.session_id);

test("Pinned sessions are first in Recent, followed by five newest ordinary sessions", () => {
  assert.deepEqual(ids(recentMeetings(sessions)), [
    "meeting_010", "meeting_007", "meeting_003",
    "meeting_011", "meeting_009", "meeting_008", "meeting_006", "meeting_005",
  ]);
});

test("Pinned sessions are never removed by the regular limit", () => {
  const allPinned = Array.from({ length: 13 }, (_, i) => ({
    session_id: `meeting_${i}`, pinned: true,
  }));
  assert.equal(recentMeetings(allPinned).length, 13);
  assert.deepEqual(ids(recentMeetings(sessions, 0)), [
    "meeting_010", "meeting_007", "meeting_003",
  ]);
});

test("Unpin restores native chronological ordering without mutating saved source", () => {
  const before = JSON.stringify(sessions);
  const changed = sessions.map(s => s.session_id === "meeting_010" ? { ...s, pinned: false } : s);
  assert.deepEqual(ids(recentMeetings(changed)), [
    "meeting_007", "meeting_003",
    "meeting_011", "meeting_010", "meeting_009", "meeting_008", "meeting_006",
  ]);
  assert.equal(JSON.stringify(sessions), before);
});

test("Every Project has an independent stable pinned-first ordering", () => {
  const projectA = sessions.filter(s => s.project_id === "project_a");
  const projectB = sessions.filter(s => s.project_id === "project_b");
  assert.deepEqual(ids(pinnedFirst(projectA)), ["meeting_007", "meeting_011", "meeting_008"]);
  assert.deepEqual(ids(pinnedFirst(projectB)), ["meeting_010"]);
});

test("Malformed recent limits reject without showing an arbitrary selection", () => {
  assert.throws(() => recentMeetings(sessions, -1), RangeError);
  assert.throws(() => recentMeetings(sessions, 1.5), RangeError);
});
