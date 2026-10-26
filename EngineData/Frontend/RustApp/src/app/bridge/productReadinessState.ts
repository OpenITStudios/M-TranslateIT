import type { ApplicationInputStatus } from "./applicationRuntimeApi";
import type { MeetingSessionStatus } from "./runtimeApi";
import { resolveMeetingVoiceGate } from "../runtime/meetingVoiceGate";
import { compact } from "../shared/state";
import type {
  HelperBridgeStatus,
  HelperBridgeWorkerResponse,
} from "../shared/types";
import { parseWorkerCapabilities } from "./workerCapabilities";
import {
  mapProductMeetingState,
  meetingBridgeUnavailable,
  meetingPreflight,
  type MeetingPreflightSnapshot,
} from "./productMeetingState";
import type {
  ProductReadiness,
  ProductReadinessLevel,
  WorkerCapabilitySnapshot,
} from "./runtimeProductTypes";

const FRONTEND_BRIDGE_UNAVAILABLE = "frontend_bridge_unavailable";

function unique(values: Array<string | null | undefined>): string[] {
  return Array.from(new Set(values.map((value) => compact(value, "")).filter(Boolean)));
}

export function helperBridgeUnavailable(helper: HelperBridgeStatus | null): boolean {
  return helper?.runtime_claim === FRONTEND_BRIDGE_UNAVAILABLE || helper?.state === "frontend_bridge_error";
}

function inputBridgeUnavailable(status: ApplicationInputStatus | null): boolean {
  return status?.blocker === FRONTEND_BRIDGE_UNAVAILABLE;
}

function collectBlockers(input: {
  helper: HelperBridgeStatus | null;
  worker: WorkerCapabilitySnapshot;
  inputStatus: ApplicationInputStatus | null;
  meeting: MeetingPreflightSnapshot;
  meetingReady: boolean;
  voiceBlocker: string | null;
}): string[] {
  const { helper, worker, inputStatus, meeting, meetingReady, voiceBlocker } = input;
  return unique([
    ...(!meetingReady ? meeting.blockers : []),
    ...(!meetingReady && worker.blocker ? [worker.blocker] : []),
    ...(!meetingReady && voiceBlocker ? [voiceBlocker] : []),
    helper?.state !== "ready" ? helper?.last_error ?? null : null,
    inputStatus?.blocker ?? null,
  ]).slice(0, 8);
}

