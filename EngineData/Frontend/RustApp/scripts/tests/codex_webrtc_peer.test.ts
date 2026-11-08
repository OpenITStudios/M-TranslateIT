import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  CodexRealtimePeer, validCodexSdp,
} from "./fixtures/codexRealtimePeer.ts";

const LEASE = { generation: 13, sourceEpoch: 3 };
const SDP = [
  "v=0",
  "o=localhost 1 1 IN IP4 127.0.0.1",
  "m=audio 9 UDP/TLS/RTP/SAVPF 111",
  "m=application 9 UDP/DTLS/SCTP webrtc-datachannel",
  "",
].join("\r\n");

class FakeChannel extends EventTarget {
  readyState: RTCDataChannelState = "connecting";

  open(): void {
    this.readyState = "open";
    this.dispatchEvent(new Event("open"));
  }

  close(): void {
    if (this.readyState === "closed") return;
    this.readyState = "closed";
    this.dispatchEvent(new Event("close"));
  }
}

class FakePeer extends EventTarget {
  connectionState: RTCPeerConnectionState = "new";
  iceConnectionState: RTCIceConnectionState = "new";
  iceGatheringState: RTCIceGatheringState = "new";
  localDescription: RTCSessionDescriptionInit | null = null;
  remoteDescription: RTCSessionDescriptionInit | null = null;
  channel = new FakeChannel();
  createdChannel = "";
  ordered = false;
  audioDirection = "";
  failOnAnswer = false;

  addTransceiver(kind: string, init: RTCRtpTransceiverInit): RTCRtpTransceiver {
    assert.equal(kind, "audio");
    this.audioDirection = init.direction ?? "";
    return {} as RTCRtpTransceiver;
  }

  createDataChannel(label: string, init?: RTCDataChannelInit): RTCDataChannel {
    this.createdChannel = label;
    this.ordered = init?.ordered === true;
    return this.channel as unknown as RTCDataChannel;
  }

  async createOffer(): Promise<RTCSessionDescriptionInit> {
    return { type: "offer", sdp: SDP };
  }

  async setLocalDescription(offer: RTCSessionDescriptionInit): Promise<void> {
    this.localDescription = offer;
    this.iceGatheringState = "complete";
    this.dispatchEvent(new Event("icegatheringstatechange"));
  }

  async setRemoteDescription(answer: RTCSessionDescriptionInit): Promise<void> {
    this.remoteDescription = answer;
    this.connectionState = this.failOnAnswer ? "failed" : "connected";
    this.iceConnectionState = this.failOnAnswer ? "failed" : "connected";
    this.dispatchEvent(new Event("connectionstatechange"));
    this.dispatchEvent(new Event("iceconnectionstatechange"));
    if (!this.failOnAnswer) this.channel.open();
  }

  close(): void {
    this.connectionState = "closed";
    this.iceConnectionState = "closed";
    this.channel.close();
    this.dispatchEvent(new Event("connectionstatechange"));
  }
}

test("P24 prepares bounded offer, accepts matching answer, and owns Stop", async () => {
  const mock = new FakePeer();
  const rtc = new CodexRealtimePeer(LEASE, () => mock as unknown as RTCPeerConnection);
  assert.equal(rtc.phase(), "idle");
  const offer = await rtc.prepareOffer(LEASE);
  assert.equal(offer, SDP);
  assert.equal(mock.audioDirection, "recvonly");
  assert.equal(mock.createdChannel, "oai-events");
  assert.equal(mock.ordered, true);
  assert.equal(rtc.phase(), "awaiting-answer");
  await rtc.applyAnswer(LEASE, SDP);
  assert.equal(mock.remoteDescription?.type, "answer");
  assert.equal(rtc.phase(), "signalling-ready");
  rtc.close();
  assert.equal(rtc.phase(), "closed");
  assert.equal(mock.channel.readyState, "closed");
  rtc.close(); // idempotent Stop
});

