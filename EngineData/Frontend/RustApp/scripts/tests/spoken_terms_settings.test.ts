import assert from "node:assert/strict";
import test from "node:test";

import { cloneSettings, defaultSettings } from "../../src/app/shared/state.ts";

test("settings defaults and cloning discard retired AI preferences", () => {
  const settings = defaultSettings();
  const legacy = {
    ...settings,
    translation_style: "formal",
    terminology: [{ indonesian: "arsip", english: "UNTRUSTED_TERM" }],
    spoken_terms: [{ term: "UNTRUSTED_HOTWORD", aliases: [] }],
    context_pairs: [["OLD_SOURCE", "OLD_TARGET"]],
  };
  const cloned = cloneSettings(legacy);
  for (const retired of ["translation_style", "terminology", "spoken_terms", "context_pairs"]) {
    assert.equal(Object.hasOwn(settings, retired), false);
    assert.equal(Object.hasOwn(cloned, retired), false);
  }
  assert.equal(settings.schema_version, 12);
  assert.deepEqual(cloned, settings);
  cloned.audio.noise_suppression = "off";
  assert.equal(settings.audio.noise_suppression, "auto");
});
