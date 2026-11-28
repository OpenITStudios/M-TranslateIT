type TranslationIssueCategory =
  | "meaning_changed"
  | "wrong_terminology"
  | "name_number"
  | "incomplete"
  | "unnatural";

export type TranslationFeedbackEntry = {
  id: string;
  category: TranslationIssueCategory;
  source: string;
  translation: string;
  sourceLanguage: string;
  targetLanguage: string;
  createdAt: number;
};

const KEY = "translateit.translation-feedback.v1";
const MAX_ENTRIES = 40;
const MAX_TEXT_CHARS = 2000;
// Keep parsing bounded even if older or damaged storage exceeds the old writer's cap.
const MAX_STORED_CHARS = 2 * 1024 * 1024;

const VALID_CATEGORIES = new Set<TranslationIssueCategory>([
  "meaning_changed",
  "wrong_terminology",
  "name_number",
  "incomplete",
  "unnatural",
]);

function validFeedback(value: unknown): TranslationFeedbackEntry | null {
  if (!value || typeof value !== "object") return null;
  const item = value as Record<string, unknown>;
  const id = item["id"];
  const category = item["category"];
  const source = item["source"];
  const translation = item["translation"];
  const sourceLanguage = item["sourceLanguage"];
  const targetLanguage = item["targetLanguage"];
  const createdAt = item["createdAt"];
  if (
    typeof id !== "string"
    || typeof category !== "string"
    || !VALID_CATEGORIES.has(category as TranslationIssueCategory)
    || typeof source !== "string"
    || typeof translation !== "string"
    || typeof sourceLanguage !== "string"
    || typeof targetLanguage !== "string"
    || typeof createdAt !== "number"
    || !Number.isFinite(new Date(createdAt).getTime())
  ) return null;
  return {
    id: id.slice(0, 80),
    category: category as TranslationIssueCategory,
    source: source.slice(0, MAX_TEXT_CHARS),
    translation: translation.slice(0, MAX_TEXT_CHARS),
    sourceLanguage: sourceLanguage.slice(0, 16),
    targetLanguage: targetLanguage.slice(0, 16),
    createdAt,
  };
}

export type LegacyTranslationFeedback = {
  state: "empty" | "ready" | "attention" | "unavailable";
  entries: TranslationFeedbackEntry[];
};

// Compatibility reader only. Do not migrate, rewrite or silently clear
// reports; a missing key differs from damaged or inaccessible storage.
export function readTranslationFeedback(storage?: Pick<Storage, "getItem">): LegacyTranslationFeedback {
  let raw: string | null;
  try {
    raw = (storage ?? localStorage).getItem(KEY);
  } catch {
    return { state: "unavailable", entries: [] };
  }
  if (raw === null) return { state: "empty", entries: [] };
  if (raw.length > MAX_STORED_CHARS) return { state: "attention", entries: [] };
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return { state: "attention", entries: [] };
    const entries = parsed.slice(0, MAX_ENTRIES).map(validFeedback)
      .filter((value): value is TranslationFeedbackEntry => value !== null);
    const state = entries.length !== parsed.length ? "attention" : entries.length ? "ready" : "empty";
    return { state, entries };
  } catch {
    return { state: "attention", entries: [] };
  }
}

export function clearTranslationFeedback(): boolean {
  try {
    localStorage.removeItem(KEY);
    return true;
  } catch {
    return false;
  }
}
