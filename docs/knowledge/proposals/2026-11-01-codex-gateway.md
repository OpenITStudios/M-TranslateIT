---
id: document.knowledge.proposals.codex-gateway
class: DOCUMENT
domain: knowledge
role: ARCHITECTURE
authority: REFERENCE
lifecycle: ACTIVE
---

# Codex Gateway — Optional Meeting Route (Architecture and Admission)

**Decision:** 2026-11-01. **Source baseline:** TranslateIT Local at
`3bfc04460cbe246fb382230ae223580722cc0f6e`.
**Evidence:** STATIC_SOURCE only; no Codex login, API inference, audio, Windows build,
latency or hardware tests executed for this work.

## Goal and boundaries

The current Local Meeting route remains the default and unchanged:

    Microphone -> finalized PCM -> Faster-Whisper -> MiLMMT -> GPT-SoVITS
     -> existing serialized virtual-microphone playback

The optional future route is:

    Same microphone owner -> verified Codex voice-to-translated-text capability
     -> final translation admission -> existing GPT-SoVITS -> same playback

This is a *choice at Meeting Start*, not two simultaneously active translation
engines, a silent cloud fallback, a second microphone/Meeting owner or another
voice engine. Original Microphone, Mute, transcript ownership and canonical Stop
remain single-source. Initial scope is Indonesian outbound to English voice;
incoming Codex caption support requires independent proof and must not
implicitly resurrect MiLMMT in an allegedly GPU-light route.

User authorization is for **included ChatGPT/Codex usage**, not paid credits,
a metered API key or automatic upsell. Private meeting audio sent to the cloud
requires explicit selection, consent and visible status before activation.
Network usage is a product behavior, not ChatGPT's GitHub editing/execution
context. ChatGPT repository work itself does not execute cloud inference.

## Phase 1: connected account introspection — source candidate

One Rust command owner under `commands/codex_gateway.rs`, divided only into
`protocol.rs` (pure JSONL admission) and `transport.rs` (one owned stdio
Codex child). No second Python worker, MCP provider, HTTP port, timer-based
profiler, or persistent credential copy.

Only explicit Diagnostics **Check Codex connection** starts a child. The
application does not automatically start it at boot or restart it after error.
It looks for an absolute regular native `codex.exe` through
`TRANSLATEIT_CODEX_EXECUTABLE`, the established `CODEX_CLI_PATH`, or a
bounded set of direct/native npm package paths under absolute PATH directories.
It will not execute `.cmd`, `.bat` or a shell, scan recursively, or take over
another Codex installation's already-running process. Release packaging/discovery remains
unverified.

The child runs `codex app-server --listen stdio://`. JSONL transport has:
- bounded 8 KiB request, 256 KiB received frame, 32 queued events;
- one in-flight request, exact RPC ID matching, eight-second request deadline;
- explicit cancellation, atomic generation fencing and no unbounded retry;
- redacted, enumerated failures; no raw stdout/stderr/chat/body/credential logs;
- one owned reader and child, verified teardown before clean-app-exit marker.

The diagnostic allowlist contains only `initialize`, `account/gatewayOAuth/read`,
`account/read` and `account/rateLimits/read`. The read-only gateway policy probe
is required before account/usage reads. `initialized` follows initialize. Account
type must be `chatgpt`; `ordinaryUsageAllowed` true is required to *record*
included-usage permission. Null/missing means unknown and is not permission.
`supportsLunaReserve=false`; inherited API keys and alternate API base
environment overrides are removed. Source tests exercise these controls.

No `thread/start`, `turn/start`, microphone, WebRTC, voice, text
translation or normal Meeting command calls the gateway in phase 1.
`realtime_audio_verified=false` and `meeting_route_enabled=false` are
immutable truth at this phase even if the account probe succeeds.

An idle account-change notification triggers fresh account and included-usage
reads on the same owned process. A failed recheck blocks that process; a stale
authenticated state must not survive a changed account. App Exit asks this owner to stop after Meeting resources are released
and refuses the clean marker if a child or reader remains unverified. Status
does not include account email or private upstream response bodies.

## Phase 2: source-derived realtime adapter — STAGED / NOT LIVE

P2 now contains a pure Rust event observer and a bounded PCM framing queue in
`commands/codex_gateway/realtime.rs`. It consumes the current upstream
role-bearing `thread/realtime/item/completed` schema, preserving `threadId`,
`realtimeSessionId`, item ID, role and complete text. It returns
`UnverifiedAssistantText`, never a committed translation; an assistant
message is not proof of speech-source correspondence or faithful translation.

Uncorrelated flat `transcript/done` and all draft `delta` events remain
noncommittable; `user` transcript and `assistant` output are distinct, and
cloud output audio is explicitly ignored. A duplicate item, foreign thread,
wrong session, missing role, unknown event, remote error, or stale event after
Stop revokes the observer. The bounded 256-ID evidence window is intentionally
not a long-Meeting production dedupe strategy: canonical Meeting sequence
correlation must replace it before activation.

The offline input framer accepts only contiguous PCM16 mono 24 kHz and at most
4,800 samples (200 ms) per frame; it queues at most four frames and emits
the upstream audio-chunk fields with Base64. A clock gap, overflow or revocation
invalidates the queue; nothing is silently skipped. Source clocks are not
assumed to be 24 kHz in the native capture pipeline: an authorized resampling
and sample-clock adapter is still required before live use.

These source components are covered by authored synthetic Rust contract cases.
`protocol.rs` now preserves notification parameters instead of discarding
them. During the **existing P1 diagnostic connection**, any unexpected realtime
notification is rejected; no Voice request is sent, and the account probe
revalidates a racing account/workspace-change event at most once. There is no
network/audio inference evidence from these fixtures.

Use the *installed and version-pinned* Codex CLI's actual protocol schema;
never synthesize unsupported JSON-RPC or treat a source type as entitlement.
The reference source was compared against `openai/codex` commit
`67f0994b532b8740f14b5b738af7b7b2a915b1d6` at
`codex-rs/app-server-protocol/src/protocol/common.rs` and
`v2/realtime.rs`. The user's installed CLI version remains unverified.
In current upstream, realtime V2 text-only output is unavailable for ChatGPT
WebRTC V1/V3, and the direct WebSocket V2 route requires API-key auth. A
ChatGPT-account WebRTC route may emit translated assistant text *alongside*
model audio; verify that its role-bearing output is actual translation,
not source transcription or an answer to spoken instructions.

**Admission conditions (all required):**
1. ChatGPT auth with no paid provider and confirmed account/workspace identity.
2. Realtime-audio access and included-usage charging verified for that account;
   ordinary token quota alone is insufficient for Voice billing proof.
3. A real request produces complete, role-bearing, correctly translated text,
   with no tool execution and no unwanted cloud audio playback.
4. Demonstrated safe cancellation, audio continuity, rate-limit behavior and
   process cleanup. UNKNOWN means route not selectable.
5. A target-Windows source-to-virtual-microphone latency and quality baseline.

Audio delivery must reuse Rust CPAL capture; compare *one* App Server
`thread/realtime/appendAudio` path against an actual authenticated WebRTC
media track before choosing one. No second simultaneous capture. Input may be
chunked (initial experiment 200 ms) and must pass format/clock negotiation,
bounded queue/backpressure, source-range and epoch consistency. A gap marks
the affected utterance invalid; it never silently drops words.

## Phase 3: strict source/assistant correlation — SOURCE CANDIDATE, NOT ADMITTED

