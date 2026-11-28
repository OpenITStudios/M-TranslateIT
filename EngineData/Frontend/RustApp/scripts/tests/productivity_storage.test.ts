import assert from "node:assert/strict";
import test from "node:test";
import { clearTranslationFeedback, readTranslationFeedback } from "../../src/app/runtime/translationFeedbackState.ts";

class MemoryStorage {
  values = new Map<string, string>();
  getItem(key: string) { return this.values.get(key) ?? null; }
  setItem(key: string, value: string) { this.values.set(key, String(value)); }
  removeItem(key: string) { this.values.delete(key); }
  clear() { this.values.clear(); }
}
const memory = new MemoryStorage();
Object.defineProperty(globalThis, "localStorage", { value: memory, configurable: true });

test("legacy translation issue reports remain private and bounded until user clears them", () => {
  memory.clear();
  const key = "translateit.translation-feedback.v1";
  const malformed = JSON.stringify([{ category: "bad" }]);
  localStorage.setItem(key, malformed);
  assert.deepEqual(readTranslationFeedback(), { state: "attention", entries: [] });
  assert.equal(localStorage.getItem(key), malformed);
  const original = JSON.stringify(Array.from({ length: 45 }, (_, index) => ({
    id: String(index), createdAt: index,
    category: "unnatural", source: `source-${index}`, translation: `translation-${index}`,
    sourceLanguage: "id", targetLanguage: "en",
  })));
  localStorage.setItem(key, original);
  const report = readTranslationFeedback();
  assert.equal(report.state, "attention");
  assert.equal(report.entries.length, 40);
  assert.equal(report.entries[0]?.source, "source-0");
  assert.equal(localStorage.getItem(key), original, "reading never rewrites historical reports");
  localStorage.setItem("unrelated", "keep");
  assert.equal(clearTranslationFeedback(), true);
  assert.deepEqual(readTranslationFeedback(), { state: "empty", entries: [] });
  assert.equal(localStorage.getItem(key), null);
  assert.equal(localStorage.getItem("unrelated"), "keep");
});
