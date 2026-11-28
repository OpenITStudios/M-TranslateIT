// Scheduling only. The live controller still owns revision fences, status
// and captions; the backend remains the sole runtime authority.
type ReconcileClock = {
  set: (callback: () => void, delayMs: number) => number;
  clear: (timer: number) => void;
};

export function createMeetingReconcileSchedule(
  read: () => void | Promise<void>,
  clock: ReconcileClock,
  eventWindowMs: number,
  fallbackMs: number,
): { request: () => void; dispose: () => void } {
  let disposed = false;
  let timer: number | null = null;
  let inFlight = false;
  let pending = false;

  function arm(delayMs: number): void {
    timer = clock.set(() => {
      timer = null;
      void run();
    }, delayMs);
  }

  async function run(): Promise<void> {
    if (disposed) return;
    inFlight = true;
    pending = false;
    try {
      await read();
    } catch {
      // The read owner reports unverified state. Keep fallback available.
    } finally {
      inFlight = false;
      if (!disposed) arm(pending ? eventWindowMs : fallbackMs);
    }
  }

  return {
    request(): void {
      if (disposed || pending) return;
      pending = true;
      // Events during IPC retain exactly one follow-up, not a promise queue.
      if (inFlight) return;
      if (timer !== null) clock.clear(timer);
      // Fixed window from first event; later events cannot postpone it.
      arm(eventWindowMs);
    },
    dispose(): void {
      disposed = true;
      pending = false;
      if (timer !== null) clock.clear(timer);
      timer = null;
    },
  };
}
