import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { readTranslationFeedback } from "../../src/app/runtime/translationFeedbackState.ts";

const outbound = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/outbound_pipeline.rs", import.meta.url), "utf8");
const incoming = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_pipeline.rs", import.meta.url), "utf8");

test("retired translation style and terminology never enter production model requests", () => {
  for (const source of [outbound, incoming]) {
    assert.doesNotMatch(source, /"translation_style":|\"terminology\":|\"context_pairs\":/);
  }
});

test("retired spoken terms never bias production Meeting ASR requests", () => {
  for (const source of [outbound, incoming]) {
    assert.doesNotMatch(source, /"hotwords":|asr_hotwords\(\)/);
  }
});

test("retired translation style never enters incoming deferred queue or replay", () => {
  const deferred = readFileSync(new URL("../../src-tauri/src/commands/meeting_session/incoming_deferred.rs", import.meta.url), "utf8");
  assert.doesNotMatch(incoming, /translation_style/);
  assert.doesNotMatch(deferred, /translation_style/);
  assert.match(incoming, /deferred_direction: Option<\(String, String\)>/);
  assert.match(deferred, /MAX_DEFERRED_INCOMING_AGE_MS: u64 = 20_000/);
});


test("legacy feedback is read-only except for explicit confirmed clearing", () => {
  const state = readFileSync(new URL("../../src/app/runtime/translationFeedbackState.ts", import.meta.url), "utf8");
  const panel = readFileSync(new URL("../../src/components/settings/TranslationFeedbackReview.svelte", import.meta.url), "utf8");
  assert.match(state, /translateit\.translation-feedback\.v1/);
  assert.doesNotMatch(state, /saveTranslationFeedback|\.setItem\(/);
  assert.match(panel, /\{#if report\.state !== "empty"\}/);
  assert.match(panel, /navigator\.clipboard\.writeText/);
  assert.match(panel, /\{#if confirmClear\}/);
  assert.match(panel, /onclick=\{clearAll\}/);
  assert.match(state, /localStorage\.removeItem\(KEY\)/);
});

test("legacy feedback reader distinguishes empty, valid, damaged and inaccessible data", () => {
  const read = (raw: string | null) => readTranslationFeedback({ getItem: () => raw });
  const valid = { id: "old", category: "meaning_changed", source: "Selamat pagi", translation: "Good evening", sourceLanguage: "id", targetLanguage: "en", createdAt: 123 };
  assert.deepEqual(read(null), { state: "empty", entries: [] });
  assert.deepEqual(read("[]"), { state: "empty", entries: [] });
  assert.deepEqual(read(JSON.stringify([valid])), { state: "ready", entries: [valid] });
  assert.deepEqual(read(JSON.stringify([valid, { ...valid, createdAt: 1e30 }])), { state: "attention", entries: [valid] });
  assert.deepEqual(read("broken"), { state: "attention", entries: [] });
  assert.deepEqual(read("x".repeat(2 * 1024 * 1024 + 1)), { state: "attention", entries: [] });
  assert.deepEqual(readTranslationFeedback({ getItem: () => { throw Error("blocked"); } }), { state: "unavailable", entries: [] });
  const bounded = read(JSON.stringify(Array.from({ length: 41 }, (_, i) => ({ ...valid, id: String(i) }))));
  assert.equal(bounded.state, "attention");
  assert.equal(bounded.entries.length, 40);
});