export function mapProductReadiness(input: {
  helper: HelperBridgeStatus | null;
  workerStatus?: HelperBridgeWorkerResponse | null;
  inputStatus: ApplicationInputStatus | null;
  meetingSession?: MeetingSessionStatus | null;
  approvedVoiceReady?: boolean | null;
}): ProductReadiness {
  const helper = input.helper;
  const inputStatus = input.inputStatus;
  const worker = parseWorkerCapabilities(input.workerStatus ?? null);
  const meeting = meetingPreflight(input.meetingSession ?? null);
  const productMeeting = mapProductMeetingState(input.meetingSession ?? null);
  // An explicitly selected raw microphone is not a translation session.
  const originalLive = productMeeting.live && input.meetingSession?.audio_source === "original";

  const helperUnavailable = helperBridgeUnavailable(helper);
  const meetingUnavailable = meetingBridgeUnavailable(input.meetingSession ?? null);
  const inputUnavailable = inputBridgeUnavailable(inputStatus);
  const runtimeUnavailable = helperUnavailable && meetingUnavailable && inputUnavailable;

  const workerHelperReady = helper?.state === "ready";
  const helperReady = meeting.helperReady;
  const microphoneReady = meeting.microphoneReady;
  const asrReady = workerHelperReady && worker.asrReady;
  const translationIdEnReady = workerHelperReady && worker.translationIdEnReady;
  const translationEnIdReady = workerHelperReady && worker.translationEnIdReady;
  const ttsReady = workerHelperReady && worker.ttsReady;
  const providerReady = meeting.providerReady;
  const modelsReady = meeting.modelsReady;
  const functionalOutboundReady = meeting.functionalOutboundReady;

  const meetingRouteReady = meeting.meetingRouteReady;
  const approvedVoiceReady = typeof input.approvedVoiceReady === "boolean" ? input.approvedVoiceReady : null;
  const voiceGate = resolveMeetingVoiceGate({
    live: productMeeting.live,
    preflightReady: meeting.readyForStart,
    selectedVoiceReady: approvedVoiceReady,
  });
  const meetingReady = voiceGate.meetingReady
    && (productMeeting.live || (!productMeeting.hasSession && productMeeting.lifecycle === "idle"));
  const recording = productMeeting.captureActive;
  const blockers = collectBlockers({
    helper,
    worker,
    inputStatus,
    meeting,
    meetingReady,
    voiceBlocker: voiceGate.blocker,
  });

  const hasRuntimeEvidence = Boolean(helper || worker.responseAvailable || inputStatus || input.meetingSession);
  const level: ProductReadinessLevel = runtimeUnavailable
    ? "unavailable"
    : meetingReady
      ? "ready"
      : hasRuntimeEvidence
        ? "blocked"
        : "checking";
  const nextAction = runtimeUnavailable
    ? "TranslateIT is unavailable right now. Try the status check again before using translation."
    : productMeeting.live
      ? originalLive
        ? "Original Microphone is live. Your own voice is sent directly without translation."
        : "Translation is live. Stop the Meeting session when you are finished."
      : productMeeting.hasSession || productMeeting.lifecycle === "cleanup_incomplete"
        ? productMeeting.message
      : voiceGate.nextAction
        ? voiceGate.nextAction
        : meeting.readyForStart
          ? "Meeting Translation is ready to start."
          : productMeeting.canStart
            ? "Start Translation will run a quick final translation check before going live."
            : "Meeting setup needs attention. Open Settings to continue.";
  const summary = runtimeUnavailable
    ? "TranslateIT is unavailable right now. Try the status check again."
    : productMeeting.live
      ? originalLive ? "Original Microphone is live." : "Meeting Translation is live."
      : productMeeting.hasSession || productMeeting.lifecycle === "cleanup_incomplete"
        ? productMeeting.message
      : voiceGate.summary
        ? voiceGate.summary
        : meeting.readyForStart
          ? "Meeting Translation is ready."
          : productMeeting.canStart
            ? "Meeting setup is available; the final local translation check has not passed for this helper session yet."
            : hasRuntimeEvidence
              ? "Meeting Translation is not ready yet."
              : "Product readiness is still checking.";

  return {
    level,
    helperReady,
    microphoneReady,
    asrReady,
    translationIdEnReady,
    translationEnIdReady,
    ttsReady,
    meetingRouteReady,
    meetingReady,
    approvedVoiceReady,
    recording,
    nextAction,
    blockers,
    summary,
    helperStatus: helperUnavailable
      ? "Unavailable"
      : helperReady
        ? worker.responseAvailable
          ? "Local worker running"
          : "Worker running; capability check unavailable"
        : compact(helper?.state ?? helper?.message, "Local worker not running"),
    modelStatus: helperUnavailable
      ? "Unavailable"
      : modelsReady && functionalOutboundReady
        ? "Required outbound translation check passed"
        : modelsReady
          ? "Final local translation check pending"
          : worker.responseAvailable
            ? "Required outbound model runtime needs setup"
            : "Worker capability not checked",
    microphoneStatus: inputUnavailable && meetingUnavailable
      ? "Unavailable"
      : microphoneReady
        ? compact(inputStatus?.selected_device_name, "Microphone ready")
        : "Setup Needed",
    voiceStatus: runtimeUnavailable
      ? "Unavailable"
      : microphoneReady && providerReady
        ? "Required local outbound AI capabilities available"
        : "Local voice runtime needs setup",
    meetingStatus: productMeeting.lifecycle !== "idle" || productMeeting.hasSession
      ? productMeeting.label
      : meeting.readyForStart
            ? voiceGate.status
            : level === "checking"
              ? "Checking"
              : "Setup Needed",
    runtimeStatus: runtimeUnavailable
      ? "Unavailable"
      : productMeeting.lifecycle !== "idle"
        ? productMeeting.lifecycle
        : compact(helper?.state, "Checking"),
  };
}