`commands/codex_gateway/correlation.rs` now contains a bounded, one-flight
correlation evidence gate. Its source lease records the canonical Meeting
generation, source epoch, utterance sequence and exact 24 kHz PCM window.
It accepts one role-bearing completed `user` item followed by one role-bearing
completed `assistant` item, validates distinct provider IDs, requires bounded
non-empty text and rejects overlapping input, duplicates, missing source,
invalid sample ranges and results after Stop.

The result type is intentionally named `UnverifiedPair`. Private speech and account payloads are redacted in the gateway's RPC and observation `Debug` formatters. It is **not**
`TranslationCommit`: Codex's documented item-completed event has no
authoritative `replyToSourceItemId`, source sample-range identity, or
Meeting utterance sequence. Two items appearing near each other do not prove
a causal or semantically correct translation. Consequently the evidence gate
retires itself after one pair rather than allowing late responses from an
earlier utterance to be attributed to the next one. The event adapter and
correlation gate are connected by a synthetic Rust regression scenario,
without adding any production TTS or Meeting call.

**Not yet satisfied:** a real, included-usage Voice entitlement for this
account; exact installed CLI protocol; sample-clock synchronization and
end-of-source acknowledgement; a demonstrated source-to-assistant mapping
that remains unambiguous with interruptions, overlap and 30-minute sessions;
real Indonesian/English semantic evaluation. None can be inferred from
`ordinaryUsageAllowed: true` or from source-only mock tests. P3 is not
complete as a live capability, and there is no path to enable it by manually
asserting an unverified success flag.

## Phase 4: persisted route selection and native fail-closed Start — SOURCE CANDIDATE

The Rust settings schema now stores `translation_engine = local | codex`,
with `local` as the default for existing configuration and unknown values.
The TypeScript DTO, clone/default projection, and Settings selector share the
same value and schema. A change while a Meeting/Mic Test/voice-recording owner
or unresolved Meeting cleanup holds resources is rejected by the native
settings owner. The selection is a preference, **not** proof of readiness or
a provider entitlement.

The existing `meeting_session/preflight.rs` reports a dedicated
`codex_voice_not_admitted` blocker for Codex without probing Local helper
readiness. `meeting_session/lifecycle.rs` refuses the selected Codex route
before Local worker recovery or microphone/device preparation.
`meeting_session/audio_source.rs` independently refuses switching
Original Microphone to translated output in Codex mode, preventing a bypass.
The raw Original Microphone path still uses its unchanged resource checks.

The frontend's automatic worker startup paths skip cold Local AI startup
when Codex is selected; explicit My Voice activities retain access to the
existing helper. Selection does not forcibly unload an already-running local
worker, and makes no claim that GPU memory was released.
Neither initial Codex selection nor the Codex Diagnostics account probe
starts Voice or sends audio to cloud inference. This is strictly source work;
UI, Rust compilation and actual Meeting acceptance are NOT EXECUTED.

## Phase 5: Realtime WebRTC RPC lifecycle and bounded audio ACKs — STAGED ONLY

The source-only `codex_gateway/realtime_rpc.rs` now builds precisely scoped
`thread/realtime/start`, `thread/realtime/appendAudio`, and
`thread/realtime/stop` JSON-RPC envelopes from the upstream V3
`ThreadRealtimeStartParams`/`ThreadRealtimeAppendAudioParams`/
`ThreadRealtimeStopParams` shapes. It selects `version = v3`,
`outputModality = audio`, an externally generated WebRTC offer SDP, and
disables unrelated startup context. This is **not text-only realtime**: under
the reviewed Codex version, V3 WebRTC still uses audio output and any
role-bearing text remains unverified until a live run establishes its meaning.
A source follow-up at upstream `openai/codex` commit
`f69c660a83b5279653085390b9be9aad7e6fd2d1` confirms the core
`realtime_conversation.rs` rejects `outputModality=text` for any
version other than V2, while WebRTC accepts V1/V3 only.
Do not switch the V3 start envelope to text: it would be rejected before
the desired STT/translation output could exist. The user-approved target is
**audio captured locally → Codex STT+translation → English translation text
→ the already-selected local GPT-SoVITS voice → one Meeting microphone**.
The Codex-generated audio associated with the currently supported V3
output mode must be ignored; only independently validated source/assistant
transcripts may be considered for later admission. This does not prove
audio input entitlement under the ChatGPT subscription, source/response
causality, provider translation fidelity, low latency or zero additional
charges. No production Voice route is enabled.
The RPC encoder does not create a Codex thread, connect to a WebRTC peer,
or authenticate Voice. A separate WebView2 WebRTC signalling candidate
exists below, but no production Meeting/Codex owner invokes it.

The offline RealtimeObserver also distinguishes a typed
`realtimeSessionClosed` item with `outcome=ended` (clean) from
`outcome=failed` or missing/unknown outcomes (fatal, revoke).
A failed Voice session cannot be reported as a clean completion.
The synthetic Rust fixtures cover these cases without executing Codex.

The existing Python helper now has a Codex-specific source-only
**GPT-SoVITS voice-only readiness probe**. It calls only the existing
`voice_actor_preflight` and `voice_actor_synthesize` handlers with a
fixed English fixture, validates the exact helper generation, actor token
and requested output path, checks a minimal RIFF/WAVE header and removes
only that owned fixture file. It explicitly does **not** call
`asr_preload`, `transcribe`, `translation_preload` or `translate`;
the full Local ASR/MiLMMT/TTS readiness cache is not overwritten.
The helper entrypoint checks that Codex is the selected engine.
This is not wired to a production Codex Meeting Start; synthesized
voice quality and native runtime behavior remain unverified.

A bounded streaming mono-f32 microphone adapter now exists in the
same `realtime.rs` owner: `Pcm24kResampler` converts supported
8–96-kHz capture sample rates to the P5 24-kHz PCM16 sample clock,
preserves one source sample across chunk boundaries, and rejects input
gaps, nonfinite input, overlong frames and downstream queue overflow.
It is a non-callback integration seam, not a running microphone pipeline,
measured audio-quality proof or finished WebRTC owner. Do not invoke
allocation-heavy conversion from a CPAL real-time callback.

The existing AppServer JSONL writer and reader now have a **sealed typed
realtime control path**, not a second Codex process: diagnostic requests
still use their strict 8-KiB method allowlist, while a
`RealtimeRpc::RpcFrame` with a separately proven
`VerifiedMediaAuthority` can use its method-specific 32-KiB Start,
24-KiB appendAudio or 1-KiB Stop frame budget.
`AppServer.queue_realtime_frame` queues one validated frame without
waiting for its model ACK, and `next_realtime_event` surfaces responses
and realtime notifications on the same bounded reader; an account
revision change revokes that path before stale events are returned.
Real media auth has **no production constructor**, and
`protocol::request` remains diagnostic-only. The currently initialized
diagnostic App Server advertises `experimentalApi=false`, so this
source-only control path cannot start Voice today.

