import assert from "node:assert/strict";
import test from "node:test";
import {
  meetingSpeakerLabel,
  meetingSpeakerDescription,
  meetingFirstRecordedTime,
  meetingSessionTitle,
  meetingTime,
  meetingTranslationDisplay,
  formatMeetingTurn,
  formatMeetingTranscript,
  filterMeetingTurns,
} from "../../src/app/runtime/meetingTranscriptPresentation.ts";
import type { MeetingCommittedTurn } from "../../src/app/bridge/runtimeApi.ts";

test("You is distinct from the one mixed incoming audio lane, not fake diarization", () => {
  assert.equal(meetingSpeakerLabel("you"), "You");
  assert.equal(meetingSpeakerLabel("incoming"), "Meeting audio");
  assert.match(meetingSpeakerDescription("incoming"), /speakers are not identified/);
  assert.doesNotMatch(meetingSpeakerDescription("you"), /speakers are not identified/);
});
test("session presentation uses a real timestamp and has safe missing-date fallback", () => {
  assert.equal(meetingSessionTitle(null), "Meeting session");
  assert.equal(meetingTime(null), "Time unavailable");
  assert.match(meetingTime(Date.UTC(2026, 10, 8, 14, 32, 17)), /^\d{2}:\d{2}:\d{2}$/);
  assert.match(meetingSessionTitle(1791800000000), /^Meeting · /);
});
test("a last-session label uses the earliest retained turn, never reuses a prior meeting", () => {
  const turn = (stamp: number) => ({ created_unix_ms: stamp }) as MeetingCommittedTurn;
  assert.equal(meetingFirstRecordedTime([]), null);
  assert.equal(meetingFirstRecordedTime([turn(90), turn(60), turn(130)]), 60);
  assert.equal(meetingFirstRecordedTime([turn(0), turn(60)]), 60);
});

test("source-first transcript never manufactures a translated string", () => {
  assert.equal(meetingTranslationDisplay({ translation_state: "pending", translated_text: "" }), "Translating...");
  assert.equal(meetingTranslationDisplay({ translation_state: "failed", translated_text: "" }), "Translation unavailable");
  assert.equal(meetingTranslationDisplay({ translation_state: "translated", translated_text: "Try again" }), "Try again");
  assert.equal(meetingTranslationDisplay({ translation_state: "translated", translated_text: "" }), "Translation unavailable");
});


test("copy and search preserve original speech and explicitly unavailable translations", () => {
  const make = (sequence: number, source_text: string, translated_text: string, translation_state: string) => ({
    sequence, lane: "you", created_unix_ms: 0, source_language: "id", target_language: "en",
    source_text, translated_text, translation_state,
  }) as MeetingCommittedTurn;
  const first = make(1, "Selamat pagi", "Good morning", "translated");
  const failed = make(2, "Coba lagi", "", "failed");
  const turns = [first, failed];
  assert.equal(formatMeetingTurn(failed), "You · Time unavailable\nIndonesian: Coba lagi\nEnglish: Translation unavailable");
  assert.equal(formatMeetingTranscript(turns), [formatMeetingTurn(first), formatMeetingTurn(failed)].join("\n\n"));
  assert.deepEqual(filterMeetingTurns(turns, " GOOD MORNING "), [first]);
  assert.deepEqual(filterMeetingTurns(turns, "coba"), [failed]);
  assert.deepEqual(filterMeetingTurns(turns, ""), turns);
  assert.deepEqual(filterMeetingTurns(turns, "missing"), []);
  assert.equal(turns.length, 2);
});
