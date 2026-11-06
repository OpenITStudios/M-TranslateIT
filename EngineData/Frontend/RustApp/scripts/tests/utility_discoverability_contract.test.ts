import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

// Command Palette and its registry were retired; Quick Access is the current
// single-window owner, not a second command/navigation implementation.
const read = (path: string) =>
  readFileSync(new URL("../../src/" + path, import.meta.url), "utf8");

const app = read("App.svelte");
const quick = read("pages/QuickAccess.svelte");
const header = read("components/layout/AppHeader.svelte");
const settings = read("pages/Settings.svelte");
const meeting = read("pages/Meeting.svelte");
const sidebar = read("components/layout/Sidebar.svelte");
const firstSetup = read("pages/FirstSetup.svelte");
const brandComponent = read("components/brand/TranslateITMark.svelte");
const brandSvg = read("components/brand/translateit-mark.svg");

test("the one Quick Access entry is discoverable without retired command palette", () => {
  assert.match(header, /aria-label="Open Quick Access"/);
  assert.match(header, /toggleQuickAccess/);
  assert.match(app, /toggleQuickAccess/);
  assert.match(app, /event\.key\.toLowerCase\(\) === "k"/);
  assert.match(quick, /applicationRuntimeApi\.dispatchIntent/);
  assert.match(quick, /stop_meeting/);
  assert.doesNotMatch(app, /CommandPalette|productCommandRegistry/);
  assert.doesNotMatch(quick, /quickTranslateClipboard|meeting-presets/);
});

test("current Settings and Meeting use their canonical task surfaces", () => {
  assert.match(settings, /aria-label="Settings categories"/);
  assert.match(settings, /Voice &amp; Audio/);
  assert.match(settings, /Meeting Captions/);
  assert.doesNotMatch(settings, /TranslationPreferences/);
  assert.doesNotMatch(meeting, /<MeetingPresetPanel/);
});

test("Quick Access respects native action ownership and explicit Stop confirmation", () => {
  assert.match(quick, /const canStart = \$derived/);
  assert.match(quick, /const canStop = \$derived/);
  assert.match(quick, /const needsStop = \$derived/);
  assert.match(quick, /if \(stop && !confirmingStop\)/);
  assert.match(quick, /const foreignOwner = \$derived/);
});

test("one source-owned SVG mark is reused across desktop brand surfaces", () => {
  assert.match(brandSvg, /viewBox="0 0 48 48"/);
  assert.match(brandSvg, /#171B1F/);
  assert.match(brandSvg, /#F5F7FA/);
  assert.match(brandSvg, /#38BDF8/);
  assert.doesNotMatch(brandSvg, /<(?:script|image|foreignObject)\b|(?:href|url)\s*=|url\(/i);
  assert.match(brandComponent, /import markUrl from "\.\/translateit-mark\.svg"/);
  assert.match(brandComponent, /alt="" aria-hidden="true"/);
  assert.match(brandComponent, /width=\{size\} height=\{size\}/);
  assert.match(sidebar, /<TranslateITMark size=\{36\} \/>/);
  assert.match(firstSetup, /<TranslateITMark size=\{40\} \/>/);
  assert.match(quick, /<TranslateITMark size=\{20\} \/>/);
  assert.match(app, /<TranslateITMark size=\{48\} \/>/);
  assert.match(app, /<TranslateITMark size=\{40\} \/>/);
  assert.doesNotMatch(sidebar, />T<\/div>/);
  assert.doesNotMatch(firstSetup, />T<\/div>/);
  assert.doesNotMatch(app, />T<\/div>/);
});
