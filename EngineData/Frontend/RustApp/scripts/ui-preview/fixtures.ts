// Browser-only synthetic data for UI rendering. No Tauri reads or application writes.
import { mapProductMeetingState } from "../../src/app/bridge/productMeetingState";
import type { ProductReadiness, ProductRuntimeSnapshot } from "../../src/app/bridge/runtimeProductTypes";
import type { MeetingCommittedTurn, MeetingCommittedTurnsSnapshot, MeetingSessionStatus } from "../../src/app/bridge/runtimeApi";
import { defaultSettings } from "../../src/app/shared/state";
import { APPLICATION_MEETING_OWNER_ID } from "../../src/app/shared/types";

export type PreviewScenario =
  | "ready"
  | "live"
  | "muted"
  | "cleanup"
  | "translation-failed"
  | "long-transcript";

export const PREVIEW_SCENARIOS: ReadonlyArray<{ id: PreviewScenario; label: string }> = [
  { id: "ready", label: "Ready (not started)" },
  { id: "live", label: "Live Meeting" },
  { id: "muted", label: "Live · muted" },
  { id: "cleanup", label: "Stop needed" },
  { id: "translation-failed", label: "Translation failed" },
  { id: "long-transcript", label: "Long transcript" },
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

function conversation(scenario: PreviewScenario): MeetingCommittedTurn[] {
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

export function makePreviewFixture(scenario: PreviewScenario): PreviewFixture {
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
