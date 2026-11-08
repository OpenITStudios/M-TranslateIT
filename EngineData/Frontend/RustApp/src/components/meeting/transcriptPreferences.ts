/**
 * UI-only transcript presentation preferences.
 * Stores no meeting content, speaker identity, audio or runtime authorization.
 * Mirrors the existing floating-caption preference persistence boundary.
 */
export type TranscriptDensity = "cozy" | "compact";
export type TranscriptTextMode = "both" | "translation-only";
export type TranscriptTextOrder = "original-first" | "translation-first";
export type TranscriptTimestamp = "always" | "hover";

export type TranscriptPreferences = {
  density: TranscriptDensity;
  textMode: TranscriptTextMode;
  textOrder: TranscriptTextOrder;
  timestamps: TranscriptTimestamp;
  showSpeakerIcons: boolean;
  followLatest: boolean;
  showPlaybackStatus: boolean;
};

export const DEFAULT_TRANSCRIPT_PREFERENCES: Readonly<TranscriptPreferences> = Object.freeze({
  density: "cozy",
  textMode: "both",
  textOrder: "original-first",
  timestamps: "always",
  showSpeakerIcons: true,
  followLatest: true,
  showPlaybackStatus: true,
});

export const TRANSCRIPT_PREFERENCES_KEY = "translateit.transcript.appearance.v1";
const CHANGED = "translateit:transcript-appearance-changed";

function safeStorage(): Storage | null {
  try { return typeof localStorage === "undefined" ? null : localStorage; }
  catch { return null; }
}

/** Validate each field independently; never deserialize arbitrary saved objects into UI state. */
export function normalizeTranscriptPreferences(value: unknown): TranscriptPreferences {
  const raw = value && typeof value === "object" && !Array.isArray(value)
    ? value as Partial<TranscriptPreferences>
    : {};
  return {
    density: raw.density === "compact" ? "compact" : "cozy",
    textMode: raw.textMode === "translation-only" ? "translation-only" : "both",
    textOrder: raw.textOrder === "translation-first" ? "translation-first" : "original-first",
    timestamps: raw.timestamps === "hover" ? "hover" : "always",
    showSpeakerIcons: raw.showSpeakerIcons !== false,
    followLatest: raw.followLatest !== false,
    showPlaybackStatus: raw.showPlaybackStatus !== false,
  };
}

export function readTranscriptPreferences(): TranscriptPreferences {
  try {
    const raw = safeStorage()?.getItem(TRANSCRIPT_PREFERENCES_KEY);
    return raw ? normalizeTranscriptPreferences(JSON.parse(raw) as unknown)
      : { ...DEFAULT_TRANSCRIPT_PREFERENCES };
  } catch {
    return { ...DEFAULT_TRANSCRIPT_PREFERENCES };
  }
}

/** If persistence fails, do not report success or update only the visible switch. */
export function updateTranscriptPreferences(patch: Partial<TranscriptPreferences>): {
  ok: boolean;
  preferences: TranscriptPreferences;
} {
  const previous = readTranscriptPreferences();
  const next = normalizeTranscriptPreferences({ ...previous, ...patch });
  try {
    const storage = safeStorage();
    if (!storage) return { ok: false, preferences: previous };
    storage.setItem(TRANSCRIPT_PREFERENCES_KEY, JSON.stringify(next));
    if (typeof window !== "undefined") window.dispatchEvent(new Event(CHANGED));
    return { ok: true, preferences: next };
  } catch {
    return { ok: false, preferences: previous };
  }
}

/** One listener per visible transcript page, not one listener per rendered bubble. */
export function subscribeTranscriptPreferences(notify: (preferences: TranscriptPreferences) => void): () => void {
  if (typeof window === "undefined") return () => {};
  const onChange = () => notify(readTranscriptPreferences());
  const onStorage = (event: StorageEvent) => {
    if (event.key === TRANSCRIPT_PREFERENCES_KEY || event.key === null) onChange();
  };
  window.addEventListener(CHANGED, onChange);
  window.addEventListener("storage", onStorage);
  return () => {
    window.removeEventListener(CHANGED, onChange);
    window.removeEventListener("storage", onStorage);
  };
}
