---
id: document.foundation.realtime-latency-architecture
class: DOCUMENT
domain: foundation
role: ARCHITECTURE
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT — Realtime Latency Architecture

**Status:** Approved target architecture; implementation remains evidence-gated  
**Updated:** 2026-11-28  
**Scope:** required outbound Meeting latency without reducing translation, ASR, voice, ordering, or safety quality

## Goal

Reduce perceived end-of-speech → first translated playback latency and prevent latency from accumulating during normal consecutive speech, while preserving the current canonical models and Meeting reliability contracts.

This file defines the target architecture. It does not claim the phases below are already implemented or TARGET_WINDOWS-proven.

## Non-negotiable invariants

- One Tauri/Rust Meeting lifecycle owner and one canonical native CUDA inference owner remain authoritative.
- Finalized utterances remain the only normal outbound AI input.
- Preserve the pinned Qwen3-ASR, Hy-MT2 and ZONOS3-Mini model identities, current input/generation policy, VAD quality, selected reference and speech quality for latency.
- English TTS playback remains serialized, ordered, bounded, and at-most-once.
- Capture may continue while previous output is processing or playing.
- No unbounded queue, parallel TTS engines, second worker, speculative cloud fallback, or user-facing Realtime/Quality mode.
- Stale generation/session/utterance work never becomes current output.
- Stop revokes output authority before cleanup; queued old audio is never replayed after Stop/restart/wake.
- Failure is explicit. Backpressure or output failure must not silently drop, reorder, duplicate, or replay speech.

## Stable target flow

```text
physical microphone
→ finalized utterance
→ bounded finalized queue
→ ASR
→ ID→EN translation
→ selected voice TTS preparation
→ bounded playback handoff
→ one serialized Rust playback owner
→ TranslateIT Meeting Microphone
```

The key separation is **AI preparation vs playback ownership**. Native inference remains serialized by the canonical helper scheduler; the existing Rust playback owner can deliver the previous complete English PCM while the next finalized utterance uses the same native inference owner.

## Phase A — bounded playback decoupling

This is the preferred first architecture change when continuous-speech `queue_ms` grows because the outbound consumer waits for previous playback to finish.

The current source uses complete native PCM, one dedicated Meeting playback consumer and a bounded `sync_channel(1)` handoff. Target behavior remains unproven.

Bound:

```text
currently playing = max 1
pending playback  = max 1
```

The AI/outbound consumer may therefore prepare at most **one utterance ahead**. It must not generate an unbounded audio backlog.

A playback job carries generation, session, event sequence, utterance identity, owned PCM and the existing selected-actor/output admission, and timing context. Playback accepts jobs only for the authoritative generation and in event-sequence order.

If the one pending slot is occupied, AI preparation experiences bounded backpressure until the playback consumer advances. If the generation becomes stale or the bounded wait cannot complete safely, the turn becomes an explicit interrupted/attention state; it is never silently emitted late.

Ownership changes required by this separation:

- the playback consumer owns `deliver_meeting_output_native_pcm_for_source` execution;
- self-output suppression covers **actual playback**, not time spent waiting in the queue;
- first-playback and delivery completion timing are reported by the playback owner;
- the playback owner consumes or drops the owned PCM after success/failure/cancellation;
- Stop cancels current playback, closes the producer, rejects pending jobs, drops their owned audio, and joins the playback consumer.

No audio synthesis or resampling math changes in Phase A.

## Phase B — persistent Meeting output stream

Introduce only if TARGET_WINDOWS shows repeated native stream/device setup remains a meaningful delivery cost or source of jitter after Phase A.

The Rust Meeting output owner then opens the selected endpoint once during `Starting`, proves a real silent callback, and retains that exact stream through the authoritative Meeting generation.

```text
Starting
→ resolve exact route
→ validate output config
→ build/start CPAL stream
→ observe silent callback
→ keep stream alive
→ commit Live
```

The same stream that proves native callback readiness becomes the playback stream; do not open a second probe-only stream and then reopen per utterance.

