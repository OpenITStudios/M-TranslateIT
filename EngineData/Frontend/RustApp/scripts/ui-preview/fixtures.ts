// Browser-only synthetic data for UI rendering. No Tauri reads or application writes.
import { mapProductMeetingState } from "../../src/app/bridge/productMeetingState";
import type { ProductReadiness, ProductRuntimeSnapshot } from "../../src/app/bridge/runtimeProductTypes";
import type { MeetingCommittedTurn, MeetingCommittedTurnsSnapshot, MeetingSessionStatus } from "../../src/app/bridge/runtimeApi";
import { defaultSettings } from "../../src/app/shared/state";
import { APPLICATION_MEETING_OWNER_ID } from "../../src/app/shared/types";
import type { GuidedRecordingState, GuidedLineStatus, SavedRecordingPage } from "../../src/app/bridge/myVoiceApi";
import type { MyVoiceBuildStatus } from "../../src/app/bridge/myVoiceBuildApi";

export type MeetingPreviewScenario =
  | "ready"
  | "live"
  | "muted"
  | "cleanup"
  | "translation-failed"
  | "long-transcript";

export type MyVoicePreviewScenario =
  | "my-voice-choose" | "my-voice-record" | "my-voice-take-review"
  | "my-voice-train" | "my-voice-training" | "my-voice-review"
  | "my-voice-library" | "my-voice-recovery";

export type PreviewScenario = MeetingPreviewScenario | MyVoicePreviewScenario;

export const PREVIEW_SCENARIOS: ReadonlyArray<{ id: PreviewScenario; label: string }> = [
  { id: "ready", label: "Ready (not started)" },
  { id: "live", label: "Live Meeting" },
  { id: "muted", label: "Live · muted" },
  { id: "cleanup", label: "Stop needed" },
  { id: "translation-failed", label: "Translation failed" },
  { id: "long-transcript", label: "Long transcript" },
  { id: "my-voice-choose", label: "My Voice · Choose" },
  { id: "my-voice-record", label: "My Voice · Record" },
  { id: "my-voice-take-review", label: "My Voice · Review take" },
  { id: "my-voice-train", label: "My Voice · Train ready" },
  { id: "my-voice-training", label: "My Voice · Training active" },
  { id: "my-voice-review", label: "My Voice · Review voice" },
  { id: "my-voice-library", label: "My Voice · Library" },
  { id: "my-voice-recovery", label: "My Voice · Draft recovery" },
];

export type PreviewViewport = "compact" | "desktop" | "large";
export const PREVIEW_VIEWPORTS: Readonly<Record<PreviewViewport, { width: number; height: number }>> = {
  compact: { width: 1280, height: 760 },
  desktop: { width: 1600, height: 940 },
  large: { width: 1920, height: 1080 },
};

const FIXED_MS = Date.UTC(2026, 10, 8, 5, 32, 0);
const SESSION_ID = "preview_1794115920000";
const CLAIM = "synthetic_preview_fixture_not_native_proof";

function sampleTurn(
  sequence: number,
  lane: "you" | "incoming",
  source: string,
  translation: string,
  failed = false,
): MeetingCommittedTurn {
  const own = lane === "you";
  const stamp = FIXED_MS + sequence * 8_000;
  return {
    session_id: SESSION_ID,
    sequence,
    generation: 42,
    utterance_id: sequence,
    lane,
    source_language: own ? "id" : "en",
    target_language: own ? "en" : "id",
    source_text: source,
    translated_text: failed ? "" : translation,
    translation_state: failed ? "failed" : "translated",
    delivery_state: own ? failed ? "output_failed" : "output_complete" : null,
    outbound_timing: null,
    created_unix_ms: stamp,
    updated_unix_ms: stamp + 500,
  };
}

