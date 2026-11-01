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

Only `initialize`, `account/read` and `account/rateLimits/read` are
admitted at this phase, with `initialized` sent after initialize. Account
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
- Normal product engine selection, audio streaming and release bundling:
  explicitly **not** enabled by the phase-1 connection probe.

**Next work:** verify the installed Codex Voice protocol and included-usage
entitlement, then obtain an authoritative or experimentally demonstrated
source-to-assistant mapping that remains correct across long sessions,
interruptions and overlapping speech. Only after that evidence may the
existing Meeting Start/Stop and GPT-SoVITS owners admit the Codex route.
Unverified or separately billed Voice remains BLOCKED.
