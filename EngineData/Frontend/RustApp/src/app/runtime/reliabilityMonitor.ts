import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { createMeetingReconcileSchedule } from "./meetingReconcileSchedule";
import {
  getMeetingReliabilitySnapshot,
  type MeetingReliabilitySnapshot,
} from "../bridge/reliabilityApi";

const LIVE_RELIABILITY_POLL_MS = 8_000;
const MEETING_RECONCILIATION_POLL_MS = 10_000;
const MEETING_EVENT_COALESCE_MS = 80;
const MEETING_RUNTIME_EVENT = "translateit://meeting-runtime";

export type ReliabilityNotice = {
  key: string;
  message: string;
  kind: "attention" | "unverified";
};
export type ReliabilityChannel = "live" | "session";
type ReliabilityNoticeCallback = (channel: ReliabilityChannel, notice: ReliabilityNotice | null) => void;

export type MeetingRuntimeEvent = {
  revision: number;
  reason: string;
  session_id: string | null;
  sequence: number | null;
};

function liveReliabilityNotice({ watchdog, devices }: MeetingReliabilitySnapshot): ReliabilityNotice | null {

  if (devices.action_required && devices.blocker) {
    return { key: devices.blocker, message: devices.note, kind: "attention" };
  }
  if (watchdog.action_required && watchdog.blocker) {
    return { key: watchdog.blocker, message: watchdog.note, kind: "attention" };
  }
  if (watchdog.state === "unavailable" || devices.state === "unavailable") {
    return { key: "reliability_unavailable", message: "Meeting health could not be checked. Open Help if this continues.", kind: "unverified" };
  }
  if (!devices.healthy && devices.blocker) {
    return { key: devices.blocker, message: devices.note, kind: "attention" };
  }
  return null;
}

function sessionPressureNotice(pressure: MeetingReliabilitySnapshot["pressure"] | undefined): ReliabilityNotice | null {
  if (!pressure || pressure.state === "unavailable") {
    return { key: "long_session_unavailable", message: "Session health could not be checked. Open Help if this continues.", kind: "unverified" };
  }
  if (pressure.state !== "attention" || pressure.warning_count === 0) return null;
  const key = [
    pressure.outbound_overflow_dropped,
    pressure.outbound_evicted_pending,
    pressure.deferred_incoming_dropped_overflow,
    pressure.deferred_incoming_dropped_stale,
    pressure.transcript_dropped_turns,
  ].join(":");
  return { key, message: pressure.note, kind: "attention" };
}

function startMeetingReliabilityMonitor(
  onNotice: ReliabilityNoticeCallback,
): () => void {
  let disposed = false;
  let inFlight = false;
  const lastKeys: Record<ReliabilityChannel, string> = { live: "", session: "" };

  function publish(channel: ReliabilityChannel, notice: ReliabilityNotice | null): void {
    const key = notice?.key ?? "";
    if (key === lastKeys[channel]) return;
    lastKeys[channel] = key;
    onNotice(channel, notice);
  }

  const poll = async () => {
    if (disposed || inFlight) return;
    inFlight = true;
    try {
      const observation = await getMeetingReliabilitySnapshot();
      // Stop may have disposed this monitor while the IPC read was pending.
      if (disposed) return;
      publish("live", liveReliabilityNotice(observation));
      publish("session", sessionPressureNotice(observation.pressure));
    } catch {
      if (!disposed) {
        publish("live", { key: "reliability_unavailable", message: "Meeting health could not be checked. Open Help if this continues.", kind: "unverified" });
        publish("session", sessionPressureNotice(undefined));
      }
    } finally {
      inFlight = false;
    }
  };

  void poll();
  const timer = window.setInterval(() => void poll(), LIVE_RELIABILITY_POLL_MS);
  return () => {
    disposed = true;
    window.clearInterval(timer);
  };
}

function startMeetingReconciliationMonitor(
  pollMeeting: () => void | Promise<void>,
): () => void {
  let disposed = false;
  let unlisten: UnlistenFn | null = null;
  let lastRevision = 0;
  const schedule = createMeetingReconcileSchedule(
    pollMeeting,
    { set: (callback, ms) => window.setTimeout(callback, ms),
      clear: (timer) => window.clearTimeout(timer) },
    MEETING_EVENT_COALESCE_MS,
    MEETING_RECONCILIATION_POLL_MS,
  );

  // Schedule asynchronously, including the first read, so the shell's effect
  // does not track reactive reads made by pollMeeting before its first await.
  schedule.request();
  void listen<MeetingRuntimeEvent>(MEETING_RUNTIME_EVENT, (event) => {
    if (disposed || event.payload.revision <= lastRevision) return;
    lastRevision = event.payload.revision;
    schedule.request();
  }).then((stop) => {
    if (disposed) {
      stop();
      return;
    }
    unlisten = stop;
    // Close the initial-read/listener-registration gap through the same
    // coalescing window; if IPC is active, retain one follow-up.
    schedule.request();
  }).catch(() => { /* The same schedule retains the slow fallback. */ });

  return () => {
    disposed = true;
    schedule.dispose();
    unlisten?.();
  };
}

export function startMeetingRuntimeMonitors(
  pollMeeting: () => void | Promise<void>,
  onNotice: ReliabilityNoticeCallback,
  reliabilityEnabled: boolean,
): () => void {
  const stopMeetingReconciliation = startMeetingReconciliationMonitor(pollMeeting);
  const stopReliability = reliabilityEnabled
    ? startMeetingReliabilityMonitor(onNotice)
    : () => {};
  return () => {
    stopMeetingReconciliation();
    stopReliability();
  };
}