function conversation(scenario: MeetingPreviewScenario): MeetingCommittedTurn[] {
  if (scenario === "long-transcript") {
    return Array.from({ length: 36 }, (_, i) => {
      const seq = i + 1;
      return i % 2 === 0
        ? sampleTurn(seq, "incoming",
          i === 4 ? "Could we review the specification, including all the long technical details about timing, placement, boundaries and final approval, before publishing this map? ".repeat(3)
            : `Please review the design requirement for section ${seq} before the meeting ends.`,
          `Mohon tinjau kebutuhan desain untuk bagian ${seq} sebelum rapat berakhir.`)
        : sampleTurn(seq, "you",
          `Saya mencatat permintaan revisi untuk bagian ${seq} dan akan menindaklanjutinya.`,
          `I have noted the revision request for section ${seq} and will follow it up.`);
    });
  }
  return [
    sampleTurn(1, "incoming", "Can we confirm the updated map layout during this meeting?",
      "Bisakah kita mengonfirmasi tata letak map yang diperbarui dalam rapat ini?"),
    sampleTurn(2, "you", "Ya, saya sudah menyiapkan revisi dan ukuran area utamanya.",
      "Yes, I have prepared the revision and the main area dimensions."),
    sampleTurn(3, "incoming", "Please make sure the center control point is accessible from both sides.",
      "Pastikan titik kontrol tengah dapat diakses dari kedua sisi."),
    sampleTurn(4, "you", "Baik, saya akan memeriksa jalur masuk dan akses ke area tengah.",
      "Understood. I will check the entry paths and access to the center area.",
      scenario === "translation-failed"),
  ];
}

export type PreviewFixture = {
  snapshot: ProductRuntimeSnapshot;
  meetingStatus: MeetingSessionStatus;
  turns: MeetingCommittedTurnsSnapshot | null;
};

