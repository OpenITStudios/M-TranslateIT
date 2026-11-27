import type {
  MeetingLiveTurnsSnapshot,
  MeetingSessionStatus,
} from "../bridge/runtimeApi";
import { readMeetingReconciliation } from "./meetingReconcileReader";
import { clearLatestMeetingOverlay, publishLatestMeetingOverlay } from "./translationOverlayRuntime";

export type MeetingLiveViewState = {
  turns: MeetingLiveTurnsSnapshot | null;
  overlayRevision: string;
  inFlight: boolean;
  revision: number;
};

export type MeetingReconcileContext = {
  ownerKind: string | null;
  booting: boolean;
  setupRequired: boolean;
  closeAfterExistingStop: boolean;
};

export type MeetingReconcileCallbacks = {
  onStatus: (status: MeetingSessionStatus) => void;
  onNotice: (message: string) => void;
  onStoppedForClose: () => void | Promise<void>;
};

export function createMeetingLiveController(): {
  read: () => MeetingLiveViewState;
  reset: () => MeetingLiveViewState;
  invalidate: () => MeetingLiveViewState;
  reconcile: (
    context: MeetingReconcileContext,
    callbacks: MeetingReconcileCallbacks,
  ) => Promise<MeetingLiveViewState>;
} {
  let state: MeetingLiveViewState = {
    turns: null,
    overlayRevision: "",
    inFlight: false,
    revision: 0,
  };

  const publish = (): MeetingLiveViewState => ({ ...state });

  return {
    read: publish,

    reset(): MeetingLiveViewState {
      void clearLatestMeetingOverlay();
      state = {
        ...state,
        turns: null,
        overlayRevision: "",
        revision: state.revision + 1,
      };
      return publish();
    },

    invalidate(): MeetingLiveViewState {
      state = { ...state, revision: state.revision + 1 };
      return publish();
    },

    async reconcile(
      context: MeetingReconcileContext,
      callbacks: MeetingReconcileCallbacks,
    ): Promise<MeetingLiveViewState> {
      if (state.inFlight || context.booting || context.setupRequired) return publish();
      if (context.ownerKind !== "meeting" && !context.closeAfterExistingStop) return publish();

      const revision = state.revision;
      state = { ...state, inFlight: true };
      try {
        const result = await readMeetingReconciliation(
          state.turns,
        );
        if (!result || revision !== state.revision) return publish();

        callbacks.onStatus(result.status);
        // The callback may reject a stale Meeting-only status and invalidate
        // this poll. Never publish its older private captions afterward.
        if (revision !== state.revision) return publish();
        const overlayRevision = await publishLatestMeetingOverlay(
          result.turns,
          state.overlayRevision,
        );
        if (revision !== state.revision) {
          // A reset can happen while a previous Meeting caption publication is in flight.
          await clearLatestMeetingOverlay();
          return publish();
        }

        state = {
          ...state,
          turns: result.turns,
          overlayRevision,
        };

        if (result.unavailable) {
          callbacks.onNotice("Meeting translation is temporarily unavailable.");
        } else if (!result.status.has_session && context.closeAfterExistingStop) {
          await callbacks.onStoppedForClose();
        }
      } finally {
        state = { ...state, inFlight: false };
      }
      return publish();
    },
  };
}
