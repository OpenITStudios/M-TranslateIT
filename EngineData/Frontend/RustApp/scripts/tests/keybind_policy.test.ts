import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_KEYBINDS,
  checkKeybindAssignments,
  validateKeybindChord,
  KEYBIND_CONFIG_VERSION,
  parseKeybindConfiguration,
  serializeKeybindConfiguration,
} from "../../src/app/runtime/keybindPolicy.ts";

test("all shortcuts are unassigned by default", () => {
  assert.equal(DEFAULT_KEYBINDS.length, 6);
  assert.ok(DEFAULT_KEYBINDS.every((assignment) => assignment.chord === null));
});

test("normalizes modifiers and detects equivalent conflicting shortcuts", () => {
  assert.deepEqual(validateKeybindChord("shift + ctrl + m", "local"), {
    ok: true, normalized: "Ctrl+Shift+M",
  });
  const issues = checkKeybindAssignments([
    { action: "toggle-microphone", scope: "local", chord: "Ctrl+Shift+M" },
    { action: "toggle-deafen", scope: "global", chord: "shift+ctrl+m" },
  ]);
  assert.equal(issues.length, 1);
  assert.match(issues[0], /conflicts/);
});

test("rejects missing modifiers, unsupported keys and reserved combinations", () => {
  for (const chord of ["M", "Ctrl+Ctrl+M", "Ctrl++M", "Ctrl+Shift+M+N", "Alt+F4", "Ctrl+Shift+Escape", "Meta+L", "Ctrl+Alt+Delete"]) {
    assert.equal(validateKeybindChord(chord, "local").ok, false, chord);
  }
  assert.equal(validateKeybindChord("Ctrl+M", "global").ok, false);
  assert.equal(validateKeybindChord("Ctrl+Alt+M", "global").ok, true);
});

test("duplicate action assignments are rejected even when chords differ", () => {
  const issues = checkKeybindAssignments([
    { action: "toggle-microphone", scope: "local", chord: "Ctrl+Shift+M" },
    { action: "toggle-microphone", scope: "local", chord: "Ctrl+Alt+M" },
  ]);
  assert.equal(issues.length, 1);
  assert.match(issues[0], /duplicate action/);
});

test("invalid assignments never claim successful normalization", () => {
  for (const chord of ["+Ctrl+M", "Ctrl+M+", "Ctrl++M", "Ctrl+Ctrl+M"]) {
    assert.equal(validateKeybindChord(chord, "local").ok, false);
  }
});

test("versioned configuration roundtrips with canonical ordering", () => {
  const input = {
    version: KEYBIND_CONFIG_VERSION,
    assignments: [...DEFAULT_KEYBINDS].reverse().map((item) =>
      item.action === "toggle-microphone" ? { ...item, chord: "shift+ctrl+m" } : item),
  };
  const result = serializeKeybindConfiguration(input);
  assert.equal(result.ok, true);
  if (!result.ok) return;
  assert.equal(result.configuration.assignments[0].chord, "Ctrl+Shift+M");
  const decoded = parseKeybindConfiguration(JSON.parse(result.json!));
  assert.deepEqual(decoded, { ok: true, configuration: result.configuration });
});

test("configuration rejects unknown fields, duplicate actions and invalid scopes", () => {
  const base = { version: KEYBIND_CONFIG_VERSION, assignments: [...DEFAULT_KEYBINDS] };
  for (const invalid of [
    { ...base, version: 2 },
    { ...base, secret: "unexpected" },
    { ...base, assignments: base.assignments.slice(1) },
    { ...base, assignments: base.assignments.map((item, index) =>
      index === 1 ? { ...item, action: "toggle-microphone" } : item) },
    { ...base, assignments: base.assignments.map((item, index) =>
      index === 0 ? { ...item, scope: "remote" } : item) },
    { ...base, assignments: base.assignments.map((item, index) =>
      index === 0 ? { ...item, chord: "Ctrl++M" } : item) },
  ]) {
    assert.equal(parseKeybindConfiguration(invalid).ok, false);
  }
});