export function makePreviewFixture(scenario: MeetingPreviewScenario): PreviewFixture {
  const isLive = scenario !== "ready" && scenario !== "cleanup";
  const cleanup = scenario === "cleanup";
  const muted = scenario === "muted";
  const settings = { ...defaultSettings(), meeting_setup_state: "completed" };
  const status: MeetingSessionStatus = {
    lifecycle: cleanup ? "cleanup_incomplete" : isLive ? "live" : "idle",
    has_session: isLive,
    authority_active: isLive,
    session_id: isLive ? SESSION_ID : null,
    generation: isLive ? 42 : null,
    started_unix_ms: isLive ? FIXED_MS - 120_000 : null,
    active_age_ms: isLive ? 120_000 : null,
    capture_active: isLive,
    audio_source: "translateit",
    muted,
    owner_id: isLive ? APPLICATION_MEETING_OWNER_ID : null,
    blocker: cleanup ? "meeting_session:cleanup_incomplete" : "",
    note: "Synthetic preview; not a local audio or Windows result.",
    preflight: {
      ready_for_start: !isLive && !cleanup,
      start_eligible: !isLive && !cleanup,
      functional_outbound_ready: !cleanup,
      functional_outbound_verified_unix_ms: cleanup ? null : FIXED_MS,
      microphone_ready: !cleanup,
      models_ready: !cleanup,
      helper_ready: !cleanup,
      provider_ready: !cleanup,
      meeting_route_ready: !cleanup,
      blockers: cleanup ? ["meeting_session:cleanup_incomplete"] : [],
      summary: cleanup ? "Cleanup needs attention." : "Synthetic components ready.",
      runtime_claim: CLAIM,
    },
    outbound: {
      generation: isLive ? 42 : null,
      session_id: isLive ? SESSION_ID : null,
      stage: scenario === "translation-failed" ? "attention_needed" : isLive ? "listening" : "idle",
      utterance_sequence: isLive ? 4 : 0,
      playback_sequence: null,
      output_active: false,
      last_stage_ok: scenario !== "translation-failed",
      timing: null,
      overflow_dropped_utterance_count: 0,
      evicted_pending_utterance_count: 0,
      blocker: scenario === "translation-failed" ? "preview:translation_failed" : "",
      note: "No audio is recorded or played in this fixture.",
      updated_unix_ms: FIXED_MS,
      runtime_claim: CLAIM,
    },
    incoming: {
      session_id: isLive ? SESSION_ID : null,
      stage: isLive ? "listening" : "idle",
      capture_active: isLive,
      suppressed: false,
      degraded: false,
      blocker: "",
      note: "Incoming lane is synthetic and mixed; no individual speakers.",
      updated_unix_ms: FIXED_MS,
      runtime_claim: CLAIM,
    },
    runtime_claim: CLAIM,
  };
  const meeting = mapProductMeetingState(status);
  const readiness: ProductReadiness = {
    level: cleanup ? "blocked" : "ready",
    helperReady: !cleanup,
    microphoneReady: !cleanup,
    asrReady: !cleanup,
    translationIdEnReady: !cleanup,
    translationEnIdReady: !cleanup,
    ttsReady: !cleanup,
    meetingRouteReady: !cleanup,
    meetingReady: !cleanup,
    approvedVoiceReady: true,
    recording: false,
    nextAction: cleanup ? "Retry Stop" : "",
    blockers: cleanup ? ["meeting_session:cleanup_incomplete"] : [],
    summary: cleanup ? "Cleanup needs attention." : "Synthetic readiness for layout only.",
    helperStatus: "Synthetic",
    modelStatus: "Synthetic",
    microphoneStatus: "Synthetic",
    voiceStatus: "Built-in",
    meetingStatus: meeting.label,
    runtimeStatus: "Synthetic preview",
  };
  const snapshot: ProductRuntimeSnapshot = {
    applicationRevision: 1,
    settings,
    readiness,
    meeting,
    meetingSession: status,
    helper: null,
    workerStatus: null,
    inputStatus: null,
    resources: {
      audio_locked: isLive || cleanup,
      microphone_available: !isLive && !cleanup,
      active_owner: isLive ? APPLICATION_MEETING_OWNER_ID : null,
      owner_kind: isLive || cleanup ? "meeting" : "none",
      blocker: cleanup ? "meeting_session:cleanup_incomplete" : "",
    },
    voiceBuildActive: false,
  };
  const turns: MeetingCommittedTurnsSnapshot | null = isLive ? {
    ok: true,
    has_session: true,
    session_id: SESSION_ID,
    turns: conversation(scenario),
    dropped_turn_count: 0,
    archive_write_errors: 0,
    truncated: false,
    blocker: "",
    note: "Synthetic bilingual transcript; saved archives are not loaded.",
    runtime_claim: CLAIM,
  } : null;
  return { snapshot, meetingStatus: status, turns };
}

export function isMyVoicePreviewScenario(scenario: PreviewScenario): scenario is MyVoicePreviewScenario {
  return scenario.startsWith("my-voice-");
}

type MyVoiceFixture = {
  workspace: "choose" | "create" | "library";
  recording: GuidedRecordingState;
  build: MyVoiceBuildStatus;
  savedRecordings: SavedRecordingPage;
};

const VOICE_LINES: GuidedLineStatus[] = [
  { line_id: 1, text: "Please share an update on the project when you're ready.", accepted: false,
    level: "short", focus: "updates", coverage: "conversation" },
  { line_id: 2, text: "I'd like to confirm the next meeting time with everyone.", accepted: false,
    level: "short", focus: "scheduling", coverage: "intonation" },
  { line_id: 3, text: "Our team is working on the final design proposal this week.", accepted: false,
    level: "natural", focus: "work", coverage: "natural" },
  { line_id: 4, text: "The latest version includes a more reliable recording process.", accepted: false,
    level: "natural", focus: "technology", coverage: "details" },
  { line_id: 5, text: "After considering the requirements, we decided to revise the implementation.", accepted: false,
    level: "detailed", focus: "planning", coverage: "explanations" },
  { line_id: 6, text: "Could you explain how we should handle the next important milestone?", accepted: false,
    level: "detailed", focus: "planning", coverage: "conversation" },
  { line_id: 7, text: "I appreciate your feedback on the updated plan.", accepted: false,
    level: "natural", focus: "feedback", coverage: "intonation" },
  { line_id: 8, text: "Thank you for taking the time to discuss these changes.", accepted: false,
    level: "short", focus: "feedback", coverage: "natural" },
];

