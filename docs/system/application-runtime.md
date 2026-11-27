---
id: document.system.application-runtime
class: DOCUMENT
domain: system
role: ARCHITECTURE
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT Application Runtime

This document owns current **ApplicationRuntime and frontend coordination architecture**. The development execution workflow stays in [Development Flow](../knowledge/flow.md), product semantics in [Foundation](../foundation/README.md), and exact implementation paths in [Source Ownership](../knowledge/source-ownership.md).

## Product runtime authority

Interactive product flows now converge through one application-level authority instead of letting individual UI pages own cross-feature lifecycle decisions.

```text
Svelte UI
→ Product intent
→ ApplicationRuntime
→ subsystem command / resource owner
→ canonical ApplicationSnapshot
→ runtime event
→ UI reconciliation
```

Current migrated vertical slices:

- Meeting start / stop
- Mic Test start / stop
- Setup recovery / helper readiness
- Shared audio device selection
- My Voice recording start / stop
- My Voice build start / cancel / approve
- Built-in Meeting voice selection

Rules:

1. UI may present convenience guards, but the backend runtime is authoritative for resource ownership and lifecycle acceptance.
2. Cross-feature actions use `dispatch_product_intent`; feature-local read APIs may remain direct until their migration is justified.
3. `ApplicationSnapshot` is the canonical cross-feature reconciliation payload. Feature-specific snapshots remain valid internal detail, not competing product truth.
4. Runtime events signal state changes. Polling remains a reconciliation/watchdog mechanism, not the preferred owner of lifecycle transitions.
5. Superseded direct mutation commands are removed from the public Tauri surface after migration. Feature-local read APIs remain direct only when they are still useful and do not create a competing lifecycle authority.
6. New cross-feature conflicts must be solved in `ApplicationRuntime` or the underlying resource owner, never by adding another page-specific boolean maze.


### Intentional frontend-owned module: translation overlay

The floating translation overlay remains frontend-owned. It manages window presentation, local preferences, caption rendering, and position state; these are UI concerns rather than shared resource ownership. Do not move overlay window implementation into ApplicationRuntime. Only promote an overlay concern into the application kernel if it becomes a real cross-feature lifecycle/resource invariant.


### Snapshot cost rule

`ApplicationSnapshot.revision` is a monotonic snapshot-build sequence, not a semantic state-change revision. Use owner/session/generation/domain fields to detect meaningful state transitions.

`ApplicationSnapshot` is a cheap reconciliation contract. Building it may inspect local in-process/native status, but it must not run expensive worker requests, model inference, functional probes, or recovery actions. Cheap capabilities therefore cover only what current local state can prove; directional translation capability remains an internal model fact inspected by the product readiness layer; Meeting preflight determines user-facing readiness. Deep AI readiness belongs to the product/detail query that explicitly needs it. This keeps close checks, settings mutations, and runtime events bounded and prevents orchestration from creating hidden latency.


### Single-observation Settings and preflight

Each application snapshot uses one fresh locked Settings observation containing
both tolerant display preferences and the strict engine-admission decision.
The latter must distinguish a clean missing installation from damaged or
unverified persisted configuration; UI defaults alone never authorize AI.
Input inspection and helper status are observed once and reused by uncached
Meeting preflight. Live generation-bound Start receipts retain their existing
behavior. Full Settings save and guarded device selection also reuse their
single current observation. No cross-operation cache, schema change or
authorization lifetime change is introduced. Explicit recovery/compare-and-set
writes, fresh post-write verification, and strict per-stage route admission
remain intact; a separate session authority design is required before reducing
those disk reads. Existing Settings backup recovery/cleanup remains in the
canonical Settings owner.

### Capture CPU preparation

The existing Silero lane CPU owner reuses one native window scratch buffer,
sized to the maximum rational 512-sample/16 kHz window at its capture rate.
The native sample order, interpolation phase, lookahead, model, threshold,
speech evidence and finalized WAV ownership are unchanged. Integer Meeting
Sound callbacks also reuse a callback-local conversion/downmix buffer just as
the existing float callback does. The scratch buffer may grow for a larger
callback, never for an unchanged size after allocation. The VAD gate moves
its evidence rather than cloning it. No additional audio owner, model,
queue, polling cadence, dependency or transport is introduced.

### Meeting Start observation boundaries

The initial Start preflight returns its checked virtual-route observation to
the existing lifecycle transaction. That same endpoint is used for the
generation claim and native output preparation; a second immediate route
enumeration is not needed. The later post-AI and final pre-Live preflight,
generation binding, microphone and native output checks remain fresh.
A preflight's cleanup summary and blockers share one cleanup observation;
the Start transaction's separate cleanup gate is unchanged. Functional
readiness status reads borrow the existing proof and copy only the requested
timestamp or actor token, not the entire receipt.

