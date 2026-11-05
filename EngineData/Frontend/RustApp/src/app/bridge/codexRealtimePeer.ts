/**
 * WebView2 WebRTC *signalling peer* for a future owned Codex V3 session.
 *
 * No Tauri command or production Meeting path imports this module yet.
 * It does not request microphone permissions, create a second capture
 * source, send audio, play server speech, or grant Voice entitlement.
 *
 * The canonical Rust Meeting owner must supply a fresh generation/epoch
 * and must revoke this peer on Stop, Mute or source changes. The existing
 * Rust PCM/appendAudio path owns all future microphone samples.
 */

export type CodexRtcLease = Readonly<{
  generation: number;
  sourceEpoch: number;
}>;

export type CodexRtcPhase =
  | "idle"
  | "offering"
  | "awaiting-answer"
  | "negotiating"
  | "signalling-ready"
  | "closed"
  | "revoked";

const MAX_SDP_BYTES = 16 * 1024;
const NEGOTIATION_TIMEOUT_MS = 10_000;
const EVENTS_CHANNEL_LABEL = "oai-events";

function validLease(value: CodexRtcLease): boolean {
  return Number.isSafeInteger(value.generation) && value.generation > 0
    && Number.isSafeInteger(value.sourceEpoch) && value.sourceEpoch > 0;
}

/** Matches the bounded WebRTC offer/answer shape admitted by Rust. */
export function validCodexSdp(sdp: unknown): sdp is string {
  return typeof sdp === "string"
    && sdp.length > 0
    && sdp.length <= MAX_SDP_BYTES
    && /^[\x09\x0a\x0d\x20-\x7e]+$/.test(sdp)
    && sdp.startsWith("v=0\r\n")
    && sdp.includes("\r\nm=audio ")
    && sdp.includes("\r\nm=application ");
}

export class CodexRealtimePeer {
  private readonly lease: CodexRtcLease;
  private readonly factory: () => RTCPeerConnection;
  private readonly cancelled = new AbortController();
  private connection: RTCPeerConnection | null = null;
  private events: RTCDataChannel | null = null;
  private current: CodexRtcPhase = "idle";

  constructor(
    lease: CodexRtcLease,
    factory: () => RTCPeerConnection = () => new RTCPeerConnection({ iceServers: [] }),
  ) {
    if (!validLease(lease)) {
      throw new Error("codex_webrtc:lease_invalid");
    }
    this.lease = { generation: lease.generation, sourceEpoch: lease.sourceEpoch };
    this.factory = factory;
  }

  phase(): CodexRtcPhase {
    return this.current;
  }

  private requireLease(lease: CodexRtcLease): void {
    if (!validLease(lease)
      || lease.generation !== this.lease.generation
      || lease.sourceEpoch !== this.lease.sourceEpoch
    ) {
      this.revoke();
      throw new Error("codex_webrtc:stale_session");
    }
    if (this.cancelled.signal.aborted) {
      throw new Error("codex_webrtc:session_closed");
    }
  }

  private shutdown(state: "closed" | "revoked"): void {
    if (this.current === "closed" || this.current === "revoked") return;
    this.current = state;
    this.cancelled.abort();
    // The event channel never forwards untrusted data into the app.
    this.events?.close();
    this.connection?.close();
    this.events = null;
    this.connection = null;
  }

  close(): void {
    this.shutdown("closed");
  }

  revoke(): void {
    this.shutdown("revoked");
  }

  private async bounded<T>(task: Promise<T>, blocker: string): Promise<T> {
    if (this.cancelled.signal.aborted) {
      throw new Error("codex_webrtc:session_closed");
    }
    return new Promise<T>((resolve, reject) => {
      let done = false;
      const finish = (action: () => void): void => {
        if (done) return;
        done = true;
        clearTimeout(timer);
        this.cancelled.signal.removeEventListener("abort", onAbort);
        action();
      };
      const onAbort = (): void => finish(() => reject(new Error("codex_webrtc:session_closed")));
      const timer = setTimeout(() => finish(() => reject(new Error(blocker))), NEGOTIATION_TIMEOUT_MS);
      this.cancelled.signal.addEventListener("abort", onAbort, { once: true });
      if (this.cancelled.signal.aborted) onAbort();
      void task.then(
        value => finish(() => resolve(value)),
        () => finish(() => reject(new Error(blocker))),
      );
    });
  }

