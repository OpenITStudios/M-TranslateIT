import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const meeting = source("src/pages/Meeting.svelte");
const settings = source("src/pages/Settings.svelte");
const native = source("src-tauri/src/commands/settings.rs");
const registry = source("src-tauri/src/commands/registry.rs");

test("Meeting presets cannot be applied through any UI or registered native command", () => {
  assert.doesNotMatch(native, /pub fn apply_meeting_preset|MeetingPresetApplyResult/);
  assert.doesNotMatch(registry, /apply_product_meeting_preset/);
  assert.doesNotMatch(meeting, /MeetingPresetPanel|meeting-presets/);
  assert.doesNotMatch(settings, /MeetingPresetPanel|Meeting presets/);
});

test("retired preference screens are not mounted", () => {
  assert.doesNotMatch(settings, /TranslationPreferences|Spoken terms|Preferred words|Translation style/);
  assert.match(settings, /Voice &amp; Audio/);
  assert.match(settings, /Meeting Captions/);
  assert.match(settings, /Help/);
});
