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

/**
 * Versioned configuration boundary. This only validates and serializes data;
 * it neither persists nor registers shortcuts.
 */
export const KEYBIND_CONFIG_VERSION = 1;
export type KeybindConfiguration = {
  version: typeof KEYBIND_CONFIG_VERSION;
  assignments: KeybindAssignment[];
};
export type KeybindConfigurationResult =
  | { ok: true; configuration: KeybindConfiguration }
  | { ok: false; reason: string };

export function parseKeybindConfiguration(raw: unknown): KeybindConfigurationResult {
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) {
    return { ok: false, reason: "Configuration must be an object." };
  }
  const record = raw as Record<string, unknown>;
  if (record.version !== KEYBIND_CONFIG_VERSION ||
      Object.keys(record).sort().join(",") !== "assignments,version") {
    return { ok: false, reason: "Unsupported configuration version or fields." };
  }
  if (!Array.isArray(record.assignments) || record.assignments.length !== KEYBIND_ACTIONS.length) {
    return { ok: false, reason: "Expected exactly one assignment per supported action." };
  }
  const knownActions = new Set<string>(KEYBIND_ACTIONS);
  const normalized: KeybindAssignment[] = [];
  for (const value of record.assignments) {
    if (value === null || typeof value !== "object" || Array.isArray(value)) {
      return { ok: false, reason: "Invalid assignment." };
    }
    const item = value as Record<string, unknown>;
    if (Object.keys(item).sort().join(",") !== "action,chord,scope" ||
        typeof item.action !== "string" || !knownActions.has(item.action) ||
        (item.scope !== "local" && item.scope !== "global") ||
        (item.chord !== null && typeof item.chord !== "string")) {
      return { ok: false, reason: "Unsupported action, scope, chord, or assignment fields." };
    }
    const action = item.action as KeybindAction;
    const scope = item.scope as KeybindScope;
    const chord = item.chord as string | null;
    if (chord === null) {
      normalized.push({ action, scope, chord: null });
      continue;
    }
    const checked = validateKeybindChord(chord, scope);
    if (!checked.ok) return { ok: false, reason: `${action}: ${checked.reason}` };
    normalized.push({ action, scope, chord: checked.normalized });
  }
  const issues = checkKeybindAssignments(normalized);
  if (issues.length) return { ok: false, reason: issues[0] };
  const actions = new Set(normalized.map((item) => item.action));
  if (actions.size !== KEYBIND_ACTIONS.length) {
    return { ok: false, reason: "Duplicate or missing action." };
  }
  // Stable canonical ordering ensures deterministic persistence payloads.
  return {
    ok: true,
    configuration: {
      version: KEYBIND_CONFIG_VERSION,
      assignments: KEYBIND_ACTIONS.map((action) => normalized.find((item) => item.action === action)!),
    },
  };
}

export function serializeKeybindConfiguration(raw: unknown): KeybindConfigurationResult & { json?: string } {
  const checked = parseKeybindConfiguration(raw);
  if (!checked.ok) return checked;
  return { ...checked, json: JSON.stringify(checked.configuration) };
}