The candidate V3 Start also selects `clientManagedHandoffs=true`
(the option used by Codex's upstream TUI) to avoid implicit delegation
of spoken meeting content to the coding agent. This does **not**
produce an admitted translation: safe client-managed translation,
pre-execution tool restriction and final-text source binding are still
missing, and the provider must not receive private speech until those
permissions exist.

## P24: WebView2 WebRTC SDP/ICE peer — SOURCE CANDIDATE ONLY

The existing Tauri WebView renderer now has a dedicated, uninvoked
`src/app/bridge/codexRealtimePeer.ts` signalling helper. It creates
one browser-native `RTCPeerConnection`, negotiates an audio SDP m-line
and the reliable ordered `oai-events` data channel, waits for ICE
gathering before returning a **bounded, ASCII SDP offer**, accepts only
a matching-generation/source-epoch remote SDP answer, and requires
both the ICE/peer connection and event channel to be open before
reporting `signalling-ready`. It has deterministic Stop/revoke,
connection-failure, unexpected-channel and cancellation behavior.
Both SDP text and native peer state are deliberately excluded from
diagnostics, and remote cloud audio tracks are stopped on arrival.

**No duplicate microphone:** this helper never invokes `getUserMedia`,
creates an AudioContext or installs a local outgoing track. Its
`recvonly` audio m-line is for negotiating the peer; the existing Rust
24-kHz `appendAudio` queue is the intended audio source. In upstream
Codex, the native voice host instead attaches an outgoing audio track,
so compatibility of a recvonly SDP plus sideband appendAudio is
**UNVERIFIED**. Real provider acceptance, ICE reachability, audio media,
and Windows WebView2 capability all remain UNKNOWN. No default STUN/TURN
server or third-party media relay is introduced.

No frontend or Rust production caller constructs this peer yet.
`codex_gateway.rs` still initializes `experimentalApi=false` and
the sealed `VerifiedMediaAuthority` has no production constructor;
no SDP is sent and no audio/translation is generated. The data channel
is **not** a second event processor: its messages are ignored while the
existing Rust App Server owns role-bearing translation observations.
A peer-connected state is not account Voice entitlement, safe
tool-free execution, source-response correlation, or Meeting readiness.
A later authorized owner must join SDP across Tauri/Rust using the same
Meeting generation, source epoch, cancellation and Stop transaction.
Do not promote the candidate to Live or GPT-SoVITS on SDP alone.

Upstream source pinned for this peer contract:
`openai/codex@af92ae158d2c9cb79cf745d609694c5a76d527b0`,
`codex-rs/app-server-protocol/src/protocol/v2/realtime.rs` (WebRTC
SDP) and `codex-rs/voice-host/src/transport.rs` (ordered
`oai-events` channel).

Before promotion, the single owned session must correlate actual
realtime items with authoritative source windows and prove safe Stop,
transport failures, media consent, and input speech privacy. Typed SDP
or writer receipts do not prove a working translation.

## P25: one-owner WebRTC SDP signalling handshake — SOURCE CANDIDATE ONLY

The existing `AppServer` owner in
`commands/codex_gateway/transport.rs` now has an offline-only
`exchange_realtime_offer` method. It accepts a WebView-generated SDP
offer using the existing `RealtimeRpc::start` encoder and exact owned
writer, then consumes the matching Start RPC ACK,
`thread/realtime/started`, and `thread/realtime/sdp` events on the
**same** bounded reader. ACK, started and SDP may arrive in any order.
Only after all three match does it return the bounded answer SDP;
no media-ready, translator, TTS, or Meeting authority is created.
Wrong IDs, unrecognized or premature events, cancellation, account
revision change, or handshake timeout revoke the `RealtimeRpc` state.
A failed in-flight Start still requires the existing AppServer child
owner to perform definitive teardown before another session.

The P24 browser peer now offers a `negotiate(lease, exchangeOffer)`
method: prepare offer → invoke the caller-provided cancellable native
exchange once → verify the same Meeting generation/source epoch →
apply answer → check ICE/channel readiness. Late answers after Stop
are rejected, and errors close the peer. The callable source seam is
not a new Tauri command, persisted controller, third-party network
client, microphone process, or active Codex model route.

**Important integration limit:** these are two matching source seams,
not a registered production WebView↔Rust command yet. Existing
`codex_gateway.rs` still initializes `experimentalApi=false`
and no production constructor exists for the sealed
`VerifiedMediaAuthority`. No Voice Start RPC is sent, no input
audio is uploaded, no source-to-translation result is authorized,
and no Codex text is fed to GPT-SoVITS. It would be unsafe to
register a publicly callable production bridge that bypasses
these prerequisites. Additional native command admission must be
scoped to the one Meeting generation and must establish Voice,
tool-free handling, SDP transport and transcript authority first.

The synthetic Rust and TypeScript handshake-order, identity,
cancellation and close cases are **AUTHORED / NOT EXECUTED**.
Actual build, WebView, ICE, account/Voice, microphone and latency
proof remain UNKNOWN.

The source-only integration now composes the **existing** queue and wire
encoder: `AudioFrameQueue.pending_state` distinguishes empty from revoked;
`RealtimeRpc.send_queued_audio` checks generation/source epoch, cancellation,
Live state, inflight capacity and ACK deadline **before dequeuing** an
encoded 24-kHz PCM frame, then returns the bounded `appendAudio` envelope.
Overflow or invalid authority revokes both the queue and RPC lease;
`prepare_stop_with_queue` revokes any queued audio before the existing
two-sided Stop handshake. These functions do not send a network request.
The source-only Pcm24kResampler now supplies conversion for 8–96-kHz
mono f32 microphone samples, but real CPAL producer wiring,
frame scheduling, audio-clock correctness on the target device,
and WebRTC media negotiation are not implemented or proven.
Correction from a source-only interpolation audit: four 16-kHz input
samples produce **five immediately available** 24-kHz samples because
the final fractional position still needs a fifth input sample. The
next input permits two more samples. The partition-invariance fixture
and reference note are reconciled; Rust tests are AUTHORED, NOT EXECUTED.

The existing `CorrelationGate.observe_event` now feeds canonical
`RealtimeObserver` events into the one-flight source/assistant pairing
gate. Drafts and cloud audio are ignored; wrong-thread/session/duplicate,
incomplete or ambiguous ordering revokes, and even a well-formed
`UnverifiedPair` has **no Meeting/TTS authority**, provider causal link
or proof of translation fidelity. This is an integration seam for
future authorized operation, NOT a live Codex invocation or provider.

A single `RealtimeRpc` state machine requires Start RPC ACK, matching
`thread/realtime/started` event (v3 + session ID), matching SDP answer,
then a separate verified media/entitlement signal before audio admission.
That final signal requires a sealed `VerifiedMediaAuthority` type, for which
there is deliberately **no production constructor**: account/read token
quota is not proof of included Voice usage, billing or SDP connectivity.

Outgoing frames are bounded: 32 KiB for Start, 24 KiB for 200-ms PCM16 audio
RPC and 1 KiB for Stop; SDP is bounded at 16 KiB. Audio carries exact
Meeting generation and source epoch, continuous sample clock, one-channel
24-kHz format and valid Base64 padding. No more than four audio requests may
remain unacknowledged. Out-of-order acknowledgements are tracked with a
bounded FIFO; contiguous confirmed samples advance only after the
corresponding request ACK. An unacknowledged frame older than five seconds
invalidates the audio lease when the future event loop drives the deadline
check. Duplicate Start/Stop acknowledgements, unexpected audio JSON keys
and malformed Base64 padding are rejected or dropped by exact-field
reconstruction. An invalid frame, unexpected RPC ID, stale
generation, discontinuity, cancellation, overflow or protocol error blocks
new audio. No late frame is replayed to catch up.

Stop revokes audio admission immediately but retains the StopPending state
until both the matching Stop RPC ACK and the upstream session-closed event
arrive. A late audio sender cannot turn a pending Stop into a false cleanup
success. The encoder's Debug implementation exposes only RPC ID, method and
byte length, never raw SDP, audio or transcript contents. Rust unit scenarios
and an existing TypeScript source-contract test were **authored, not run**.

**Voice admission boundary unchanged:** the diagnostic `protocol::request`
allowlist contains `initialize`, `account/gatewayOAuth/read`,
`account/read` and `account/rateLimits/read`; it contains no model,
login-action or Voice RPC. `transport.rs` does not invoke the P5 codec;
there is no production WebRTC peer, audio upload, TTS emission,
experimental API activation or Meeting route. A mocked envelope may be inspected in a Rust
unit test but cannot pass the P1 RPC allowlist. This is executable source
preparation, not proof of account Voice availability or latency.

## Phase 6: bounded App Server write ownership and control dispatch — SOURCE CANDIDATE

The existing `codex_gateway/transport.rs` now owns **one writer thread**
in addition to the existing bounded stdout reader. Only the writer can touch
`ChildStdin`. The controller submits a maximum 8-KiB JSONL body through a
one-slot `sync_channel` using `try_send`, then waits for the specific
write receipt in 50-ms cancellation slices. A shared eight-second deadline
covers both write completion and the exact JSON-RPC reply, avoiding unbounded
blocking of the controller on `write_all` when the Codex child stops reading.
The writer itself uses a blocking OS pipe, but it is owned and isolated; this
is **not** an async OS pipe or a guarantee that a stuck descendant releases
every inherited handle.

The initial `initialized` notification also uses this same writer. Writer
failure permanently poisons the live request gate; `Full` and
`Disconnected` queues are explicit failures, never automatic retries.
The reader retains its 256-KiB per-frame bound and 32-event channel; the
request controller continues to admit exactly one in-flight RPC and rejects
any mismatched response ID. A single account-notification classifier is
shared by the request and idle loops; changing accounts invalidates the
previous probe while an unexpected realtime event remains forbidden.

On Stop, the gateway closes the writer queue and terminates/reaps **only its
owned Codex child**. Cleanup is complete only after *both* the writer and
reader have finished and been joined. Unknown thread ownership, thread panic
or a child-handle issue is not a successful cleanup receipt; the existing
`cleanup_incomplete` state remains authoritative. No new audio process,
tool/MCP invocation, auto-reconnect, extra model provider or paid API path
has been introduced.

The new Rust writer/queue/error cases and TypeScript bridge source assertions
are authored, **NOT EXECUTED**. No compiler, installed Codex CLI, Windows
stdio stall, target-Microphone, live meeting, or ChatGPT Voice entitlement
evidence has been collected.

## Phase 7: deterministic in-memory fake App Server — TEST SOURCE ONLY

A test-only module at `commands/codex_gateway/transport/fake_app_server_tests.rs`
now drives the SAME `write_jsonl` and `read_jsonl` production helpers with
small in-memory byte channels. The request/writer code shares its
`submit_with_receipt` wait contract; the response/notification code shares
`recv_expected_response` and `recv_idle_notification`. This is not a
second App Server executable, network client, fake account entitlement or
alternative production control owner. Only `#[cfg(test)]` references the
fixture; no diagnostic/Meeting command launches it.

The scripted test cases cover:
- JSONL fragmentation across reads and exact RPC ID matching;
- account-change notification before a successful response;
- unsolicited account notification versus mismatched response ID;
- cancellation while waiting for a late response and cancellation while a
  simulated stdin writer is blocked;
- a response arriving after the deadline, which must never satisfy a newer
  request;
- bounded writer timeout, no retries, and an explicit channel-release path;
- cleanup that stays unverified until all owned readers/writers are joined;
- denial of unadmitted Realtime notifications and the existing paid/Voice
  RPC boundary.

The simulated writer blocking is released by an explicit channel signal; it
does NOT establish that Windows pipes, external descendants, or operating
system process teardown will behave identically. Test bodies and the
TypeScript source contract have been authored but **NOT EXECUTED**.
No real Codex account, API usage, cloud audio, TTS, Windows environment or
device has been accessed. Failure paths retain enumerated, redacted errors
and cannot promote an unknown shutdown to a verified one.

## Phase 8: bounded Stop settlement and permanent cleanup latch — SOURCE CANDIDATE

The existing `codex_gateway.rs` now classifies the owner's
`stop_and_reap()` outcome into `Complete`, `Retry`, and
`Incomplete(reason)`. A healthy `Ok(false)` can be checked again
within a three-second settlement budget, with 50-ms observation intervals.
An `Err`—including a latched reader/writer join failure—is **terminal
immediately**, not retried indefinitely. The budget limits repeated
observations; it does **not** turn OS `kill`/`wait`, inherited pipe
handles or Rust `Drop` into time-bounded operations.

A terminal outcome retains a `cleanup_incomplete` status with a
redacted reason and no voice/account entitlement. The exact `AppServer`
child and remaining read/write thread handles remain within the owning
thread until the existing `Drop` backstop attempts to release them.
If its teardown blocks, the owner thread and incomplete state remain
visible; if the owner finishes, `reconcile` clears its `process_owned`
observation **without** clearing `cleanup_incomplete`. A panicked owner
also becomes an unresolved cleanup fault, not a normal recoverable
connection error. The status is latched for this application lifetime.

Connect cannot create another Codex child after `cleanup_incomplete`,
Disconnect is idempotent and cannot override the unresolved state, and
App Exit cannot produce a clean-release receipt from that state.
Diagnostics explains the latched failure without offering a misleading
repeat-Disconnect action. No automatic retry, process-name-wide kill,
fallback to paid API, or modification of the Local Meeting engine.

Rust unit cases and the existing P7 in-memory fake now include a
terminal-join-failure scenario and a deadline-exhaustion scenario, while
the TypeScript source contract checks source wiring. They are
**AUTHORED / NOT EXECUTED**. Installed Windows process ownership,
inherited stdout handles, application exit behavior, actual Codex CLI,
Voice entitlement and Meeting delivery remain unverified.

## Phase 9: installed CLI build and read-only protocol gate — SOURCE CANDIDATE

The existing single owned App Server now validates the upstream v1
`InitializeResponse` shape directly: `userAgent`, `codexHome`,
`platformFamily` and `platformOs`. The Codex build version is extracted
from the bounded first user-agent token, which upstream produces as
`<originator>/<CARGO_PKG_VERSION>`. A second `codex --version` process is
**not** spawned. Invalid, missing, malformed or `0.0.0` placeholder versions
fail closed. The Windows response and codexHome are validated, but the raw
user-agent, path, and private account data are not returned to the UI.

The upstream development workspace reports `0.0.0`, so the source does not
establish a trustworthy minimum shipped version for Codex Voice. This gate
therefore validates **version syntax and the documented account-diagnostic
protocol**, not a guessed released-version range, the experimental audio
protocol, signed binary identity, or product Voice entitlement.

Initialize with `explicitGatewayOauth: true` does not acknowledge that an
older server actually honored the no-automatic-browser policy. Before each
ChatGPT account/ordinary-usage read, the gateway now sends the upstream
params-free `account/gatewayOAuth/read` RPC. Only its read-only response for
the built-in `openai` provider with `required: false`, null `status`, and
null `error` is accepted. Missing support, another provider, required
external gateway credentials, or malformed responses block diagnostics.
The status check is repeated during an account-change re-probe. This expands
the allowlist by **one read-only account method**, not a login or inference
method. Older server builds may attempt legacy automatic sign-in before the
probe detects missing support; this source-only gate cannot guarantee
otherwise without an actual Windows execution trace. It does not prove
account quota is billed for realtime Voice.

Rust exposes only `app_server_version` and
`diagnostic_protocol_compatible`; UI explicitly describes compatibility
as **diagnostic reads only**. Unknowns and errors do not become Voice
readiness. The P7 in-memory fake has scripted support/unsupported cases for
`account/gatewayOAuth/read` and exact request ordering, along with new
Rust and TypeScript regression source contracts. These are AUTHORED /
**NOT EXECUTED**. Native Windows, actual account entitlement, Cloud Voice
or real translation behavior remain unverified.

Upstream shape was inspected at `openai/codex` commit
`67f0994b532b8740f14b5b738af7b7b2a915b1d6`, especially
`codex-rs/app-server-protocol/src/protocol/v1.rs`,
`protocol/v2/account.rs` and
`app-server/src/request_processors/initialize_processor.rs`.

## Phase 10: account/provider authority revision — SOURCE CANDIDATE

The existing Rust App Server `transport.rs` reader now advances a bounded
generation-local `AtomicU64` account-authority revision **before** enqueueing
a parsed `account/*` or `modelProvider/*` notification. The same rule
covers `config/changed` and `configRequirements/changed` if a future
server emits them. Revision arithmetic fails closed on overflow by setting
the existing transport-overflow flag. This does not add polling services,
new subprocesses, login actions, model calls or a second state owner.

The canonical `codex_gateway.rs` owner keeps the current reader revision
beside the exact generation and remembers which revision was actually
verified. A successful diagnostic snapshot requires the same revision
throughout `account/gatewayOAuth/read`, `account/read` and
`account/rateLimits/read`, followed by an owner-locked revision
check **before** a status is published. The existing bounded two-attempt
account re-probe remains: if the account/provider changes repeatedly,
diagnostics fail closed rather than reporting earlier results.

A status read that observes a newer revision immediately changes the
visible state to `account_revalidating`, masks ChatGPT login and included
usage, clears the CLI-version and diagnostic compatibility claims, and
retains only the true process ownership and generation. Thus a notification
already parsed by the reader cannot leave old claims visible while queued.
The same invalidation runs before Connect/Disconnect; Stop and terminal
cleanup remove verified-revision authority. A completed owner still
retains any P8 cleanup-incomplete latch.

The P7 fake server now has source-only scenarios for an account change
and a model-provider authentication recovery event arriving *before*
the control thread dequeues them. Rust owner unit cases cover stale
snapshots, cancellation, generation mismatch and cleanup revocation.
Existing TypeScript source contracts assert the same wiring.
All tests are **AUTHORED / NOT EXECUTED**.

**Proof boundary:** the revision records notifications actually received
by the reader. It cannot prove that an upstream identity/provider change
which emits no notification has been observed, nor can it certify exact
instantaneous UI state after a concurrent change that has not reached the
process. The real Voice route and inference remain blocked independently;
no stored compatibility claim can currently authorize audio, TTS or a
paid provider. Installed-account, OS, latency and Voice-billing proof
remain UNKNOWN until one later integrated Windows acceptance stage.

## Phase 11: canonical Meeting/voice integration boundaries — SOURCE CANDIDATE

The current source audit found that Codex Voice was correctly blocked at
Meeting preflight, Start Translation and Original-to-TranslateIT source
switching, but the same decision appeared in three independent conditions.
More importantly, the Local outbound consumer, finalized-audio processor,
committed-turn store and native playback runtime did not independently
recheck the chosen translation engine. A queued or direct late Local job
could therefore attempt ASR, MT, TTS or playback if a preference changed
outside the normal session-locked settings path.

The existing `commands/meeting_session.rs` now owns a single
`local_translated_meeting_route_admitted` decision derived from canonical
Rust settings. Only the literal installed `local` engine admits the existing
ASR → MiLMMT → GPT-SoVITS → VB-CABLE pipeline. The same check is used by
preflight, Start, source switching and outbound-consumer initialization.
Original Microphone retains its separate raw-passthrough Start, and
incoming Meeting Sound is not recast as a Codex output lane.

The outbound consumer rechecks the gate before producing its temporary
ASR WAV. If the engine is no longer admitted, it reports
`attention_needed` and terminates its processing loop without releasing
or replacing the canonical Meeting/Stop owner. The outbound WAV processor
checks before Local ASR, MT and TTS calls and reports a bounded
`engine_not_admitted` failure without emitting voice. The existing
committed-turn store checks only the `you`/outbound lane at source and
translation commit, plus its playback reservation; incoming captions
continue to use their existing independent store path. The
`interrupted` mutation remains possible after Stop.

The one existing playback runtime checks the Local route when starting,
enqueueing and consuming a prepared WAV, including directly before native
audio delivery. Its existing generation, source epoch, monotonic
sequence and committed-source reservation remain authoritative; the
Codex gateway does not create its own turn store, worker, TTS or output
queue. The entire Codex Realtime/Codex assistant-candidate branch stays
disconnected from these production calls. The selected Codex setting
remains visible, but it cannot silently run the Local worker as a fallback.

Rust and TypeScript regression contracts now assert that these boundaries
use one Meeting-owned gate and that direct outbound/turn/playback paths
fail closed. These are authored source checks only: **NOT EXECUTED**.
The extra settings reads are only at existing utterance/worker or playback
boundaries; their target-Windows cost and correctness are not measured.
The code does not claim Codex audio entitlement, causal source/translated
text pairing, working WebRTC negotiation, physical receiver delivery,
or a completed Cloud Voice integration.

## Phase 12: native route/bootstrap parity and Original/incoming regression — SOURCE CANDIDATE

A source-level review of P11 against the actual native dispatchers found two
reachable bypasses of the intended early Local-engine decision:

1. `commands/runtime.rs::start_meeting_translation` prepared or cleared the
   selected virtual microphone route before reaching the canonical Meeting
   lifecycle's Codex rejection. A direct native Start now delegates to the
   existing Meeting blocker *before* any route-pair preparation when the
   persisted translation engine is not Local. Duplicate active-session Start
   retains the existing lifecycle owner.
2. `commands/application_runtime/intents.rs` could start/check the Local
   helper through `ensure_runtime_ready`, `check_readiness` or `fix_setup`
   when the engine was Codex. Those Meeting-only setup intents now reuse
   `meeting_session::local_translated_meeting_route_admitted`, exported to
   sibling native owners without consulting the Codex diagnostic flag.
   Rejection precedes worker status/inference and does not change the
   persisted engine or fall back to a paid provider.

**Preserved:** Explicit My Voice worker/recovery actions are separate from
implicit Meeting bootstrap. Native Original Microphone Start and source
selection keep their own non-AI prerequisites; the Codex gate applies only
to translated Start. The incoming Meeting Sound processor remains on its
existing optional, Local-enabled incoming lane; it is not promoted to an
independent Codex session, and it is not proof that a Codex Meeting can
start. One Meeting/Stop, worker, virtual output and committed-turn owner
remain unchanged.

**Proof:** STATIC_SOURCE review on pinned parent
`7fd21a9c412d2f325720898ea55800644d0dda31`; extended existing
TypeScript source assertions for direct Start routing, alternate native
intents, Original Microphone and incoming-caption ownership. The prior
Rust Meeting-engine predicate regression remains authored, not executed. All Rust/TypeScript,
build, CI, installed Codex, Windows, physical mic, VB-CABLE, realtime
Voice and paid-usage checks are **NOT EXECUTED / UNKNOWN**. A check
before source preparation is not a concurrency, CPU cost or runtime timing
proof. Codex Voice, WebRTC and TTS integration remain **NOT ADMITTED**.

## Phase 13: truthful setup diagnostics and integrated acceptance scope — SOURCE CANDIDATE

Review of P12 against the current First Setup UI found a presentation
mismatch: `productSetupFacade.runProductSetupAction("check-readiness")`
discarded the native `engine_not_admitted` explanation, and
`FirstSetup.refreshReadiness` discarded its return value altogether. The
existing facade now returns the bounded native failure message (or its
previous generic fallback when the bridge is unavailable). First Setup
reuses that message only when translation readiness remains unverified,
without inventing ready state or making new model/audio requests. No
Codex-specific UI status store or alternate admission owner was added.

The canonical [Target Windows Performance Acceptance](../operations/target-windows-performance.md)
now carries a conditional P12/P13 route matrix covering persisted
Local/Codex choice, translated Start rejection before native route
preparation, Original passthrough and source switching, optional incoming
only in Local Meeting, diagnostics-only Codex App Server, stale account
authority and cleanup failures. It reuses the existing source/compile
gate and B/C/D scenario IDs in **one** later exact-SHA integrated
Windows acceptance phase, not a second test harness. An absent Codex CLI,
account or compatible entitlement must not block the Local baseline.

**Proof:** STATIC_SOURCE on pinned parent
`4ba6e6156b7417ee4e74e863eaa8979baf396cea`; reviewed existing
frontend readiness, native intent and Meeting boundaries and authored a
TypeScript source regression. Rust, TypeScript, Svelte, CI, Windows,
installed account, real audio, WebRTC and payment terms remain
**NOT EXECUTED / UNKNOWN**. No Codex Voice, model turn, paid fallback or
cloud audio was admitted.

## Phase 14: strict persisted-engine admission and Settings mutation fencing — SOURCE CANDIDATE

The old Meeting route gate consumed `RuntimeSettings::load_or_default`,
which intentionally returns Local defaults on a missing or unreadable file.
A corrupted saved Codex choice could therefore admit the Local ASR/MT/TTS
route without valid persisted authorization.

The canonical Rust Settings owner now separates tolerant UI loading from
read-only `MeetingEngineAdmission = Local | Codex | Unverified`.
The same primary and committed backup settings are read with an actual
64-KiB file-handle bound; malformed JSON, invalid explicit engine values
and schema-v14 files missing their engine are not trusted. Pre-v14 legacy
settings still accept absent engine as Local. Only a verified absence of
primary, backup AND temporary write artifacts admits Local as a clean
first-install default. A valid backup may recover its exact Local/Codex
choice; any unresolved file/recovery state blocks Local inference.

A process-local Settings IO mutex protects canonical read/recover/save
against intermediate in-process writes. Native `save_runtime_settings`
and `select_audio_device` also reuse the *existing* canonical Meeting
Start/Stop transition lock. Device selection rereads current Settings
after this lock so a stale device preference cannot overwrite a more
recent engine selection. No new meeting/audio/credential owner exists.

Meeting preflight, translated Start, native setup intents and product
readiness now distinguish damaged Settings with `settings_unverified`
from deliberate Codex selection. The existing Settings engine control
presents a single explicit **Reconfirm Local engine** action when the
Meeting preflight reports unverified settings; confirmation uses the
existing native save and re-read path. The Original Microphone passthrough
and optional incoming within admitted Local Meeting are unchanged.

**Proof:** STATIC_SOURCE on pinned parent
`8a41115b17c724c4a5ef8fc019b7ed8c695b7626`. Rust and
TypeScript regressions are authored for clean install, old schema, Codex,
corrupt/oversized files, pending temp, valid backup, native mutation
guard and explicit UI recovery; **NOT EXECUTED**. Source does not prove
Windows file races, actual UI, compiler, GPU/audio, installed Codex,
Voice entitlement or meeting-app delivery. Complete external deletion of
all settings evidence cannot be distinguished from a first install
without a separately authorized durable provenance record. No paid
inference, Cloud Voice, WebRTC session or fallback is enabled.

## Phase 15: First Setup recovery and source-contract reconciliation — SOURCE CANDIDATE

A P14 integration review found a real recovery mismatch: when the tolerant
Settings loader cannot read a saved engine it returns fresh-install UI
defaults, including `meeting_setup_state = new`. The root desktop shell
therefore presents **First Setup**, not the Settings page containing
`Reconfirm Local engine`. Previously First Setup skipped native snapshot
inspection on its first step and could persist those tolerant defaults
through Continue/Defer, without the explicit engine confirmation required
by P14. The Settings-only recovery claim did not cover that route.

The existing First Setup now obtains an authoritative product/native Meeting
snapshot even on Step 1 before enabling its normal setup navigation. A missing
snapshot keeps setup saves disabled and offers a bounded explicit status retry;
a `meeting_session:settings_unverified` preflight result displays the
already-existing `MeetingTranslationEngine` confirmation control instead of
normal setup actions. Both the UI branch and `persistSetupFact` guard prevent
ordinary setup progress from silently overwriting corrupt saved settings.
After the user explicitly confirms Local or Codex through the existing
Settings persistence bridge, the page refreshes its canonical native
snapshot before returning to setup. No new Settings store, native command,
model request, media producer or background retry was introduced. A failed
initial read remains unknown, never an assumed clean install.

The existing `codex_engine_routing_contract.test.ts` gains a targeted
First Setup recovery regression, checking initialization order, the
persistent-save guard, native blocker visibility, shared engine component
and bounded retry control. The existing Target Windows acceptance plan
reuses the same D1/D2/D3 checks for the previously unreachable First Setup
recovery route; no parallel test harness or scenario ID is introduced.

**Proof boundary:** inspected source at pinned parent
`7d499bfafcd11c5caa976687fd5f4e6dbd6b7484`; authored source
regression only, **NOT EXECUTED**. There is no Rust/Svelte build,
rendered UI, installed Windows, device, account or billing proof. The
existing general-purpose native `save_product_settings` command still
receives a full Settings candidate without a distinct explicit-recovery
intent, so out-of-band/native callers can remain an authorization gap.
This P15 change closes the observed First Setup UI bypass, **not** that
independent native contract issue. Codex Voice, cloud audio and paid API
fallback remain unadmitted.

## Phase 16: explicit native engine recovery admission — SOURCE CANDIDATE

The P14 native `save_runtime_settings` path still accepted a full
`RuntimeSettings` candidate when `MeetingEngineAdmission::Unverified`
and no audio session was active. P15 made First Setup's user-facing flow
explicit, but a different full-settings caller could still save tolerant
Local defaults and silently repair an unreadable persisted Codex choice.

The existing Rust `commands/settings.rs` owner now rejects **every**
ordinary full-settings save while the authoritative persisted engine is
Unverified, irrespective of whether its proposed `translation_engine`
matches the displayed fallback. The Settings IO and canonical Meeting
Start/Stop transition guards remain unchanged.

A single narrowly scoped `confirm_unverified_translation_engine(engine)`
native action accepts exactly `local` or `codex`, never a full Settings
candidate or arbitrary model/provider parameter. Under the existing
Meeting transition lock, it refuses an active audio owner or a newer
already-verified setting, loads the same canonical Settings state, sets
only the explicitly chosen engine field in that candidate, uses the
existing Settings writer and confirms the resulting strict on-disk
admission matches the choice before returning success. An unknown
choice, stale confirmation, interrupted writer or uncertain post-save
read is rejected with an explicit message; no implicit retry occurs.
Because irrecoverably corrupt settings can only be displayed as
defaults by the tolerant loader, explicit recovery **may also reset
unrecoverable old preferences to defaults** when the canonical Settings
object is saved; native evidence must not claim that those older fields
were preserved.

The action is registered in the existing Tauri mutation module and
native registry, exposed via the current TypeScript runtime bridge, and
called only by the existing `MeetingTranslationEngine` component's
`needsReconfirmation` branch. The ordinary already-verified engine
change continues through `save_product_settings`. Both First Setup
and Settings already reuse this component, so no second page-local
recovery action, device writer, Meeting source or model process is
introduced. A Tauri command expresses a distinct *caller's recovery
intent* but cannot authenticate a physical user click against a
compromised renderer; it is not OS user-consent proof.

The existing Rust exact-choice regression is authored alongside
TypeScript source contracts for normal save denial, native command
registration, current admission checks, Start/Stop resource fence,
post-write verification and shared frontend caller dispatch. This is
**STATIC_SOURCE only — tests, compilation, Windows execution, Codex
account, Voice/audio entitlement, WebRTC, metering, microphone and
receiver outcomes remain NOT EXECUTED / UNKNOWN**. No cloud inference,
paid-key fallback, AI worker or new output ownership was activated.

## Phase 17: atomic recovery fidelity and stale UI engine isolation — SOURCE CANDIDATE

Source audit of P16 identified two first-wrong-owner defects. First, the
explicit recovery path called tolerant `load_settings`, which discarded
otherwise well-formed microphone, incoming-sound, setup and suppression
preferences whenever a saved JSON document had only an invalid
`translation_engine`. The separate admission read, recovery read,
write and post-write verification also released the Settings IO lock
between each operation.

The existing `engine/settings.rs` owner now provides one
`RuntimeSettings::recover_unverified_engine` operation. It holds the
existing Settings IO mutex for the full strict-Unverified check, bounded
recovery read, canonical atomic save and strict primary-file receipt.
It retains type-valid non-engine preferences by replacing only the engine
field **in memory** before deserializing through the existing Settings
schema/sanitizer, and falls back to a recoverable `.bak` object if the
primary is not parseable. When neither contains a usable Settings object,
the explicit user choice still permits recovery with documented safe
defaults; unrecoverable data cannot be claimed restored. On an
already-verified engine it refuses stale recovery without modifying
the file. A correct-looking prior backup is not destroyed with a
truncated/unparseable primary at the beginning of `write_atomic`; this
preserves a recoverable crash-window source, not a guarantee that
external process writes or Windows filesystem errors are impossible.

Second, the general `save_runtime_settings` path still accepted a
stale full Settings DTO carrying an old Local/Codex preference whenever
the persisted engine was otherwise valid. That could silently reset
the engine while another preference such as noise suppression was
being edited. It now rejects **all engine changes** from broad Settings
saves. An explicit `select_translation_engine(engine,
expected_current_engine)` native action checks literal supported
values, no active Meeting/audio owner, the existing canonical
Start/Stop transition mutex, strict persisted admission matching the
caller's observed previous engine, then reloads *current* Settings
and writes only the chosen engine field through the existing writer.
Stale comparisons or post-write admission mismatches block rather
than silently changing engines. The existing shared Svelte
`MeetingTranslationEngine` component calls this action for normal
engine selection; its P16 narrow recovery route remains reserved for
Unverified Settings. TypeScript and Tauri use one native mutation owner
and no new runtime output, TTS, model or network owner.

**Proof ceiling:** STATIC_SOURCE on pinned parent
`b3d6e97cd91d812e87923bd109b1b864cca60745`.
Rust unit regressions were authored for valid non-engine preference
retention, recoverable backup fallback, refusal of later stale recovery
and the backup replacement decision. TypeScript source regressions
were updated for the new canonical compare-and-set mutation and
registry/bridge contracts. All Rust/TypeScript/Svelte builds, tests,
rendered UI, Windows filesystem concurrency, hardware and Codex
account/Voice/billing remain **NOT EXECUTED / UNKNOWN**. Full-settings
saves of other stale *non-engine* fields remain an independent
general Settings concern; this phase specifically prevents silent
engine mutation. No experimental Codex Voice, paid fallback or cloud
inference is admitted.

## Phase 18: integrated source-readiness audit and Settings admission repair — SOURCE CANDIDATE

The P17 narrow engine recovery correctly retained typed, recoverable
non-engine preferences, but First Setup's `confirmEngineSelection`
only refreshed its own snapshot. When the native recovery restored
`meeting_setup_state = completed` or `deferred`, the outer desktop
application continued presenting a second wizard; following its
normal Continue/Defer would risk writing `new` back over a previously
finished setup. The same First Setup owner now verifies the refreshed
native preflight, hands the restored settings back through the existing
`onComplete`/application-controller owner for completed or deferred
sessions, and restores the persisted checkpoint through the existing
safe-resume rules when setup was genuinely still new. No synthetic
setup success or second persistence call is introduced.

The native general-purpose `save_runtime_settings` path had another
source-proven boundary gap: when stopped, a full stale DTO could write
`audio.input_device_id` or `output_device_id` without the existing
probe-and-select owner; when Meeting or Mic Test was active, it could
also write new language directions, setup state/checkpoint or noise
suppression as long as device and engine stayed unchanged. That
created a potential mismatch between a live audio generation and
saved preferences. The existing Settings command now rejects
**all full-DTO physical audio endpoint changes** in favor of
`select_product_audio_device`, even when idle. It also rejects
changes to Meeting-sensitive language, setup and capture suppression
fields while the existing runtime/audio owner or Retry Stop retains
resources. Explicit engine compare-and-set and unverified-engine
confirmation stay as P17/P16, using the same Meeting transition and
Settings IO locks. Unrelated normal idle setup and suppression saves
remain admitted; no extra background worker or state database was
created.

A narrower backup preflight defect remained from P17:
`replace_previous_settings_backup` treated any parseable primary JSON
as suitable to overwrite a parseable backup. A primary containing
invalid `translation_engine` could therefore displace a recoverable
backup before an interrupted rename. The existing policy now requires
a **strictly valid primary Settings record** before replacing a
parseable prior backup. An absent/unparseable backup still permits the
usual previous-file copy. The existing Rust backup-policy unit case
now includes a parseable but invalid-engine primary.

**Source proof:** inspected pinned parent
`cf533d72482623b76c44fba948ad4134967304cb`; authored Rust unit
regression for Meeting-sensitive fields and expanded the original
backup-policy case; added focused TypeScript source assertions for
First Setup resumption, native endpoint/Live Settings admission and
strict backup protection. All tests, compilers, Svelte rendering,
actual Windows device/race conditions, Codex account/Voice billing,
and receiver end-to-end behavior are **NOT EXECUTED / UNKNOWN**.
An idle stale full DTO can still modify unrelated non-engine
preferences; it cannot silently rebind a microphone or change the
selected engine through the general save. Real in-process timing
and external filesystem interference remain target-host evidence
requirements, not a source PASS. Codex Voice, paid inference and
automatic fallback remain **NOT ADMITTED**.

## Phase 19: stale full-Settings snapshot admission — SOURCE CANDIDATE

The P18 broad Settings command already refused implicit engine changes,
unprobed device rebinds and Meeting-sensitive edits while audio was
owned. Its remaining idle boundary accepted a full `RuntimeSettings`
candidate without verifying the UI's base settings version. When
First Setup, Meeting's `Finish Setup` action or noise suppression
saved an old snapshot after another native Settings mutation,
the full DTO could silently overwrite a newer non-engine preference.

The same native `save_runtime_settings` owner now requires an
`expected_settings` snapshot and compares its sanitized canonical
fields with the currently loaded Settings **under the already existing
Meeting lifecycle mutation guard**. A mismatch refuses the entire
write before the current unverified-engine, device/engine and
live-resource restrictions are relaxed; the earlier restrictions
remain in place. Existing `AudioSettings` and `RuntimeSettings`
derive structural equality, not a new persisted version field,
revision cache or settings database. A new install with matching
defaults remains Local by existing policy.

The existing `save_product_settings` Tauri mutation and
`runtimeApi.saveSettings(settings, expectedSettings)` forward the
expected base snapshot. Exactly three source callers in the current
frontend tree—First Setup checkpoint persistence, Meeting's
`Finish Setup`, and noise suppression—supply their prior Settings
snapshot; explicit Local/Codex engine changes continue to use
the P16/P17 narrow native commands. The existing runtime-settings
contract test receives source assertions for the compare-before-write,
Tauri/TypeScript argument parity and all three callers. No new
settings owner, model invocation, audio thread, scheduled task or
retry mechanism was introduced.

**Proof:** STATIC_SOURCE review of 71 frontend TypeScript/Svelte
files and the three actual saveSettings callers at pinned parent
`89aa24bfa70a8ca4497deadec184088478093d03`.
One regression is authored but **NOT EXECUTED**; Rust compile,
TypeScript typecheck, Svelte check, live Windows filesystem races,
audio/meeting receiver and Codex diagnostic compatibility remain
UNKNOWN. External writers that bypass the in-process lock and changes
arriving after a successful persistence receipt are not guaranteed
by this source-only optimistic-concurrency contract.
Codex Voice/cloud inference and paid fallback are still not admitted.

## Codex text-only source candidate — 2026-11-06 (OFFLINE / NOT ADMITTED)

At upstream openai/codex `0327a3ec34fd8fe9454eb57151a3798f127a53bf`,
App Server v2 exposes ephemeral `thread/start` and `turn/start` text requests
and optional JSON output schemas. The existing gateway now owns one
`text.rs` **source-only** adapter: one independent bounded finalized
utterance, strict ID↔EN direction, structured single-field output, exact
thread/turn/generation/epoch/sequence binding, one-shot final acceptance
and rejection of ambiguous, incomplete, oversized or tool-bearing results.
The gate additionally binds the exact serialized one-utterance request to
the completed turn's single text user-message item: changed direction or
source, missing/duplicate source and answer-before-source are rejected.
Matching this source is structural evidence, not proof of semantic fidelity.
The pinned Codex App Server implementation (openai/codex
`0327a3ec34fd8fe9454eb57151a3798f127a53bf`,
`codex-rs/app-server/src/bespoke_event_handling.rs`) actually sends
`turn/completed` with `itemsView=summary` and only its last assistant
message, or `notLoaded` with no items. Its v2 `turn_start.rs` test
asserts `Summary`. Neither includes the source turn or a complete
tool/action audit. Our offline adapter now rejects `summary` explicitly
as `codex_text:completion_summary_unverified` and keeps `full` as a
synthetic inspection-only boundary; it does **not** turn normal
`turn/completed` into a usable translation provider.
Building a live evidence collector would require complete item provenance
and a pre-execution no-tools guarantee, neither admitted yet. No
`thread/read` or `thread/items/list` backfill is added: it would
create another data, privacy and latency boundary without solving tool
safety.
It performs no model requests, is not registered in the account-only JSONL
allowlist and cannot produce a Meeting translation commit or TTS output.
The offline request builder now checks its **fully serialized nested JSON**
against the existing 8-KiB JSONL budget, reserving 32 bytes for a future u64
RPC ID and newline. A 4-KiB source that expands after escaping fails
explicitly instead of being truncated. Final response evidence requires the
literal upstream `turn/completed` event; a premature or unrelated event
consumes the one-shot evidence gate. Neither check enables inference or proves
Codex is a tool-free translator.

**Safety gate:** Codex is a coding agent, not a proven tool-free translator.
`sandbox=read-only` and `approvalPolicy=never` do not prohibit all
tool invocations or prevent their side effects. Post-hoc rejection of
`commandExecution`/MCP/file/web items is only forensic evidence; no
live tool-free entitlement, text translation correctness, included usage,
cloud privacy boundary, GPU savings or latency has been verified.
Do not connect these source-only shapes to App Server, Meeting, or the
existing Python worker until a supported pre-execution no-tool gate and
usage/cost are established. Preserve the Local pipeline and current
selection blocks; no silent fallback.

**Efficiency and quality acceptance:** one completed utterance, one model
turn with no prior-turn context or correction-chain, no interim TTS, the
existing 28-case fast and full translation-quality corpora, and later
paired stage/first-playback P50/P95 plus device VRAM observations. No
runtime improvement is claimed. Rust unit/TS source contracts are
AUTHORED, NOT EXECUTED; CI, cloud inference and Windows tests remain
disabled/deferred pending explicit owner authorization.

## Translation commit and resource ownership

One future internal result contract should carry session ID, generation,
source epoch, monotonically increasing utterance sequence, provider item ID,
source language, target language, final translated text and proof of final
completion. Source transcript and translated assistant response must be
distinguished by role and correlated, not inferred from free text.

- Draft transcriptions/captions are never committed to GPT-SoVITS.
- A final result is admitted at most once for its provider item ID and
  generation; stale or out-of-order results never get playback authority.
- The same existing committed-turns and serialized playback owner remains
  authoritative. The Codex gateway does not write temporary WAVs or operate
  VB-CABLE itself.
- Output authority is revoked *before* cancellation and owned teardown at
  Stop. Incomplete cleanup remains an explicit blocking state.
- Route switching occurs outside Live, after one verified Stop. Do not
  silently switch to local inference when a Codex live session fails.

## Latency and quality acceptance

Measure source speech onset/end, last submitted audio frame, first/final
translated text, TTS completion, and first CPAL audio playback. Record
P50/P95, jitter/queue depths and intentional gaps for the same source SHA,
account entitlement, voice actor, sentences and Windows device. Target initial
P50 offset-to-first-audio <=2 s and P95 <=4 s, *not a promise or upstream
performance number*. Compare the current Local baseline and reject
optimizations that break complete numbers, negation, names, corrections,
speaker intent, transcript ordering, or final delivery.

Use existing translation quality corpus and Meeting source contracts. Phase-1
source tests are bounded protocol/ownership examples, not network, account,
compile or physical Meeting acceptance. Later live acceptance requires one
authorized integrated Windows test phase and private receipts, not new
incremental user-PC or hosted CI requirements.

## Source owners and continuation

- Native gateway, account admission and process lifecycle:
  `src-tauri/src/commands/codex_gateway.rs` and its children, with a
  one-flight unverified output boundary in `codex_gateway/correlation.rs`.
- User-facing *diagnostic only*: `src/components/settings/CodexGatewayDiagnostics.svelte`
  and `src/app/bridge/codexGatewayApi.ts`.
- Shared Meeting Start/Stop, voice readiness, output and archive: their
  **existing** owners in `docs/knowledge/source-ownership.md`.
- Source contract: `scripts/tests/codex_gateway_contract.test.ts`.
- Product engine selection persists through the existing Settings owner,
  while Codex translation Start and source switching are explicitly blocked
  until admission. Streaming audio and release bundling are NOT enabled.

**Next acceptance gate (deferred, not an execution request):**
preserve this P19 source candidate at the exact Local SHA after commit
and use the existing Target Windows Performance Acceptance sequence
for locked source-contract tests, Rust/frontend compilation, then
one coordinated developer-mode Local/Original/optional-incoming and
Codex diagnostics-only acceptance. Native Windows/audio/install proof
remains UNKNOWN until that separately authorized integrated phase runs.
Do not dispatch paused CI, request incremental PC tests, activate
Codex Voice or use paid inference.
