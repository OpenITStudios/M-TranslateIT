import { runtimeApi, type MeetingCommittedTurnsSnapshot, type MeetingSessionStatus } from "../bridge/runtimeApi";
import { meetingBridgeUnavailable } from "../bridge/runtimeProductFacade";
import { APPLICATION_MEETING_OWNER_ID } from "../shared/types";

export type MeetingReconcileResult = {
  status: MeetingSessionStatus;
  turns: MeetingCommittedTurnsSnapshot | null;
  unavailable: boolean;
};

export async function readMeetingReconciliation(
  currentTurns: MeetingCommittedTurnsSnapshot | null,
): Promise<MeetingReconcileResult | null> {
  try {
    // The existing live read carries status and either a fresh snapshot or
    // an explicit unchanged receipt. Every event/fallback uses the same path.
    const { status, unchanged, ...nextTurns } = await runtimeApi.getMeetingCommittedTurns(
      currentTurns?.ok && currentTurns.has_session ? currentTurns.live_revision ?? null : null,
    );
    if (!status) return null;
    if (meetingBridgeUnavailable(status)) {
      return { status, turns: null, unavailable: true };
    }
    if (!status.has_session || status.owner_id !== APPLICATION_MEETING_OWNER_ID) {
      return { status, turns: null, unavailable: false };
    }
    // A changed session between observations never retains private old turns.
    if (!nextTurns.has_session || nextTurns.session_id !== status.session_id) {
      return { status, turns: null, unavailable: false };
    }
    const retainedTurns = currentTurns?.ok && currentTurns.has_session
      && currentTurns.session_id === status.session_id ? currentTurns : null;
    if (nextTurns.ok) {
      if (!unchanged) return { status, turns: nextTurns, unavailable: false };
      // An unchanged receipt is not an empty transcript. Reuse only the
      // exact verified session and token that the backend acknowledged.
      if (retainedTurns && nextTurns.live_revision
          && nextTurns.live_revision === retainedTurns.live_revision) {
        return { status, turns: retainedTurns, unavailable: false };
      }
      return { status, turns: null, unavailable: false };
    }
    // A failed observation never supplies a new token. Retaining the last
    // same-session view allows the next read to retry, never foreign captions.
    return { status, turns: retainedTurns, unavailable: false };
  } catch {
    return null;
  }
}
