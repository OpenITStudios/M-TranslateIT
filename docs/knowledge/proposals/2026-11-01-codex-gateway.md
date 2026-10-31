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

## Phase 2: capability and audio admission — NOT IMPLEMENTED

Use the *installed and version-pinned* Codex CLI's actual protocol schema;
never synthesize unsupported JSON-RPC or treat a source type as entitlement.
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
  `src-tauri/src/commands/codex_gateway.rs` and its children.
- User-facing *diagnostic only*: `src/components/settings/CodexGatewayDiagnostics.svelte`
  and `src/app/bridge/codexGatewayApi.ts`.
- Shared Meeting Start/Stop, voice readiness, output and archive: their
  **existing** owners in `docs/knowledge/source-ownership.md`.
- Source contract: `scripts/tests/codex_gateway_contract.test.ts`.
- Normal product engine selection, audio streaming and release bundling:
  explicitly **not** enabled by the phase-1 connection probe.

**Next work:** prove and implement the smallest authorized ChatGPT-account
Voice-to-final-translated-text adapter with a source-derived event contract,
then admit the Codex route through the existing Meeting preflight/Stop
lifecycle. If included Voice access is not demonstrably available, keep
Jalur 2 blocked without weakening Local Meeting.
