import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { createMeetingReconcileSchedule } from "./meetingReconcileSchedule";
import {
  getLongSessionHealthStatus,
  getMeetingReliabilitySnapshot,
} from "../bridge/reliabilityApi";

const LIVE_RELIABILITY_POLL_MS = 8_000;
const LONG_SESSION_POLL_MS = 30_000;
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

async function readLiveReliabilityNotice(): Promise<ReliabilityNotice | null> {
  const { watchdog, devices } = await getMeetingReliabilitySnapshot();

  if (devices.action_required && devices.blocker) {
    return { key: devices.blocker, message: devices.note, kind: "attention" };
  }
  if (watchdog.action_required && watchdog.blocker) {
    return { key: watchdog.blocker, message: watchdog.note, kind: "attention" };
  }
  if (!devices.healthy && devices.blocker) {
    return { key: devices.blocker, message: devices.note, kind: "attention" };
  }
  return null;
}

function startMeetingReliabilityMonitor(
  onNotice: ReliabilityNoticeCallback,
): () => void {
  let disposed = false;
  let inFlight = false;
  let lastKey = "";

  const poll = async () => {
    if (disposed || inFlight) return;
    inFlight = true;
    try {
      const notice = await readLiveReliabilityNotice();
      // Stop may have disposed this monitor while the IPC read was pending.
      if (disposed) return;
      if (!notice) {
        if (lastKey) onNotice("live", null);
        lastKey = "";
        return;
      }
      if (notice.key !== lastKey) {
        lastKey = notice.key;
        onNotice("live", notice);
      }
    } catch {
      if (!disposed && lastKey !== "reliability_unavailable") {
        lastKey = "reliability_unavailable";
        onNotice("live", {
          key: "reliability_unavailable",
          message: "Meeting health could not be checked. Open Help if this continues.",
          kind: "unverified",
        });
      }
    } finally {
      inFlight = false;
    }
  };

  void poll();
  const timer = window.setInterval(() => void poll(), LIVE_RELIABILITY_POLL_MS);

  let lastLongSessionState = "";
  let longSessionInFlight = false;
  const pollLongSession = async () => {
    if (disposed || longSessionInFlight) return;
    longSessionInFlight = true;
    try {
      const health = await getLongSessionHealthStatus();
      // Do not surface an old session's warning after monitor teardown.
      if (disposed) return;
      if (!health.healthy && health.warning_count > 0) {
        const key = [
          health.outbound_overflow_dropped,
          health.outbound_evicted_pending,
          health.deferred_incoming_dropped_overflow,
          health.deferred_incoming_dropped_stale,
          health.transcript_dropped_turns,
          health.meeting_temp_file_count,
        ].join(":");
        if (key !== lastLongSessionState) {
          lastLongSessionState = key;
          onNotice("session", { key, message: health.note, kind: "attention" });
        }
      } else {
        if (lastLongSessionState) onNotice("session", null);
        lastLongSessionState = "";
      }
    } catch {
      if (!disposed && lastLongSessionState !== "long_session_unavailable") {
        lastLongSessionState = "long_session_unavailable";
        onNotice("session", {
          key: "long_session_unavailable",
          message: "Session health could not be checked. Open Help if this continues.",
          kind: "unverified",
        });
      }
    } finally {
      longSessionInFlight = false;
    }
  };
  const longSessionTimer = window.setInterval(
    () => void pollLongSession(),
    LONG_SESSION_POLL_MS,
  );

  return () => {
    disposed = true;
    window.clearInterval(timer);
    window.clearInterval(longSessionTimer);
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