When idle, the callback emits silence. Callback work is restricted to bounded sample copy, generation/cancel atomics, first-playback signaling, and non-blocking buffer-boundary handoff. No file I/O, device enumeration, Python/JSON work, allocation-heavy conversion, or ordinary logging belongs in the callback.

A native callback error marks the output runtime failed for that generation. Do not automatically rebuild/replay uncertain audio mid-session. Subsequent delivery fails closed until explicit recovery through the normal lifecycle.

## Phase C — strong functional-proof rebinding

Introduce only if warm Start → Live is materially dominated by repeating the full ASR→translation→voice→ASR functional fixture on an otherwise unchanged healthy worker.

A reusable AI proof must be bound to a strong identity, not elapsed time or file presence. Minimum identity:

```text
helper worker generation token
+ readiness/invalidation epoch
+ ASR model id + executed device/compute type
+ Hy-MT2 model id + pinned revision + executed device/precision
+ selected voice engine/revision
+ exact actor token/fingerprint
```

Full functional proof remains required when no matching proof exists. A later Meeting generation may rebind the proof only after a cheap worker status confirms the exact same identity and no invalidation event occurred.

Invalidate the proof on at least:

- helper exit/restart/transport replacement;
- hard required outbound ASR/translation/TTS failure;
- actor selection/promotion/rebuild/change;
- model/runtime device or precision identity change;
- explicit runtime repair/restart;
- power lifecycle invalidation where runtime continuity is uncertain.

Normal Stop does not need to invalidate a healthy proof by itself; a Stop that hard-cancels/restarts the worker already changes worker generation.

Microphone and native output readiness are still proven for **every new Meeting Start**. AI proof rebinding must never turn PR-053 into file-presence readiness.

## Phase D — quality-preserving TTS fragments

This is last, not first. Enable only when TARGET_WINDOWS shows `tts_ms` remains the material first-audio bottleneck after safer overlap/output work.

The pinned ZONOS3 helper has a streaming path, but early streaming can decode incomplete codebooks and cannot retract audible output if a later frame fails. Current native synthesis explicitly uses non-streaming complete PCM. Keep Phase D held: enabling any progressive output requires explicit product approval of changed partial-failure semantics and target quality/ordering/cancellation evidence, not just lower first-audio latency.

No forced sentence splitting, model/sampling changes, interleaved utterances, duplicate playback, alternate engine or user-facing quality toggle. Preserve the complete-output baseline until the new contract is approved and demonstrated.

## Metrics

Retain current stage metrics and distinguish preparation from playback pressure:

```text
speech_boundary_ms
finalization_ms
queue_ms                finalized utterance → AI owner
asr_ms
translation_ms
tts_ms
playback_queue_ms       TTS ready → first playback ownership
output_prepare_ms       Rust decode/resample/preparation when applicable
delivery_ms             playback execution → first audible frame
outbound_latency_ms     finalized utterance → first translated playback
```

Also expose bounded counters/state for playback queue depth, playback backpressure, interrupted queued output, callback errors, and persistent-stream failures when those phases exist. Diagnostics remain redacted and must not log conversation text/audio.

## Rollout order

Do not implement all phases together.

```text
TARGET_WINDOWS baseline
→ measured continuous-speech queue growth?  → Phase A
→ native delivery/open jitter still material? → Phase B
→ warm Start dominated by full AI proof?     → Phase C
→ TTS still dominates first-audio latency?   → Phase D
```

Each phase must preserve the previous accepted behavior and have its own rollback boundary. A later optimization is not justification to weaken an earlier safety invariant.

## Source acceptance

REMOTE_GITHUB/source proof may establish bounded queue logic, order, generation rejection, cleanup ownership, authored compile/lint/unit contracts (not executed results), identity matching, and failure-state behavior.

TARGET_WINDOWS remains required for actual microphone/device behavior, playback callback stability, meeting-app reception, real Start → Live, end-to-end latency, resource pressure, speaker fidelity, and repeated-session claims.

The architecture is successful only when measured latency improves **without** quality reduction, stale output, duplicate playback, unbounded backlog, or weaker readiness truth.