const HELD_OUT_LINES = [
  "Thank you for joining the meeting today.",
  "I will send the updated plan this afternoon.",
  "Could you clarify the next steps, please?",
  "We have reviewed all the remaining details.",
  "Let me summarize the important changes.",
  "Our team is preparing the next release.",
  "Please let me know if you have questions.",
  "Everything is ready for our next discussion.",
];

// Every element is synthetic; none describes measured audio/model quality.
export function makeMyVoicePreviewFixture(scenario: MyVoicePreviewScenario): MyVoiceFixture {
  const isRecordingStage = scenario === "my-voice-record" || scenario === "my-voice-take-review" ||
    scenario === "my-voice-recovery";
  const hasAccepted = !isRecordingStage && scenario !== "my-voice-choose";
  const recording: GuidedRecordingState = {
    recording_line_id: null,
    pending_review: scenario === "my-voice-take-review"
      ? { line_id: 1, duration_ms: 4500, quality_blocker: "", review_advice: [],
          warning_intervals: [], warning_intervals_truncated: false }
      : null,
    pending_review_id: scenario === "my-voice-take-review" ? "synthetic-review-owner" : null,
    orphaned_drafts: scenario === "my-voice-recovery"
      ? [{ line_id: 1, pair_present: true, note: "Synthetic unfinished recording. No files were opened." }]
      : [],
    draft_recovery_error: null,
    lines: VOICE_LINES.map((line) => ({ ...line, accepted: hasAccepted })),
  };
  const inTraining = scenario === "my-voice-training";
  const inReview = scenario === "my-voice-review";
  const accepted = hasAccepted ? VOICE_LINES.length : 0;
  const build: MyVoiceBuildStatus = {
    active: inTraining,
    preview_active: false,
    generation: inTraining ? 7 : null,
    phase: inTraining ? "training" : inReview ? "evaluation_ready" : "idle",
    message: inTraining
      ? "Synthetic training stage. No GPU, model or audio task is running."
      : "Synthetic My Voice layout data; no native status was queried.",
    accepted_take_count: accepted,
    accepted_duration_ms: hasAccepted ? 65_000 : 0,
    minimum_duration_ms: 60_000,
    missing_coverage: null,
    recording_suggestions: [],
    can_build: hasAccepted,
    evaluation_ready: inReview,
    evaluation_review_id: inReview ? "synthetic-held-out-review" : null,
    evaluation_samples: inReview ? HELD_OUT_LINES.map((exact_text, index) => ({
      line_id: 1001 + index,
      exact_text,
      wav_file: "synthetic-not-a-real-file",
      sha256: "0".repeat(64),
      speaker_similarity: 0.5,
      intelligibility_text: exact_text,
      intelligibility_wer: 0.15,
      artifact_flags: [],
    })) : [],
    evaluation_comparison: [],
    reference_selection: null,
    approved_voice_ready: false,
    selected_voice_kind: "built_in",
    selected_builtin_voice_id: "MaleVoice",
    saved_personal_voice_available: true,
  };
  const savedRecordings: SavedRecordingPage = {
    items: VOICE_LINES.slice(0, 4).map((line) => ({
      archive_id: "synthetic-archive-" + line.line_id,
      line_id: line.line_id,
      text: line.text,
      duration_ms: 5700 + line.line_id * 300,
      native_rate_available: true,
    })),
    next_cursor: null,
    attention_entries: 0,
  };
  return {
    workspace: scenario === "my-voice-choose" ? "choose"
      : scenario === "my-voice-library" ? "library" : "create",
    recording, build, savedRecordings,
  };
}
