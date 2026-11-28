import {
  applicationRuntimeApi,
  type ApplicationIntentResult,
  type ApplicationSnapshot,
} from "./applicationRuntimeApi";
import { runtimeApi, type MeetingSessionActionResult } from "./runtimeApi";
import { compact } from "../shared/state";
import type { RuntimeSettings } from "../shared/types";
import {
  mapProductMeetingState,
  mapProductReadiness,
} from "./runtimeProductState";
import type {
  ProductMeetingState,
  ProductRuntimeSnapshot,
} from "./runtimeProductTypes";
import {
  loadProductAudioDevices,
  probeProductAudioDevice,
  selectProductAudioDevice,
} from "./productAudioFacade";
import {
  runProductRecoveryAction,
  runProductSetupAction,
} from "./productSetupFacade";

export {
  mapProductMeetingState,
  mapProductReadiness,
  meetingBridgeUnavailable,
  parseWorkerCapabilities,
} from "./runtimeProductState";
export {
  loadProductAudioDevices,
  probeProductAudioDevice,
  selectProductAudioDevice,
} from "./productAudioFacade";
export {
  runProductRecoveryAction,
  runProductSetupAction,
} from "./productSetupFacade";

export type {
  ProductAudioDeviceKind,
  ProductAudioDeviceProbe,
  ProductAudioDeviceSelectionResult,
} from "./productAudioFacade";
export type {
  ProductRecoveryAction,
  ProductSetupAction,
  ProductSetupResult,
} from "./productSetupFacade";
export type {
  ProductMeetingState,
  ProductReadiness,
  ProductReadinessLevel,
  ProductRuntimeSnapshot,
  WorkerCapabilitySnapshot,
} from "./runtimeProductTypes";

export type ProductMeetingAction = "start" | "stop";

export type ProductMeetingActionResult = {
  ok: boolean;
  action: ProductMeetingAction;
  state: string;
  message: string;
  meeting: ProductMeetingState;
  status: MeetingSessionActionResult["status"];
  snapshot: ApplicationSnapshot;
  reason: string;
};

const WORKER_REUSE_SAFE_REASONS = new Set([
  "stop_meeting",
  "start_mic_test",
  "stop_mic_test",
  "select_audio_device",
  "save_settings",
  "start_voice_recording",
  "stop_voice_recording",
  "start_voice_build",
  "cancel_voice_build",
  "voice_build_completed",
  "voice_build_cancelled",
  "voice_build_failed",
]);

export function requireApplicationSnapshot(application: ApplicationSnapshot): void {
  if (application.revision <= 0 || application.lifecycle === "unavailable") {
    throw new Error("TranslateIT application runtime is unavailable.");
  }
  if (!application.settings) throw new Error("TranslateIT settings are unavailable.");
}

export function shouldBootstrapLocalRuntime(snapshot: {
  settings: RuntimeSettings;
  helper: { state: string } | null;
} | null): boolean {
  return snapshot?.settings.translation_engine === "local"
    && snapshot.settings.meeting_setup_state !== "new"
    && snapshot.helper?.state === "not_started";
}

// Startup and explicit refresh consume the same verified preparation receipt.
export async function prepareLocalRuntime(initialRevision: number): Promise<ApplicationIntentResult> {
  const outcome = await applicationRuntimeApi.dispatchIntent("ensure_runtime_ready");
  requireApplicationSnapshot(outcome.snapshot);
  if (outcome.snapshot.revision <= initialRevision) {
    throw new Error("TranslateIT runtime preparation returned a stale snapshot.");
  }
  return outcome;
}

async function mapApplicationSnapshotToProduct(
  application: ApplicationSnapshot,
  previous?: ProductRuntimeSnapshot | null,
  reason?: string,
): Promise<ProductRuntimeSnapshot> {
  requireApplicationSnapshot(application);
  const settings = application.settings;

  const helper = application.helper;
  const helperIdentityChanged = Boolean(
    previous
      && (
        previous.helper?.generation_token !== helper.generation_token
        || previous.helper?.state !== helper.state
      ),
  );
  const refreshWorker = !previous
    || helperIdentityChanged
    || !WORKER_REUSE_SAFE_REASONS.has(reason ?? "");
  // Voice eligibility is observed by the native snapshot owner. Missing
  // or unknown fields never inherit a previous approved voice.
  const observedVoiceReady = application.summaries.voice.approved_voice_ready;
  const approvedVoiceReady = typeof observedVoiceReady === "boolean" ? observedVoiceReady : null;
  const observedWorkerStatus = refreshWorker
    ? (helper.state === "ready" ? await runtimeApi.helperBridgeWorkerStatus() : null)
    : previous.workerStatus;
  // The native application snapshot and the worker response are separate reads.
  // A helper restart between them must not combine an old/new worker capability
  // or GPU memory observation with the other generation's application state.
  // Retained snapshots are allowed only while the owning generation still matches.
  const workerStatus = observedWorkerStatus?.task === "status"
    && helper.state === "ready"
    && observedWorkerStatus.generation_token === helper.generation_token
    ? observedWorkerStatus
    : null;
  const meetingSession = application.meeting;
  const inputStatus = application.input;
  const meeting = mapProductMeetingState(meetingSession);
  const readiness = mapProductReadiness({
    helper,
    workerStatus,
    inputStatus,
    meetingSession,
    approvedVoiceReady,
  });

  return {
    applicationRevision: application.revision,
    settings,
    readiness,
    meeting,
    meetingSession,
    helper,
    workerStatus,
    inputStatus,
    resources: application.resources,
    voiceBuildActive: application.summaries.voice.build_active,
  };
}

export async function loadProductRuntimeSnapshot(
  initialApplication?: ApplicationSnapshot,
): Promise<ProductRuntimeSnapshot> {
  let application = initialApplication ?? await applicationRuntimeApi.getSnapshot();
  requireApplicationSnapshot(application);
  if (shouldBootstrapLocalRuntime(application)) {
    application = (await prepareLocalRuntime(application.revision)).snapshot;
  }
  return mapApplicationSnapshotToProduct(application);
}

export async function loadProductRuntimeSnapshotFromApplication(
  application: ApplicationSnapshot,
  previous: ProductRuntimeSnapshot | null,
  reason: string,
): Promise<ProductRuntimeSnapshot> {
  return mapApplicationSnapshotToProduct(application, previous, reason);
}

export async function runProductMeetingAction(
  action: ProductMeetingAction,
  source: "original" | "translateit" = "translateit",
): Promise<ProductMeetingActionResult> {
  const result = await applicationRuntimeApi.dispatchIntent(
    action === "stop" ? "stop_meeting"
      : source === "original" ? "start_original_meeting" : "start_meeting",
  );
  const status = result.snapshot.meeting;
  const fallbackMessage = action === "start"
    ? "Start Translation finished."
    : "Stop Translation finished.";

  return {
    ok: Boolean(result.ok),
    action,
    state: compact(result.state, result.ok ? "completed" : "blocked"),
    message: compact(result.message, fallbackMessage),
    meeting: mapProductMeetingState(status),
    status,
    snapshot: result.snapshot,
    reason: result.intent,
  };
}

export const runtimeProductFacade = {
  loadProductRuntimeSnapshot,
  loadProductRuntimeSnapshotFromApplication,
  loadProductAudioDevices,
  probeProductAudioDevice,
  selectProductAudioDevice,
  mapProductMeetingState,
  mapProductReadiness,
  runProductMeetingAction,
  runProductSetupAction,
  runProductRecoveryAction,
};
