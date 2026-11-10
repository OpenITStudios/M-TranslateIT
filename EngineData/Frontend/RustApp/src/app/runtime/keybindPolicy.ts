/**
 * Keybind policy, not a shortcut registrar.
 * No global hooks, persistence, or device actions are implemented here.
 */
export const KEYBIND_ACTIONS = [
  "toggle-microphone",
  "toggle-deafen",
  "push-to-talk",
  "toggle-main-window",
  "toggle-floating-captions",
  "start-stop-meeting",
] as const;

export type KeybindAction = (typeof KEYBIND_ACTIONS)[number];
export type KeybindScope = "local" | "global";
export type KeybindAssignment = {
  action: KeybindAction;
  scope: KeybindScope;
  chord: string | null;
};

const MODIFIERS = ["Ctrl", "Alt", "Shift", "Meta"] as const;
const MODIFIER_ALIASES: Record<string, string> = {
  ctrl: "Ctrl", control: "Ctrl",
  alt: "Alt", option: "Alt",
  shift: "Shift",
  meta: "Meta", win: "Meta", windows: "Meta", cmd: "Meta", command: "Meta",
};
const ALLOWED_KEYS = new Set([
  ...Array.from({ length: 26 }, (_, index) => String.fromCharCode(65 + index)),
  ...Array.from({ length: 10 }, (_, index) => String(index)),
  ...Array.from({ length: 12 }, (_, index) => `F${index + 1}`),
  "Space", "Enter", "Tab", "Escape", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
]);
const KEY_ALIASES: Record<string, string> = {
  space: "Space", spacebar: "Space", esc: "Escape", escape: "Escape",
  enter: "Enter", return: "Enter", tab: "Tab",
  up: "ArrowUp", down: "ArrowDown", left: "ArrowLeft", right: "ArrowRight",
  arrowup: "ArrowUp", arrowdown: "ArrowDown", arrowleft: "ArrowLeft", arrowright: "ArrowRight",
};
export type KeybindValidation =
  | { ok: true; normalized: string }
  | { ok: false; reason: string };

export function validateKeybindChord(chord: string, scope: KeybindScope): KeybindValidation {
  const parts = chord.split("+").map((part) => part.trim());
  if (parts.some((part) => !part)) {
    return { ok: false, reason: "Empty shortcut key or modifier." };
  }
  if (parts.length < 2 || parts.length > 5) {
    return { ok: false, reason: "Use one key with at least one modifier." };
  }
  const modifiers = new Set<string>();
  let key: string | null = null;
  for (const part of parts) {
    const lower = part.toLowerCase();
    const modifier = MODIFIER_ALIASES[lower];
    if (modifier) {
      if (modifiers.has(modifier)) return { ok: false, reason: "Duplicate modifier." };
      modifiers.add(modifier);
      continue;
    }
    const candidate = KEY_ALIASES[lower] ?? part.toUpperCase();
    if (!ALLOWED_KEYS.has(candidate) || key !== null) {
      return { ok: false, reason: "Choose exactly one supported non-modifier key." };
    }
    key = candidate;
  }
  if (!key || !modifiers.size) return { ok: false, reason: "Modifier and key required." };
  if (scope === "global" && modifiers.size < 2) {
    return { ok: false, reason: "Global shortcuts require two modifiers." };
  }
  if ((modifiers.has("Alt") && key === "F4") ||
      (modifiers.has("Ctrl") && modifiers.has("Alt") && key === "Delete") ||
      (modifiers.has("Ctrl") && modifiers.has("Shift") && key === "Escape") ||
      (modifiers.has("Meta") && ["D", "E", "L", "R"].includes(key))) {
    return { ok: false, reason: "Reserved operating-system shortcut." };
  }
  return {
    ok: true,
    normalized: [...MODIFIERS.filter((modifier) => modifiers.has(modifier)), key].join("+"),
  };
}

export function checkKeybindAssignments(assignments: readonly KeybindAssignment[]): string[] {
  const issues: string[] = [];
  const seen = new Map<string, KeybindAction>();
  const assignedActions = new Set<KeybindAction>();
  for (const assignment of assignments) {
    if (assignedActions.has(assignment.action)) {
      issues.push(`${assignment.action}: duplicate action assignment`);
      continue;
    }
    assignedActions.add(assignment.action);
    if (assignment.chord === null) continue;
    const validation = validateKeybindChord(assignment.chord, assignment.scope);
    if (!validation.ok) {
      issues.push(`${assignment.action}: ${validation.reason}`);
      continue;
    }
    const previous = seen.get(validation.normalized);
    // A global shortcut also conflicts with the same local shortcut.
    if (previous) issues.push(`${assignment.action}: conflicts with ${previous}`);
    else seen.set(validation.normalized, assignment.action);
  }
  return issues;
}

export const DEFAULT_KEYBINDS: readonly KeybindAssignment[] = KEYBIND_ACTIONS.map((action) => ({
  action, scope: "local", chord: null,
}));
