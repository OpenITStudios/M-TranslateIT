import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import {
  DEFAULT_TRANSCRIPT_PREFERENCES,
  TRANSCRIPT_PREFERENCES_KEY,
  normalizeTranscriptPreferences,
  readTranscriptPreferences,
  updateTranscriptPreferences,
} from "../../src/components/meeting/transcriptPreferences.ts";

function withStorage(run: (items: Map<string, string>) => void, rejectWrites = false): void {
  const items = new Map<string, string>();
  const original = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  const storage = {
    get length() { return items.size; },
    getItem(key: string) { return items.get(key) ?? null; },
    setItem(key: string, value: string) {
      if (rejectWrites) throw new Error("Storage denied");
      items.set(key, value);
    },
    removeItem(key: string) { items.delete(key); },
    clear() { items.clear(); },
    key(index: number) { return [...items.keys()][index] ?? null; },
  };
  try {
    Object.defineProperty(globalThis, "localStorage", { value: storage, configurable: true });
    run(items);
  } finally {
    if (original) Object.defineProperty(globalThis, "localStorage", original);
    else Reflect.deleteProperty(globalThis, "localStorage");
  }
}

test("transcript preferences accept only enumerated display fields", () => {
  assert.deepEqual(normalizeTranscriptPreferences({
    density: "huge", textOrder: "invalid", timestamps: "never",
    showSpeakerIcons: "false", followLatest: null, showPlaybackStatus: 0,
    privateMeeting: "never persisted",
  }), DEFAULT_TRANSCRIPT_PREFERENCES);
  assert.deepEqual(normalizeTranscriptPreferences(null), DEFAULT_TRANSCRIPT_PREFERENCES);
});

test("transcript choices roundtrip in local UI storage without conversation data", () => {
  withStorage((items) => {
    const result = updateTranscriptPreferences({
      textMode: "translation-only", density: "compact", textOrder: "translation-first", timestamps: "hover",
      showSpeakerIcons: false, followLatest: false, showPlaybackStatus: false,
    });
    assert.equal(result.ok, true);
    assert.deepEqual(readTranscriptPreferences(), result.preferences);
    const raw = items.get(TRANSCRIPT_PREFERENCES_KEY) ?? "";
    assert.equal(Object.keys(JSON.parse(raw) as object).length, 7);
    assert.equal(raw.includes("source_text"), false);
    assert.equal(raw.includes("meeting audio"), false);
  });
});

test("invalid or corrupt stored settings recover to safe defaults", () => {
  withStorage((items) => {
    items.set(TRANSCRIPT_PREFERENCES_KEY, "{bad json");
    assert.deepEqual(readTranscriptPreferences(), DEFAULT_TRANSCRIPT_PREFERENCES);
    items.set(TRANSCRIPT_PREFERENCES_KEY, '{"density":"compact","timestamps":"unknown"}');
    assert.deepEqual(readTranscriptPreferences(), {
      ...DEFAULT_TRANSCRIPT_PREFERENCES, density: "compact",
    });
  });
});

test("unwritable local UI storage cannot produce a fake saved state", () => {
  withStorage(() => {
    const result = updateTranscriptPreferences({ density: "compact" });
    assert.equal(result.ok, false);
    assert.deepEqual(result.preferences, DEFAULT_TRANSCRIPT_PREFERENCES);
    assert.deepEqual(readTranscriptPreferences(), DEFAULT_TRANSCRIPT_PREFERENCES);
  }, true);
});

test("all transcript readers share settings; F2 remains read-only", () => {
  const root = new URL("../../src/", import.meta.url);
  const activity = readFileSync(new URL("components/meeting/MeetingActivity.svelte", root), "utf8");
  const last = readFileSync(new URL("components/meeting/MeetingSessionReview.svelte", root), "utf8");
  const saved = readFileSync(new URL("components/meeting/MeetingTranscriptExport.svelte", root), "utf8");
  const bubble = readFileSync(new URL("components/meeting/MeetingTurnBubble.svelte", root), "utf8");
  const settings = readFileSync(new URL("pages/Settings.svelte", root), "utf8");
  const frame = readFileSync(new URL("../../scripts/ui-preview/FrameApp.svelte", import.meta.url), "utf8");
  for (const reader of [activity, last, saved]) {
    assert.match(reader, /subscribeTranscriptPreferences\(/);
    assert.match(reader, /preferences=\{appearance\}/);
  }
  assert.match(activity, /if \(previewReadOnly\) return;/);
  assert.match(frame, /previewReadOnly=\{true\}/);
  assert.match(bubble, /preferences\.textOrder/);
  assert.match(bubble, /preferences\.textMode/);
  assert.match(bubble, /preferences\.timestamps/);
  assert.match(bubble, /preferences\.showSpeakerIcons/);
  assert.match(bubble, /preferences\.showPlaybackStatus \|\| criticalStatus/);
  assert.match(settings, /<TranscriptAppearanceSettings \{onNotice\}/);
});
