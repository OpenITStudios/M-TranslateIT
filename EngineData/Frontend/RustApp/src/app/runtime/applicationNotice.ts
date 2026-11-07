import type { ProductRuntimeSnapshot } from "../bridge/runtimeProductTypes";
import { compact } from "../shared/state.ts";

// Presentation only: native Meeting and ApplicationRuntime remain authoritative.
// Never infer a severity or successful mutation from free-form message text.
export type AppNoticeKind = "info" | "progress" | "success" | "attention" | "error" | "unverified";
export type AppNotice = { message: string; kind: AppNoticeKind };

export function appNotice(message: string, kind: AppNoticeKind = "info"): AppNotice {
  return { message: compact(message, "Status unavailable.", 220), kind };
}

// Native blockers persist through unrelated status or UI events until the
// canonical snapshot confirms that the blocking lifecycle has changed.
export function activeMeetingIssue(snapshot: ProductRuntimeSnapshot | null): AppNotice | null {
  if (!snapshot) return null;
  const meeting = snapshot.meeting;
  if (meeting.lifecycle === "cleanup_incomplete") {
    return appNotice("Meeting output stopped, but cleanup is incomplete. Open Meeting and use Retry Stop.", "attention");
  }
  if (meeting.lifecycle === "unavailable" && (meeting.hasSession || snapshot.resources.owner_kind === "meeting")) {
    return appNotice("Meeting status cannot be confirmed. Check Meeting before changing audio controls.", "unverified");
  }
  if (meeting.lifecycle === "ownership_unknown") {
    return appNotice("Meeting ownership cannot be confirmed. Check Meeting before changing audio controls.", "unverified");
  }
  if (meeting.lifecycle === "runtime_conflict" && meeting.hasSession) {
    return appNotice("Meeting audio is in use by another operation. Finish that operation before starting another session.", "attention");
  }
  if (meeting.live && snapshot.meetingSession?.audio_source !== "original" && meeting.outboundStage === "attention_needed") {
    return appNotice("The latest translated phrase needs attention. Check Meeting and Diagnostics if this continues.", "attention");
  }
  return null;
}

const priority: Record<AppNoticeKind, number> = {
  info: 0,
  progress: 1,
  success: 1,
  unverified: 2,
  attention: 3,
  error: 4,
};

export function visibleAppNotice(
  current: AppNotice,
  snapshot: ProductRuntimeSnapshot | null,
  liveHealth: AppNotice | null,
  longSessionHealth: AppNotice | null,
): AppNotice {
  const nativeBlocker = activeMeetingIssue(snapshot);
  if (nativeBlocker) return nativeBlocker;
  // Monitoring warnings persist until the same read-owner clears them.
  // An unrelated general status update cannot hide a still-active warning.
  let selected = current;
  for (const active of [liveHealth, longSessionHealth]) {
    if (active && priority[active.kind] > priority[selected.kind]) selected = active;
  }
  return selected;
}