Cross-generation functional-proof reuse remains conditional under
[Phase C](../foundation/04-realtime-latency-architecture.md#phase-c--strong-functional-proof-rebinding).
The existing receipt has worker generation, Meeting generation, actor token,
and verification timestamp, but does not establish the required full
Qwen3-ASR/Hy-MT2/voice execution identity and invalidation epoch. There is no
matched target timing proving repeated inference dominates warm Start.
Do not replace the full functional probe with cached readiness or file
presence on this source evidence alone.

### Worker response ownership

The newline-JSON worker transport parses and validates each received response
at its existing request/correlation boundary. The Rust receipt now retains
that owned JSON value through scheduler timing annotation, transport recovery,
functional proof, and Meeting ASR/translation/voice admission. Consumers
borrow fields instead of serializing and reparsing the entire response.
The existing Tauri diagnostic `worker_response_json` field is still a
JSON string, serialized only at that external boundary. No new protocol,
scheduler, cache or worker is introduced. Permit limits, priority, deadlines,
retry admission, generation checks, failure invalidation and incoming
degradation are unchanged.

### Event-first Meeting reconciliation

Live Meeting UI freshness uses lightweight backend change events from authoritative committed-turn state. The event payload contains only revision/reason/session/sequence metadata. Both event and 10-second fallback use the existing live transcript command with an opt-in reconciliation request: one IPC returns Meeting status and either a full bounded live snapshot or an explicit unchanged receipt. The store-owned revision covers accepted source, translation, delivery, timing, and interruption mutations; the token also binds the session and observed archive errors. Unknown archive health or a new session never authorizes unchanged reuse. No timestamp heuristic, delta log, extra command, or cross-session transcript cache. The bridge passes the explicit nullable reconciliation argument; the shared saved/export DTO remains unchanged, with live-only metadata kept in the conditional read type. Standalone full transcript reads and saved/export formats remain available. The superseded standalone Meeting-status IPC and its unused frontend wrapper are retired; internal Rust Meeting status observations remain available to runtime and diagnostics.

Reliability monitoring remains polling-based where the condition itself is time-dependent (watchdog, device-loss detection, long-session pressure). Watchdog + device-loss share one live reliability IPC every 8 seconds and reuse one fresh Meeting status observation. The device-loss guard and virtual-route owner also share one current audio-device enumeration; standalone diagnostics still take their own fresh observations. No cross-poll cache or cadence change is introduced; long-session health remains on its separate 30-second cadence. UI-only advisory checks such as meeting-app detection and idle audio-quality status are on-demand/page-entry reads rather than permanent polling. Explicit support export reuses one Meeting observation, one helper observation and one bounded incident-history read through the existing watchdog, device and session-health owners; standalone diagnostics and privacy-whitelisted export remain unchanged. This is not a persistent cache or an atomic snapshot of every subsystem.


### Bounded audio preparation

The existing two-second diagnostic PCM window evicts excess samples before
appending, keeping its allocated capacity stable for a fixed audio format.
Oversized incoming batches retain only the newest window while counting every
received sample. Optional finalized-WAV noise attenuation is applied directly
during the existing PCM16 encoding pass, preserving the gain formula,
quantization, single buffered write, exclusive staging and atomic promotion.
No extra filtered audio vector, callback owner, transport or model is added.

Finalized Meeting PCM now moves from the producer to its existing outbound
or incoming consumer and is borrowed at the existing helper scheduler's
admitted ASR boundary. The bounded deferred incoming queue owns the original
PCM allocation, not a file path. Legacy ASR still requires PCM16 WAV; its
transitional file is created only after admission and authority checks and
released at the end of the ASR attempt, not after MT/TTS. A late outbound
claim during file preparation yields before worker I/O. Existing scoped Stop
file cleanup remains authoritative. Audio-preparation timing is separated
from ASR attempt timing. The native CUDA candidate is not yet dispatched
here; actual Windows execution, memory use and latency are unverified.

### Demand-scoped playback, archive and auxiliary work

The native playback callback directly owns the prepared device PCM. Decoded
source PCM is released before stream creation; the callback processes the
available bounded prefix and silent tail while preserving first-audio timing,
generation/Mute/source and failure checks. The current one-pending-slot queue
and per-utterance stream lifecycle remain unchanged.

The archive scanner retains one filename/type validation pass. Count-only
finalization, History and warning reads do not collect or sort turn paths;
Preview and Export opt into the ordered inventory. Atomic archive receipts,
actual disk observations and scoped Stop/Retry Stop ownership remain intact.

Explicit Reliability Diagnostics uses the existing combined watchdog and
device-loss observation; recovery, incident and long-session reads remain
independent. Quick Access retains event subscription and immediate refresh,
but schedules periodic recovery only while focused and releases its timer
on blur/disposal. No new controller, storage index or background manager.

### Frontend application controller

The frontend shell consumes one `ProductRuntimeSnapshot` through `src/app/runtime/applicationController.ts`. The controller owns refresh sequencing, setup-state transitions, settings projection, and Meeting-session projection. `App.svelte` owns only UI-shell concerns such as route, transient notices, busy flags, close-dialog state, and the live transcript view.

Do not reintroduce parallel `$state` copies for helper, worker, input, settings, approved voice readiness, or Meeting runtime state in `App.svelte`. Derived views must come from the controller snapshot.


### Shell controller split

`App.svelte` is intentionally a composition shell. Runtime-heavy view coordination is split into focused frontend controllers:

- `applicationController.ts` — product snapshot, setup transition, refresh sequencing.
- `meetingLiveController.ts` — transcript reconciliation, overlay revision, Meeting live-view freshness.
- `closeController.ts` — native-close dialog/check/stop flow.

These controllers may coordinate existing public runtime/query APIs, but they must not absorb domain algorithms. The shell keeps route, notices, and transient button busy state only.

### Product facade split

`runtimeProductFacade.ts` is a compatibility/public composition surface, not a domain implementation owner. Product audio and setup/recovery logic live in `productAudioFacade.ts` and `productSetupFacade.ts` respectively. Direct text translation belongs only to internal worker/developer verification, with no standalone native application command. New unrelated product behavior must not be appended to the root facade by default.


### Product-state policy split

Frontend product state policy is split by concern:

- `productMeetingState.ts` — Meeting lifecycle/status/preflight mapping.
- `productReadinessState.ts` — cross-capability readiness, blockers, voice gate, and product status copy.
- `workerCapabilities.ts` — parsing worker capability evidence only.
- `runtimeProductState.ts` — compatibility re-export surface only.

Do not merge these implementations back into one large product-state file. Compatibility surfaces may re-export stable APIs, but policy ownership stays domain-scoped.