  private async until(
    target: EventTarget,
    eventName: string,
    isReady: () => boolean,
    blocker: string,
  ): Promise<void> {
    if (isReady()) return;
    let cleanup = (): void => {};
    const pending = new Promise<void>(resolve => {
      const check = (): void => {
        if (isReady()) resolve();
      };
      target.addEventListener(eventName, check);
      cleanup = () => target.removeEventListener(eventName, check);
      check();
    });
    try {
      await this.bounded(pending, blocker);
    } finally {
      cleanup();
    }
  }

  /**
   * Generate one complete ICE-gathered offer. No getUserMedia / remote URL.
   * The audio m-line has no sending track: Rust still owns the only mic.
   * Future native PCM is sent by the existing app-server appendAudio path.
   */
  async prepareOffer(lease: CodexRtcLease): Promise<string> {
    this.requireLease(lease);
    if (this.current !== "idle") {
      throw new Error("codex_webrtc:offer_already_started");
    }
    this.current = "offering";
    try {
      const peer = this.factory();
      this.connection = peer;
      // No external STUN/TURN service is silently configured. ICE
      // reachability on the target WebView2 is NOT verified here.
      peer.addTransceiver("audio", { direction: "recvonly" });
      const channel = peer.createDataChannel(EVENTS_CHANNEL_LABEL, { ordered: true });
      this.events = channel;
      peer.addEventListener("datachannel", (event: Event) => {
        const incoming = event as RTCDataChannelEvent;
        incoming.channel.close();
        this.revoke();
      });
      peer.addEventListener("track", (event: Event) => {
        const remote = event as RTCTrackEvent;
        // Cloud-generated speech MUST NOT enter TranslateIT playback.
        remote.track.enabled = false;
        remote.track.stop();
      });
      peer.addEventListener("connectionstatechange", () => {
        if (["failed", "disconnected", "closed"].includes(peer.connectionState)) this.revoke();
      });
      peer.addEventListener("iceconnectionstatechange", () => {
        if (["failed", "disconnected", "closed"].includes(peer.iceConnectionState)) this.revoke();
      });
      channel.addEventListener("close", () => this.revoke());
      channel.addEventListener("error", () => this.revoke());
      // Realtime events are read by the existing Rust AppServer, not via
      // this RTC channel. Never parse or replay arbitrary channel frames.
      channel.addEventListener("message", () => { /* intentionally ignored */ });

      const offer = await this.bounded(peer.createOffer(), "codex_webrtc:offer_failed");
      if (this.current !== "offering" || offer.type !== "offer" || !validCodexSdp(offer.sdp)) {
        throw new Error("codex_webrtc:offer_invalid");
      }
      await this.bounded(peer.setLocalDescription(offer), "codex_webrtc:local_description_failed");
      await this.until(
        peer, "icegatheringstatechange",
        () => peer.iceGatheringState === "complete", "codex_webrtc:ice_timeout",
      );
      const sdp = peer.localDescription?.sdp;
      if (this.current !== "offering" || !validCodexSdp(sdp)) {
        throw new Error("codex_webrtc:offer_invalid");
      }
      this.current = "awaiting-answer";
      return sdp;
    } catch {
      this.revoke();
      throw new Error("codex_webrtc:offer_unavailable");
    }
  }

  /**
   * Apply only the answer for the SAME source generation and epoch.
   * A connected ICE peer plus open oai-events is signalling readiness,
   * NOT Voice authorization or proof that audio entered Codex.
   */
  async applyAnswer(lease: CodexRtcLease, answerSdp: string): Promise<void> {
    this.requireLease(lease);
    if (this.current !== "awaiting-answer" || !validCodexSdp(answerSdp)
      || !this.connection || !this.events
    ) {
      this.revoke();
      throw new Error("codex_webrtc:answer_invalid");
    }
    const peer = this.connection;
    const channel = this.events;
    this.current = "negotiating";
    try {
      await this.bounded(
        peer.setRemoteDescription({ type: "answer", sdp: answerSdp }),
        "codex_webrtc:remote_description_failed",
      );
      await Promise.all([
        this.until(
          peer, "connectionstatechange",
          () => peer.connectionState === "connected", "codex_webrtc:ice_connection_timeout",
        ),
        this.until(
          channel, "open",
          () => channel.readyState === "open", "codex_webrtc:events_channel_timeout",
        ),
      ]);
      if (this.current !== "negotiating") {
        throw new Error("codex_webrtc:session_closed");
      }
      this.current = "signalling-ready";
    } catch {
      this.revoke();
      throw new Error("codex_webrtc:negotiation_failed");
    }
  }
}