test("P24 lease mismatch invalidates peer instead of reusing SDP", async () => {
  const mock = new FakePeer();
  const rtc = new CodexRealtimePeer(LEASE, () => mock as unknown as RTCPeerConnection);
  await rtc.prepareOffer(LEASE);
  await assert.rejects(
    rtc.applyAnswer({ generation: LEASE.generation, sourceEpoch: 4 }, SDP),
    /codex_webrtc:stale_session/,
  );
  assert.equal(rtc.phase(), "revoked");
  assert.equal(mock.connectionState, "closed");
  await assert.rejects(rtc.prepareOffer(LEASE), /codex_webrtc:session_closed/);
});

test("P24 reject invalid SDP and failed transport before media admission", async () => {
  assert.equal(validCodexSdp(SDP), true);
  assert.equal(validCodexSdp("v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\n"), false);
  assert.equal(validCodexSdp(SDP + "X".repeat(16_384)), false);
  assert.equal(validCodexSdp(SDP + "\0"), false);
  assert.throws(() => new CodexRealtimePeer({ generation: 0, sourceEpoch: 1 }), /lease_invalid/);
  const invalid = new CodexRealtimePeer(LEASE, () => new FakePeer() as unknown as RTCPeerConnection);
  await invalid.prepareOffer(LEASE);
  await assert.rejects(invalid.applyAnswer(LEASE, "invalid"), /answer_invalid/);
  assert.equal(invalid.phase(), "revoked");

  const mock = new FakePeer();
  mock.failOnAnswer = true;
  const failed = new CodexRealtimePeer(LEASE, () => mock as unknown as RTCPeerConnection);
  await failed.prepareOffer(LEASE);
  await assert.rejects(failed.applyAnswer(LEASE, SDP), /negotiation_failed/);
  assert.equal(failed.phase(), "revoked");
});

test("P25 negotiates one SDP offer using a caller-owned signal exchange", async () => {
  const mock = new FakePeer();
  const peer = new CodexRealtimePeer(LEASE, () => mock as unknown as RTCPeerConnection);
  let requests = 0;
  await peer.negotiate(LEASE, async (offer, lease, signal) => {
    requests++;
    assert.equal(offer, SDP);
    assert.deepEqual(lease, LEASE);
    assert.equal(signal.aborted, false);
    return SDP;
  });
  assert.equal(requests, 1);
  assert.equal(peer.phase(), "signalling-ready");
  peer.close();
});

test("P25 Stop during native SDP exchange rejects a late answer", async () => {
  const mock = new FakePeer();
  const peer = new CodexRealtimePeer(LEASE, () => mock as unknown as RTCPeerConnection);
  let requests = 0;
  await assert.rejects(
    peer.negotiate(LEASE, async (_offer, _lease, signal) => {
      requests++;
      peer.close();
      assert.equal(signal.aborted, true);
      return SDP;
    }),
    /codex_webrtc:signalling_unavailable/,
  );
  assert.equal(requests, 1);
  assert.equal(peer.phase(), "closed");
  assert.equal(mock.connectionState, "closed");
  await assert.rejects(peer.prepareOffer(LEASE), /codex_webrtc:session_closed/);
});

test("P24 drops remote cloud audio and unexpected data channels", async () => {
  const mock = new FakePeer();
  const rtc = new CodexRealtimePeer(LEASE, () => mock as unknown as RTCPeerConnection);
  await rtc.prepareOffer(LEASE);
  let stopped = false;
  const remoteTrack = {
    enabled: true,
    stop: () => { stopped = true; },
  };
  const trackEvent = new Event("track");
  Object.defineProperty(trackEvent, "track", { value: remoteTrack });
  mock.dispatchEvent(trackEvent);
  assert.equal(remoteTrack.enabled, false);
  assert.equal(stopped, true);

  const extra = new FakeChannel();
  const unexpected = new Event("datachannel");
  Object.defineProperty(unexpected, "channel", { value: extra });
  mock.dispatchEvent(unexpected);
  assert.equal(extra.readyState, "closed");
  assert.equal(rtc.phase(), "revoked");

  const implementation = readFileSync(
    new URL("./fixtures/codexRealtimePeer.ts", import.meta.url), "utf8",
  );
  assert.doesNotMatch(implementation, /\bgetUserMedia\s*\(|\bnew\s+(AudioContext|MediaStream)\s*\(/);
  assert.doesNotMatch(implementation, /\.send\(|\.play\(/);
  assert.match(implementation, /new RTCPeerConnection\(\{ iceServers: \[\] \}\)/);
});
