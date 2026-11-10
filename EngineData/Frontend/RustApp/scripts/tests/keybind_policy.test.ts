import assert from "node:assert/strict";
import test from "node:test";
import {
  DEFAULT_KEYBINDS,
  checkKeybindAssignments,
  validateKeybindChord,
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
  for (const chord of ["M", "Ctrl+Ctrl+M", "Ctrl+Shift+M+N", "Alt+F4", "Ctrl+Shift+Escape", "Meta+L"]) {
    assert.equal(validateKeybindChord(chord, "local").ok, false, chord);
  }
  assert.equal(validateKeybindChord("Ctrl+M", "global").ok, false);
  assert.equal(validateKeybindChord("Ctrl+Alt+M", "global").ok, true);
});
