---
id: document.knowledge.current-validation
class: DOCUMENT
domain: knowledge
role: CONTRACT
authority: CANONICAL
lifecycle: ACTIVE
---

# Current Validation

This file owns **proof interpretation**, not a run diary. Opt-in `tools/repository_verification.py` receipts may prove only the selected commands actually completed on the exact clean `Local` SHA. `SELECTED_CHECKS_PASS_PARTIAL_PROOF` leaves unknown dependency closure and other tests unverified; it is never release/native acceptance. `planning/development.md` owns continuation; GitHub remains authoritative for exact run/job metadata.

## Source Authority

Repository: `OpenITStudios/M-TranslateIT` (same repository ID; earlier owner URLs are historical).

**Local-only development/source authority:** `Local` owns active development, governance, source proof, continuation, and release-source validation. The GitHub default branch `main` is reserved for accepted final releases; do not use its status or older workflows as current `Local` proof.

one SHA does not prove another SHA. Later documentation-only commits do not upgrade runtime/source proof.

## Current Source Proof

### My Voice audio playback ownership and async handoff — 2026-10-30

**Pinned source parent:** `740f1bc0202601d770c60711e70519b42a569edc` on `Local`. My Voice has one parent Audio element for Built-in, Saved and guided Replay, and an independently mounted `MyVoiceBuild.svelte` Audio element for Quick Preview and eight held-out evaluation samples. The parent previously called only its own `stopReplay()` before microphone capture or pane changes, leaving child playback active. Parent Built-in/Saved asynchronous WAV loads also lacked the existing request-revision check used by guided Replay, so a late response could start playback after mic ownership changed.

**Bounded source repair:** The existing child exports a synchronous `stopPlayback()` (Svelte 5 component export), still owning its Audio element. The page binds that export and calls both player Stops immediately before starting microphone capture, stage transitions, voice switching, archive-backed improvement and unknown recording state; hidden/locked child playback is also invalidated. Both audio owners request focus before loading, and the page's Saved/Built-in WAV fetches and player callbacks now compare `replayRevision` before using bytes or clearing active playback. The existing `playEvaluation` and `playQuickPreview` guards prevent overlapping child requests. The source-only frontend regression owner asserts these invariants. No new persistent audio manager, Tauri command, model, WAV writer, device route or approval path.

**Proof ceiling:** `STATIC_SOURCE` only, including Svelte 5 component export semantics and authored source contracts. No Svelte typecheck/rendering, Rust/Node/Python execution, audio fixture playback, microphone/device, training/GPU or installed Windows acceptance was run. Actual playback isolation and keyboard interaction remain **UNKNOWN / NOT EXECUTED**.

### My Voice integration admission: bounded status file reads — 2026-10-30

**Pinned source parent:** `c84cefa5628f5a1dd6a35958144855c85b6c771e` on `Local`. The current `evaluation_manifest` loaded `evaluation.json` with `fs::read` *before* enforcing its 128-KiB bound; the main `reviewable_evaluation` read candidate `actor.json` with no size bound. Even an ordinary My Voice status refresh could therefore attempt excessive allocation on corrupted/oversized local files. P3 reference and frozen dataset checks also used size-before-`fs::read` patterns without a bounded actual read.

**Bounded source fix:** One existing Rust evaluation owner now uses a bounded read helper that rejects empty, oversized, symlinked or nonregular metadata based on path metadata, checks the opened file's size, caps bytes read via `Read::take(max + 1)`, and requires returned length to match the checked length. Evaluation JSON retains its existing 128-KiB ceiling; actor and dataset manifest checks retain the existing 64-KiB ceiling; reference WAV explanation keeps its 4-MiB ceiling. This is a fail-closed source-read bound, not an OS-level guarantee against all concurrent file replacement. Review ID, eight selected WAV hashes, manual approval, training, engine and VB-CABLE are unchanged. The existing Rust test module and frontend contract test owner received targeted negative cases.

**Proof ceiling:** `STATIC_SOURCE` only. The Rust/TS tests were authored but not executed; no Rust compile, dependency install, Python worker, Svelte typecheck, GPU, audio, installed Windows or Meeting receiver was run. Runtime integrity and model-quality acceptance remain **UNKNOWN / NOT EXECUTED**.

### My Voice P1–P3 contract consolidation — 2026-10-30

**Pinned source parent:** `060860d6a42eff6b312010473c86a84d7344a1f3` on `Local`. Source review traced P1 guided-take advisory intervals through Rust and TypeScript, P2 eight-sentence checkpoint comparison from Python `candidate_evidence` through the canonical Rust review, and P3 reference-selection rationale from Python's original lexicographic ranker through the frozen source WAV and actor reference. The fixed synthetic Python review SHA-256 fingerprint was independently checked for its canonical compact sorted-key form; this is not a Rust compiler/runtime result.

**Demonstrated source gap and repair:** The optional P3 `reference_selection` receipt was accepted with contradictory metadata: `only_eligible` with more than one reported eligible take, a comparative criterion when only one take was reported, or `lower_relative_edge_energy` despite `quiet_edge_comparable=false`. The Rust evaluation owner now rejects these impossible combinations before rendering explanatory evidence. The optional `actor.json` read now refuses oversized, symlinked and non-regular files rather than risking an unbounded status-read merely for display. The My Voice copy labels the eligible count as training-reported; the existing selected reference WAV hash/dataset match remains required. Invalid optional explanation still does not block the eight-listen/human-approval gate or automatically select a voice.

**Proof ceiling:** `STATIC_SOURCE` review of repository source and authored Rust/frontend regression contracts. No Python/Rust/TypeScript/Svelte build, unit execution, real audio recording/seek, GPU/voice quality or installed Windows test was performed. This does not independently verify recorded eligible counts or prove acoustic quality. All runtime claims remain **UNKNOWN / NOT EXECUTED** until the single authorized integration gate.

### My Voice frozen-reference selection rationale — 2026-10-30

**Pinned source parent:** `1b7964fd641f3e5af232b79f028ed106b5447ff0` on `Local`. The existing pinned GPT-SoVITS V2ProPlus `select_reference` ranks only 3–10s eligible accepted recordings using a lexicographic tuple: clipping fraction, comparable relative quiet-edge energy, silence fraction, DC offset, median peer embedding cosine (tie-break), closeness to 5s and stable guided line ID. This was already the source decision; no selected-reference explanation was surfaced to My Voice Review.

**Bounded source change:** The same Python ranking call optionally records its *first distinguishing comparator* against the next-ranked eligible take; it does not introduce scoring, ASR, neural models or a change to the selected reference. The existing `Evaluation/evaluation.json` gains optional selected line/file, WAV SHA-256, duration, eligible count, criterion, and whether all edges were comparable. The Rust evaluation owner exposes only bounded, user-facing provenance if the original frozen dataset manifest has the matching guided line and canonical WAV, both that source and the candidate actor's actual `reference.wav` hash to the recorded selection, and actor reference transcript/duration agree. Missing, old or invalid evidence shows no explanation and does not gate the original eight-listen/manual approval. UI displays this only in a collapsed explanation next to existing Review. Generated/private WAVs, approved actors, package selection and Meeting remain untouched.

**Proof ceiling: STATIC_SOURCE.** Only pinned source, exact owner boundaries and authored existing-suite contracts were reviewed; Python/Rust/Node/Svelte/Windows/GPU/model training, waveform, quality and Meeting playback were **UNKNOWN / NOT EXECUTED**. Lexicographic ranking explains a deterministic selection, not measured SNR, optimal speaker identity or perceptual improvement. This receipt does not independently authenticate maliciously re-authored local metadata.

### P1/P2 reviewed WAV identity and audio lifecycle repair — 2026-10-30

**Pinned source parent:** `5027e3575b5b9c2542488333f83b085c8556bf09`. The Python worker computes `review_id` as a SHA-256 of compact sorted-key JSON containing engine revision, selection method, candidate ID and eight ordered `[line_id, WAV sha256]` pairs. The Rust reader previously checked the ID's hex shape but did not recompute its binding. Modified selected WAV receipts with a retained ID could therefore reuse earlier eight-listen UI state. P1 Start could also begin microphone capture while saved Replay audio was still playing or loading.

**Source repair:** The existing Rust evaluation owner now checks the exact sorted-key SHA-256 fingerprint before admitting Review, using the same Windows CNG digest owner as bounded WAV verification. A fixed cross-language synthetic Python digest fixture tests changed hashes, order and candidate ID. The P1 UI stops pending/playing audio before capture, invalidates Replay when recording ownership or Pending Review changes and closes audio on unknown recording status. The existing P2 horizontal comparison table has keyboard focus and a descriptive accessible region. Training, selected WAV contents, model choice, human eight-listen/manual approval, native route and CI are unchanged.

**Proof ceiling:** Source-only static review plus authored regression cases (`STATIC_SOURCE`). No Rust/TypeScript/Svelte compilation, source test execution, installed Windows playback/capture, focus navigation, GPU/quality or meeting receiver evidence. The fingerprint is consistency binding, not authentication of a fully maliciously re-authored local package; these areas remain **UNKNOWN / NOT EXECUTED**.

### My Voice matched held-out candidate metrics (2026-10-30)

**Pinned source parent:** `1c58280e61a21f762881486bfee8a9e94666732b`. The pinned GPT-SoVITS worker already evaluates 2–3 checkpoints on the same eight authored held-out English lines and writes `candidate_evidence` with WER, speaker similarity, artifact flags and WAV hashes to the existing `evaluation.json`. Before this change, Rust exposed only the selected checkpoint's samples, and My Voice Review provided listening controls without explaining the numeric checkpoint comparison.

**Source-only change:** The canonical Rust evaluation owner now derives a bounded 2–3-row numeric comparison only if all eight line IDs/texts match the authored held-out set, all candidate IDs are unique, scores/flags/hashes are structurally valid, and the prepared checkpoint's evidence matches each currently reviewable selected WAV hash, transcript and metric. Invalid or legacy optional comparison metadata produces **no comparison**, not a false before/after claim or a new approval blocker. The existing build-status API exposes these rows, and the existing Svelte Review offers one collapsed table plus per-sample WER, recognizer transcript, cosine similarity and signal flags. This is a comparison of checkpoints **within this build**, not a measurement of the previously approved voice; nonselected WAV playback/integrity is not independently proven here. The original eight-sample playback completion, human quality confirmation, review ID and Rust actor-promotion gates remain unchanged. No new training model, evaluation subprocess, file store, external provider or workflow was introduced.

**Proof ceiling:** `STATIC_SOURCE` from the pinned source, owned contracts and authored regression cases only. Rust/TypeScript/Svelte compilation, tests, full audio/model inference, GPU use, actual Windows UI/accessibility/meeting audio remain **UNKNOWN / NOT EXECUTED**. WER is recognizer disagreement, not perceptual naturalness; speaker cosine is relative and not speaker authorization/identity proof.

### Bounded timestamped My Voice review (2026-10-30)

**Pinned source parent:** `3f67c38893ab0b058d06c4efa5ddb1d5c77f566b`. The existing `guided_take.rs` review exposed nonlocalized advice on clipping, edge pauses and quiet-edge energy; users could Replay but not go to an indicated point.

**Source change:** `review_guided_samples` now reports up to 12 chronologically earliest, bounded `code/start_ms/end_ms` intervals and a truncation flag, using its existing 32-kHz, 20-ms frame analysis and clipping evidence. Clean takes receive no invented warnings, heavily clipped takes retain their existing quality blocker, and the interval list does not mutate WAVs or training datasets. The same review function is used on newly stopped takes and recovered Drafts after verifying the canonical/native WAV pair. The existing `myVoiceApi.ts` type and mounted `MyVoice.svelte` render accessible timecoded Replay buttons; the same `getTakeAudio` endpoint seeks to the requested time and invalidates stale async fetches. Existing Rust and frontend-runtime test owners received focused source regression cases.

**Proof: STATIC_SOURCE only.** Current source was inspected and regression source authored. No Rust/Node/Svelte compile or test run, actual Windows playback/seek, keyboard/DPI verification, voice listening, GPU training or model quality acceptance was performed. Time intervals are *signal review hints*, never proof of background noise, intelligibility, speaker identity or improved cloning. All target-Windows outcomes remain **UNKNOWN / NOT EXECUTED**.

### Native WRITE arithmetic and STATUS framing/counter integrity — 2026-10-30

**Pinned source parent:** `bc4dc15310438169605b3925dd5c8ee72b31d450` on `Local`. The kernel `TranslateITCaptureWrite` computed `g_written + frames` and incremented cumulative counters without handling `ULONGLONG` overflow. A wrapped logical cursor could invalidate cyclic reader positions, and wrapped telemetry could report misleading low diagnostics. The privileged `TI_CAPTURE_IOCTL_STATUS` handler also ignored nonzero input bytes even though the broker's STATUS operation required zero-length input; the Rust status parser accepted counters showing more buffered or overwritten frames than lifetime accepted frames.

**Source candidate:** Under the existing PnP/PCM spinlock, WRITE now checks `g_written > MAXULONGLONG - frames` and returns `STATUS_INTEGER_OVERFLOW` **before** modifying ring bytes, cumulative counters or expected sequence. Accepted, overwritten and underrun diagnostics use one non-allocating saturating 64-bit helper. Native STATUS rejects nonempty input with zero successful response bytes. The Rust parser rejects inconsistent buffered/overwritten-to-accepted counters; its existing test module has a focused negative fixture. The canonical route source-validator now checks overflow ordering, STATUS input rejection and parser markers. No new owner, wire schema/version, provider, CI job or runtime dependency was introduced.

**Proof ceiling:** `STATIC_SOURCE` review only of source ownership, checked arithmetic, byte-length contracts and regression source; WDK/C++/Rust/Node compilation, regression execution, installed Windows service, driver, headset/microphone, PnP and Zoom/Teams receiver remain **UNKNOWN / NOT EXECUTED**. Microsoft METHOD_BUFFERED defines one shared SystemBuffer with separate `InputBufferLength` and `OutputBufferLength`; this is a source contract, not a measured device result. The single existing target-Windows runbook owns future integrated acceptance.

### Kernel/broker END acknowledgement after FLUSH — 2026-10-30

**Pinned source parent:** `325cc8141b01b714dbe35891e58262614d541662` on `Local`. The earlier broker/kernel state permitted FLUSH to retain the writer FILE_OBJECT while clearing active PCM, but both broker `BrokerDriver::Send(TiBrokerEnd)` and kernel `TranslateITCaptureRevoke` rejected END when PCM was no longer active. Both Rust native `end()` methods skipped the END request in that state and could report Stop success solely from implicit handle cleanup.

**Source candidate:** The kernel accepts exactly one matching generation/source-epoch END after FLUSH for the original FILE_OBJECT under the existing PnP/PCM spinlock, while rejecting duplicate FLUSH, stale identity, different writer and stale PnP handles. The broker now differentiates reserved writer lease from active PCM; successful BEGIN grants both, FLUSH clears only active PCM, and END explicitly releases the lease. Both Rust candidate transports always require an END response, even after FLUSH; a missing response cannot be reported as Stop success. The existing virtual-route validator includes cross-owner source contracts, and one Rust regression case covers missing END ACK with inactive PCM. No driver activation, provider change, wire version, new worker/service or additional test harness was added.

**Proof ceiling: STATIC_SOURCE only.** Source-level contract markers and logic ordering reviewed on the pinned parent/candidate tree. WDK/C++/Rust/Node compilation and execution, GitHub CI, installed Windows/PnP, kernel IOCTL behavior, Mute/Stop tail and meeting receiver all remain **UNKNOWN / NOT EXECUTED**. Existing integrated native acceptance now includes FLUSH → END and wrong-epoch/wrong-writer/race cases; the document is a test plan, not test results.

### Native capture-only broker and WaveRT source boundary — 2026-10-29

**Current `Local` source identity:** `7bec29bb8ec6247a123af6d19faead85143af164`. The capture-only WaveRT microphone, PCM/IOCTL protocol, authenticated SYSTEM broker, Rust pipe adapter and 20-ms pacing helper are present **as unadmitted source candidates**. The native provider is not the Meeting-selected route; the standard VB-CABLE pair remains the only admitted provider. The latest broker change rechecks security identity after each named-pipe header and payload, but checking user/logon/integrity plus the executable's **on-disk** hash does not establish the executing image that wrote each message. Same-user duplicated handles/injection, installed service privileges/ACLs and signed installer trust are unresolved.

**Bounded on-disk executable-file stability repair (2026-10-30):** Review of pinned `Local` parent `6303dfe7f49e4b3a564e5a921c0c185bac178123` showed the approved executable was hashed by a short-lived file open, then closed before the broker session forwarded privileged IOCTLs. The candidate now opens a single `GENERIC_READ / FILE_SHARE_READ` HANDLE, hashes it, and retains the same handle in `BrokerClientIdentity` until pipe teardown. Source validator markers guard open → hash → retain order. Conflicting file write/delete sharing now fails closed during connection. This does **not** prove the original loaded executable image or the identity of every same-user named-pipe writer.

**Proof:** `STATIC_SOURCE` from the pinned source and relevant commit; no WDK/C++/Rust/Node compiler run, GitHub CI, broker service installation, Windows driver/endpoint enumeration, WASAPI/PCM receiver tests or real Zoom/Teams reception on this SHA. These all remain **UNKNOWN / NOT EXECUTED**. Source-only modifications or doc synchronization do not upgrade historical tests to current executable PASS. Required native acceptance remains in the existing [target-Windows runbook](./operations/target-windows-performance.md).

### Consolidated Meeting Stop native owner and integrated acceptance admission — 2026-10-29

**Pinned source:** `OpenITStudios/M-TranslateIT:Local` parent `b2d8eda84b75db374f8e8760d6b1caca6bc99a9e`. Cross-owner source review covered the existing Meeting Start/Stop lifecycle, Python helper authority, finalized Silero/PCM ingress and ASR/TTS session-scoped temporary files, outbound/incoming consumer, playback, optional Meeting Sound, transcript writer, product lifecycle/Close and `application_runtime/shutdown.rs::finish_app_exit`. The existing integrated Windows source/build/package/installed acceptance owner is `docs/knowledge/operations/target-windows-performance.md`, which already groups Meeting scenarios into five flows and requires locked revision provenance; no parallel harness or acceptance registry is needed.

**First wrong owner:** `engine/audio/meeting_sound_capture.rs::stop_meeting_sound_capture_runtime_owned` removed its `MeetingSoundCaptureRuntime` record *before* checking that both thread `JoinHandle::join()` and scoped Silero producer cleanup had succeeded. The failed-start cleanup also ignored failed join/producer cleanup after a capture startup error. A timed-out thread remained owned, but a finished thread that panicked or an unsuccessfully cleared Silero producer could lose the original capture session ID. The next Stop could therefore see `None` and claim success without proof. Separately, `commands/meeting_session/lifecycle.rs::stop_meeting_translation_impl` treated `snapshot=None` with `has_active_session=false` as idle even when `blocker != runtime_session:no_active_session`, then invoked global, unscoped Meeting Sound cleanup. This contradicted the existing fail-closed native-session authority rule.

**Bounded source repair:** The same capture store uses fallible named OS thread admission (with an explicit startup blocker instead of panicking under its mutex), and retains the original `session_id`, `cleanup_pending`, and a sticky `join_failed` bit through Stop/Retry Stop and failed Start until *both* the capture thread and exact-session Silero producer release are verifiable; Stop admission is nonblocking on the capture-store mutex. A previously joined thread is not joined again while awaiting producer cleanup. The existing read-only Sound orphan observation and exact-session `stop_meeting_sound_capture_runtime_for_session` are reused by a no-snapshot function that rechecks canonical native idle ownership before and after scoped capture cleanup. `meeting_session/lifecycle.rs` denies orphan teardown unless the native state report proves idle and invokes the matching Sound-owner path instead of unscoped cleanup. Existing helper, Silero, WAV, consumer, playback, transcript and Stop/Close gates remain unchanged, as do Tauri commands and public DTOs.

**Acceptance readiness, not acceptance execution:** The existing `target-windows-performance.md` Flow 3 now explicitly includes the verifiable native resource-release and cross-session Stop/Close boundary within its one coordinated, locked-SHA test sequence. An existing Rust Sound-owner fixture protects failed capture joins, retry attribution and Mic Test admission; the existing Node source-contract case is reconciled. The source acceptance ceiling is **STATIC_SOURCE** only. Cargo compilation, Rust tests, Node/TypeScript tests, Python/model work, installed Tauri, device/GPU/Meeting behavior, CI and Windows acceptance are all **UNKNOWN / NOT EXECUTED**. Do not use the user's PC for incremental testing, do not dispatch paused CI, and do not claim build-ready or installed-release PASS without the separately authorized coordinated execution.


### Finalized PCM producer attribution and exact-session temporary WAV release — 2026-10-29

**Pinned `Local` parent:** `1edc118663a946ab736acfc29534ecf139059d8c`. `engine/audio/speech_ingress.rs::stop_with_budget` returned the result of a completed `JoinHandle::join` after taking the handle; if the worker panicked the handle disappeared, so a subsequent retry saw no thread and returned true. `engine/audio/finalized_utterance.rs::clear_producer_if` already preserved its CPU ingress lease after an unverified Stop, but no-snapshot Meeting cleanup/status did not inspect that lease or its stored original session. `meeting_session/lifecycle.rs` also released the last finalized sequence identity before confirming temporary file cleanup. In `live_segment_writer.rs`, session-wide removal used a lossy sanitized session prefix (`final_<session>_`) that could select another session's similarly prefixed finalized WAV.

**Source repair:** The same `SpeechIngress` retains `join_failed` across Stop retries and cannot promote a consumed failed join to successful cleanup. A read-only `finalized_orphan_producer_owners()` derives original session and lane/generation from the existing producer state, CPU lease and finalized sequence; owner mismatch, poisoned/contended mutex or invalid lane fails closed. The original sequence is cleared only by its matching session after verified Starting rollback, active Stop or no-snapshot Stop. No-snapshot Stop retains original identity before other owners are cleaned, verifies matching Silero CPU cleanup, and uses the existing native `STOP_CLEANUP_PENDING` gate until helper, consumers, playback, Meeting Sound, deferred audio, transcript, Silero CPU, scoped ASR WAV and scoped TTS WAV cleanup all complete. Deferred work is not cleared while an unverified consumer/producer might still own its WAV.

The original WAV cleanup functions remain canonical. `cleanup_finalized_meeting_session_wavs` now requires an exact, unmodified session ID and validates complete `final_<session>_s<sequence>_<lane>_<identity>.wav[.tmp]` filename grammar, rather than an ambiguous prefix. The existing Meeting TTS scanner now supports exact native numeric generation/sequence validation and staging files. Its no-snapshot variant sweeps only the exact session ID recovered from the finalized owner/sequence, and only after all related native owners are verified released; worker-provided paths and unrelated sessions are never cleanup authority. No cached filenames are inferred when no exact native owner exists.

**Proof ceiling: STATIC_SOURCE only.** Authored Rust regression fixtures for sticky Silero CPU panicked join, contended producer identity reads, collision-safe ASR/WAV cleanup, invalid session IDs and bounded session-scoped TTS/temporary files. Reconciled the existing Node startup/recovery source-contract assertions to the new owner and sequence lifecycle. Any regex matching or static inspection is not compiled/execute proof. No Cargo compilation, Rust unit execution, Node/TypeScript test execution, Tauri build, CI, live Windows CPU/GPU/audio capture, actual filesystem/release smoke or target app acceptance was performed. Native runtime and installed-release readiness remain **UNKNOWN / NOT EXECUTED**. Historical temporary files without a surviving exact owner cannot be safely attributed by the no-snapshot cleanup; startup recovery and user-approved integrated acceptance are distinct owners, not implicit blanket deletion.


### Meeting orphan outbound/incoming consumer and playback ownership — 2026-10-29

**Pinned parent:** `457d6f1f6b48aa07ac1ee0f3a9d903a15b0ac5a2` (`OpenITStudios/M-TranslateIT:Local`). `commands/meeting_session/lifecycle.rs` had a no-snapshot branch that verified helper, optional Meeting Sound and transcript ownership but omitted the existing outbound/incoming consumer and playback `Mutex<Option<...Runtime>>` owners. Their Start functions still refused replacement while the old owner was stored, so a missing Meeting snapshot could coexist with a retained inference/stream thread while preflight and Close appeared idle. Their bounded Stop functions correctly preserved unfinished JoinHandles, but a completed worker that panicked could have its failed JoinHandle consumed and its store removed; a later Stop could mistake the absence for verified cleanup. A successfully joined consumer could also lose its original generation/session even if scoped Silero producer or incoming deferred-WAV cleanup failed. Native Meeting Stop and `application_runtime/shutdown.rs::finish_app_exit` are the canonical stop/close decision makers; no additional Close command is justified.

**Source candidate:** The same `consumer_runtime.rs` and `playback_runtime.rs` native stores now carry a per-owner sticky `join_failed` flag: once a completed JoinHandle fails to join, its original generation/session remains stored and Retry Stop cannot report release. Outbound/incoming consumer owners are also retained when producer or deferred queue cleanup fails after a successful thread join, and the same native Retry Stop can repeat that final cleanup without treating an already-consumed successful JoinHandle as a panic. Their Stop lock admission uses `try_lock()`, returning an explicit unverified result instead of waiting behind an unrelated long-held native store. Read-only `inactive_meeting_consumers_may_need_stop` and `inactive_meeting_playback_may_need_stop` inspect these existing stores without blocking; contention/poison is treated conservatively. Bounded `finish_orphaned_meeting_consumers_without_session` and `finish_orphaned_meeting_playback_without_session` look up original owner IDs, then invoke those *same* native Stop functions only while the canonical runtime is verifiably idle. `MEETING_LIFECYCLE_GATE` excludes a new overlapping Meeting Start during the no-snapshot transaction; matching generation/session checks remain in the stores. The idle Stop path now includes both owners in its existing `STOP_CLEANUP_PENDING` gate and prohibits `already_stopped` until they are verified released. Unscoped deferred queue/sequence clearing is not performed while an orphan consumer remains unverified. Existing Meeting Stop under an active snapshot still reuses the original scoped consumer/playback teardown.

**Proof ceiling: STATIC_SOURCE.** Inspected source owners, no-snapshot Stop, startup/Stop authority, ApplicationRuntime lifecycle/resources/Close and producer cleanup APIs at the pinned `Local` SHA. Authored Rust fixtures for stuck/poisoned-style owner contention, successful orphan generation release, Mic Test admission rejection, and retained failed thread joins, plus an expanded Node source contract; no Rust, Node or TypeScript tests were executed. No Cargo build, Tauri package/install, CI, Windows driver/GPU/audio, live Meeting, or installed Close acceptance was performed. The source repair does not establish measured Stop latency or guarantee cleanup of unattributed finalized PCM producers or session-scoped temporary WAVs; those require the next bounded source review.


### Meeting orphan cleanup with no snapshot and native Close fail-closed — 2026-10-28

Pinned `OpenITStudios/M-TranslateIT:Local` parent `1f3ff1f1a515a9e552a124e68dea923e4e366de5`. `meeting_session/lifecycle.rs::stop_meeting_translation_impl()` had a no-snapshot branch that cleaned optional Meeting Sound, deferred queues and orphan transcript writing, then returned `already_stopped` without examining helper-owned Meeting resources. `transient_stop_cleanup_pending()` similarly checked the existing Stop flag and orphan transcript writer, not the canonical helper's retained Meeting ID. Thus `application_runtime/shutdown.rs::finish_app_exit` could see no session and no pending Meeting cleanup even when native helper resources still carried an orphan. The frontend `closePolicy` treated no-session audio as safe to destroy and offered no explicit path for ownerless `cleanup_incomplete` or temporarily unverified Checking states.

The bounded repair retains the original ownership design and also detects orphan Meeting Sound capture through a nonblocking read of its existing native owner; contention/poison is conservatively treated as potentially occupied. `helper_bridge.rs` exposes only a nonblocking, read-only observation of confirmed Meeting orphan ownership (busy is not mislabelled as a known orphan; native Stop/Exit retain their independent fail-closed try_lock guards), plus an explicit `finish_orphaned_meeting_helper_without_session()` owned by the existing no-snapshot Meeting Stop transaction. It requires a verifiably idle native runtime and the original Meeting ID on the helper; no other worker task is terminated. Its bounded cleanup returns a failure until the original worker and stdin/stdout/stderr owners finish. `stop_child` no longer erases `pending_meeting_cleanup_session_id` merely on child reap: status observation cannot certify Meeting audio/transcript cleanup, so this attribution stays until matching Meeting Stop, verified no-snapshot Stop, or a still-authoritative Live recovery releases it only after a final generation check following Python ping/status. Public Worker Start and native ExitRequested also refuse to silently replace or reap an orphan Meeting owner before Stop.

`meeting_session/lifecycle.rs` combines orphan helper, orphan Meeting Sound capture and existing idle transcript/Stop cleanup admission; no-snapshot Stop verifies that helper before reporting `already_stopped` and retains the existing `STOP_CLEANUP_PENDING` gate on any unverified result. `application_runtime/lifecycle.rs` projects an ownerless `cleanup_incomplete` before Ready/Checking, which the existing `resources.rs` maps to `audio_locked`. The existing close policy can now offer a Retry Stop for that case and refuse optimistic destroy during `checking`, without adding a second owner or a new Tauri command. `application_runtime/shutdown.rs` retains its sole `shutdown_helper() && mark_clean()` authority after Meeting cleanup is verified.

**STATIC_SOURCE only.** Reviewed source ownership across native Meeting Stop, helper process/status/admission, transcript writer, ApplicationRuntime lifecycle/resources/shutdown, and Svelte Close controller; authored Rust/TypeScript regression fixtures (including contended Meeting Sound capture) and a new Node source contract. Any static regex/source mapping performed is *not* Node/Rust test execution. Cargo compilation, Rust tests, Node/TypeScript tests, Tauri build, CI, native Windows ExitRequested and audio/worker process scenarios are all **NOT EXECUTED**. In particular, an un-attributed helper cleanup and any other native/Meeting Stop step that fails verification remain fail-closed rather than being declared repaired.


### Nonblocking Meeting helper Stop and session-attributed Retry Stop — 2026-10-28

**Pinned source:** `6ca6b40e2c49dec2c0e3eb6d358f31a5d858103f`, `Local`. `commands/meeting_session/lifecycle.rs::stop_meeting_translation_impl` already revokes the current Meeting generation, cancels output, and treats `helper_cancel.ok == false` as a reason to retain the Meeting snapshot in `cleanup_incomplete`. But `helper_bridge.rs::cancel_helper_bridge_meeting_session` used `runtime().lock()` even when Python startup held the same mutex through slow ping/status. This blocked a critical cleanup stage behind the worker. Its prior shortcut for `last_error == "helper_bridge:cleanup_incomplete"` invoked `stop_child` without requiring that the pending cleanup belonged to the stopped session; this risked cross-session reaping and false release confirmation.

**Source repair:** The existing Meeting helper cancellation owner now uses `runtime().try_lock()`. Contention returns an explicit `helper_busy` / `ok=false` outcome with no asserted helper generation, preserving the already-revoked Meeting session for Retry Stop; poisoned locks remain distinct errors. The existing generation-scoped outbound CAS is performed only for the requested session, even if helper admission is temporarily unavailable. `commands/helper_bridge_runtime.rs::stop_child_with_budget` now captures the current active Meeting session into a single `pending_meeting_cleanup_session_id` on the existing runtime owner before writer/reader cleanup begins. Bounded failure retains that session ID independently of `set_blocked`/transport clearing active request fields. Cleanup success clears the attribution only after child reap, stdout/stderr/stdin thread joins and the preexisting sticky join-failure gate all pass. Retry Stop cannot reap a pending helper from another or unknown session: only the exact matching retained ID allows the bounded retry. No pending cleanup without a provable exact session is promoted to a clean Meeting Stop.

**STATIC_SOURCE only:** source-level review of the existing Meeting stop authority/revocation and cleanup `helper_cancel.ok` gates, `request_policy.rs` generation-scoped outbound CAS, helper runtime ownership and transport error paths. Added one Rust retained-owner fixture and three Rust helper-cancellation fixtures (mutex busy, wrong-session retry, un-attributed error); amended the existing Node source contract. Checked 114 simple regex assertions against staged/current text without running tests. No Cargo compilation or Rust tests, Node test execution, CI, Windows live helper startup/inference, Stop latency, audio, devices, native shutdown, or package acceptance was performed. A source-level `try_lock` removes this specific blocking helper admission but does not prove every other Meeting Stop step is nonblocking. The previously un-attributed cleanup case deliberately remains fail-closed until its distinct native helper owner is verified or repaired.


### Worker failure restart admission and Meeting generation fencing — 2026-10-28

**Pinned source:** `896f26d22138560c83cd77fd1cb461552cb98890` on `OpenITStudios/M-TranslateIT:Local`. The current worker helper is used by application `ensure_runtime_ready`, `fix_setup`, explicit `runtime::start_helper_bridge`, and Meeting Stop recovery. The facade in `commands/runtime.rs` checked active session ownership, but several direct calls to `helper_bridge::start_helper_bridge` bypassed it. Consequently an ordinary worker restart could race with an active Meeting/Mic Test/recording or an installer reservation despite the native application being designed to exclude those operations. Separately, `helper_bridge/transport.rs::recover_live_meeting_helper_transport` checked its Meeting generation before scheduler admission and just before calling `start_helper_bridge_internal(false)`, leaving an admission window while waiting for the helper mutex. The internal restart could replace the worker even after its Meeting generation was revoked.

**Source repair:** `helper_bridge.rs` preserves the existing single process/transport runtime owner. Ordinary public startup now acquires the existing updater operation permit through cold Python startup and denies restart unless `latest_runtime_session_state` proves no session or unresolved blocker, including for direct setup-intent callers. Inside the helper mutex, it repeats the audio-owner check before mutation and once more after the bounded prior-child cleanup; only the successful admission can clear the existing outbound marker or spawn Python. Live Meeting transport recovery passes `Some(generation)` to this internal owner and revalidates `live_outbound_generation_is_authoritative` while it owns the mutex, and after cleanup before process creation. This does not give arbitrary public start permission to restart a Live Meeting helper and does not create a parallel recovery supervisor. Stop still owns revocation and cleanup; an authority revoked after the last check or during a long Python import remains a separate concurrency boundary.

**STATIC_SOURCE only:** reviewed exact helper call sites, updater operation permit, session state fail-closed semantics and the existing Live recovery scheduler/Meeting generation checks. Authored three Rust regression cases: direct Start while Mic Test owns audio, public Start while updater installation is reserved, and stale/revoked Live generation rejection before process creation. Extended the existing Node startup recovery source-contract test for guard placement, exact generation routing and marker ordering. No Rust/Node test execution, Cargo compilation, TypeScript/Svelte build, CI, installed Windows application, GPU, microphone, real Meeting Stop or helper process was run. These changes are **NOT EXECUTED** as native behavior and cannot establish a passing build or prove that all helper recovery races are eliminated.


### Cross-module helper thread admission and join-failure source repair — 2026-10-28

**Parent source:** `2ced3899f78155db05b85af2370eca3104a65dc4` (`Local`). A source-only cross-module review of the latest single-instance, app shell, updater, application snapshot, My Voice Tauri and helper I/O changes found the Tauri registration and TypeScript `runCommand` shapes for `get_voice_lab_build_status`, `get_application_snapshot`, and `dispatch_product_intent` still aligned. However, `scripts/tests/startup_recovery_contract.test.ts` contained outdated positive assertions after successive write/read revisions: it expected `stdout_reader = Some(thread::spawn(...)` and direct adjacent `stdout_reader.is_none() && stderr_logger.is_none()` while current cleanup also requires stdin writer completion, and it expected a single-line call for startup status reading that had become multiline.

**Source repair:** The existing `helper_bridge_runtime.rs` now uses fallible `thread::Builder::spawn` for the stdout reader (consistent with the already-fallible stdin writer) and stderr logger, with specific source errors rather than panicking while the canonical helper mutex is held. `helper_bridge.rs` routes stderr logger admission failure through the already-owned child and bounded `fail_start_after_spawn` cleanup; no additional process or thread authority exists. The canonical `HelperBridgeRuntime` now retains a sticky `io_thread_join_failed` flag whenever a completed stdin writer, stdout reader or stderr logger has a failed `join()`. Bounded `stop_child_with_budget` still attempts child termination and joining remaining owners but cannot report success on later Retry Stop/ExitRequested just because a failed join consumed its handle. One Rust panic-join fixture protects this invariant. The existing startup/shutdown Node source contracts now match the actual multiline read call, fallible stdout admission and all three I/O thread cleanup prerequisites.

**STATIC_SOURCE only:** inspected source and calls against pinned `Local`, prepared updated contracts, and matched 83 straightforward `assert.match` / `assert.doesNotMatch` regex conditions to the exact current/staged source as a nonexecuting source check. This number **does not** mean 83 Node tests executed or that Rust compiles. No Cargo compilation, Rust unit execution, Node test execution, Svelte typecheck/build, CI, native Windows helper/pipe tests, installed app, GPU, microphone, Meeting or release acceptance was performed. The source changes reduce specific integration risks; executable build readiness remains **UNKNOWN / NOT EXECUTED** until the separately approved integrated acceptance phase.


### Worker stdin pipe write deadline and backpressure — 2026-10-28

Source candidate based on `004af9fb63cfe6cb7c486154e32270c97ccc002d`: `helper_bridge_runtime.rs::write_worker_request_with_deadline` previously called synchronous `write_all()`, newline write and `flush()` on the current thread. `request_deadline_payload` already inserted `deadline_unix_ms` in JSON, but the OS pipe operations themselves had no host-side wait deadline. During cold Python import or when a worker stopped consuming stdin, a blocked native write could prevent prompt Stop/shutdown or retain the single helper resource indefinitely.

The existing `HelperBridgeRuntime` now owns `stdin_writer: Option<JoinHandle<()>>` alongside its stdout reader and stderr logger. The same writer function still inserts the existing protocol metadata and serializes one JSONL request. A bounded completion channel (using the existing task-specific response deadline) and a fallible thread-spawn admission now separate the potentially blocking write/flush from the waiting caller. On timeout or disconnection, its JoinHandle remains owned until the process and stdin/stdout/stderr threads all finish and the existing bounded `stop_child` verifies their joins. No old handle may be overwritten, and a task must pass its original generation check before any completed pipe is published back to the runtime.

Startup ping/status use the owned writer while holding the canonical helper mutex. Transport requests register writer and reader under that same mutex before releasing it to await bounded pipe I/O. On successful response the original stdin and stdout must both be returned and both thread handles joined before allowing another request. A failed startup status write now explicitly blocks startup instead of falling through to ping-only Ready. Request IDs, scheduler priority, Python wire format and all configured task deadlines remain unchanged; the writer and reader each use the applicable existing deadline rather than a new user-facing timeout policy.

**Proof: STATIC_SOURCE** — reviewed the exact pinned native owner, two startup write call sites, all worker task transport admissions/completions, Stop/restart cleanup and prior regression tests. Authored three native Rust regression fixtures for blocked write, blocked flush, owned-handle retry and successful JSON metadata/pipe return, and updated the existing Node source contract for reader/writer ordering, startup fail-closed status and cleanup. Rust/Node execution, compilation, hosted CI, Windows pipe pressure, child-process behavior and installed Meeting/audio acceptance are **NOT EXECUTED**. This is a source-level deadline/ownership repair, not measured timing or runtime certification.


### Stdout response deadline reader ownership — 2026-10-28

Pinned source parent `13b17b42680a1cfc8e16f762f738f94a57f2ea07` showed that `read_worker_response_direct_with_deadline()` previously used `thread::spawn` to move the owned `BufReader<ChildStdout>` into a read thread, but discarded its `JoinHandle`. A `recv_timeout()` failure could therefore return while the reader still held the original stdout pipe, outside the reach of the helper's existing bounded `stop_child()` cleanup. Child termination alone could not prove that a descendant-inherited pipe or reader had finished. The root owner remained `commands/helper_bridge_runtime.rs`.

Source candidate: `HelperBridgeRuntime` now retains `stdout_reader: Option<JoinHandle<()>>` and registers it *under the same canonical helper mutex* before transport request I/O, or while cold-start ping/status still owns that mutex. A bounded channel continues to deliver the parsed response and original stdout buffer; a completed successful response must join its registered reader before the pipe can be reused. On a timeout, the JoinHandle stays attached to the runtime instead of being dropped. `stop_child_with_budget()` checks the reaped child and both stdout/stderr readers, joins only completed reader threads, retains unfinished handles for explicit retry, and otherwise returns false. Existing helper Stop/ExitRequested and Meeting Retry Stop already propagate this false result without replacing the worker or writing a clean shutdown marker. The transport keeps the previous generation check before committing a response; no scheduler/worker protocol/deadline changes were made.

**Proof: STATIC_SOURCE.** Exact-`Local` inspection of startup, task transport, cleanup, existing shutdown gate and tests; authored two Rust stdout reader fixtures (timed-out blocked reader remains owned then reaped, complete reply retains original pipe) with a second-reader exclusion assertion, plus a bounded existing Node source contract. Rust/Node execution, compilation, hosted CI, Windows inherited-handle behavior, model inference, latency, Meeting audio and installed application acceptance are **NOT EXECUTED**. Code-level ownership and fail-closed behavior are prepared, but no live Windows cleanup guarantee or performance improvement is claimed.


### Python helper process cleanup & fail-closed exit — 2026-10-28

Source candidate based on `b92e7e065fa7c79d92984d1851cdb72e4b813576`: `helper_bridge_runtime.rs::stop_child` previously called `child.wait()` and `stderr_logger.join()` without a deadline while holding the canonical helper mutex; both return outcomes were ignored. This can stall native clean exit and cause a misleading success if errors are ignored. The bounded source fix uses `try_wait` to establish child termination, a termination request only while needed, and `JoinHandle::is_finished()` before joining the owned stderr reader. Its explicit 250 ms cleanup budget caps *one attempt* rather than silently accepting a timeout; an unreaped Child or still-running stderr JoinHandle remains in the same runtime state, and `stop_child` returns false. No detached/replacement logger, second process owner or change to Python worker protocol is added.

`helper_bridge.rs` now owns spawned child immediately, including stdin/stdout initialization failures, and treats unsuccessful cleanup as `helper_bridge:cleanup_incomplete` with no new worker. Worker-exit observation, both transport error paths, Meeting hard cancel and Retry Stop propagate that condition rather than overwriting it with Stopped. ExitRequested uses `try_lock` for helper ownership to avoid waiting behind Python startup/inference; it refuses clean exit and preserves the recovery marker if the child/logger is not confirmed released. The existing `application_runtime/shutdown.rs` gate remains the only authority for calling `mark_clean_shutdown` after helper shutdown succeeds. The worker's stdout deadline reader remains a distinct, preexisting thread boundary and is **not** claimed to be fully reaped by this change.

**STATIC_SOURCE:** inspected all current stop_child call sites and the session-scoped Meeting cleanup contract; authored two Rust bounded-cleanup fixtures, three Rust exit/Retry Stop fixtures and one Node source contract. All new Rust/Node tests, typecheck, CI, native Windows child-process/pipe behavior, audio and installed-app shutdown are **NOT EXECUTED**. This is not live proof of a particular cleanup duration or of full descendant-process reclamation.


### Helper status mutex contention — source repair 2026-10-28

Candidate based on `c5670153b992fe6bbffbe1421e0f1abf15bc76e4`. A read-only `get_helper_bridge_status()` previously used blocking `runtime().lock()` even while `start_helper_bridge_internal()` held the same mutex for Python worker creation, ping and startup status response. This also affected Meeting preflight and ApplicationRuntime snapshots. The updated helper uses `try_lock()`: when the canonical owner is busy it reports `checking` without inventing a verified generation, model readiness, stopped state, error or worker process identity. A poisoned lock keeps the existing failure status. Logging of a successfully obtained status is moved outside the helper transport lock. Public helper Start refuses a duplicate restart during contention. The actual mutex-owning startup code checks if a competing caller already established a live, ready worker before clearing the outbound marker or replacing the process. No worker protocol or inference deadline changed.

Native ApplicationRuntime and frontend product-readiness owners now classify this temporary unverified status as Checking rather than Ready or a confirmed setup failure. An authoritative Live Meeting continues to use its own session state; Stop/cleanup ownership is unchanged. Three focused test cases were authored: Rust held-lock status/duplicate-Start fixture, Rust lifecycle mapping, and TypeScript readiness mapping.

**STATIC_SOURCE / NOT EXECUTED:** owner/call-site review and authored test source only. No Rust build/test, frontend typecheck/test, Python worker, CI, Windows cold-model load, installed-app, physical audio or target latency was executed. This is a source-level reduction of status mutex waiting, **not** a measured claim that historical startup delays or OS Not Responding are eliminated.


### Cold startup shell and My Voice status responsiveness — 2026-10-28

Candidate based on `a90f72e1e797ab5546b0be6372554825112577a8`: returning-user `App.svelte` no longer awaits the canonical Python helper's cold `ensure_runtime_ready` intent before ending the initial shell loading state. First it loads persisted settings and one native `ApplicationSnapshot`, maps it through the existing application controller with event sequencing and displays the resulting Meeting workspace. Only if the authoritative helper reports `not_started`, one non-blocking intent then starts the same single worker; its resulting native snapshot is reconciled by the existing monotonic ApplicationRuntime controller, including when the event subscription was not yet ready. Failed/blocked workers are not retried automatically, and unavailable native snapshots do not grant readiness. Native `get_voice_lab_build_status` now sends its existing read-heavy `current_status` work to Tauri's blocking executor, keeping the response DTO and canonical voice semantics unchanged. No model, lockfile, inference/prompt, helper deadline, worker count, polling, cloud or startup service was changed.

**Proof: STATIC_SOURCE** — exact source and ownership inspection, one added source contract for initial shell ordering/worker non-retry and one native status offload contract authored. Node/Svelte/Cargo builds and tests, Windows UI responsiveness, cold-import timing, GPU/device, installed app and CI remain **NOT EXECUTED**. This source change removes one sequential wait from initial UI presentation; it does not prove the historical OS 'Not Responding' window or the 30s worker cold-start defect is resolved.


### Native updater install/admission exclusion — 2026-10-28

Source candidate based on `bc474eb708520d30592e6fd0d26aead985c0d9d2`: the final updater owner check now occurs atomically with an exclusive application admission reservation, after a candidate update is found but before download/install. Meeting/Mic Test/recording lifecycle state and VoiceLab build/cleanup are retained as canonical authorities. Temporary admission permits cover Meeting route preparation, audio session claims, My Voice build claims, voice selection and backup/restore, and guided Draft Save/Retry/Recover plus explicit legacy protection. Unresolved Meeting cleanup, unverified runtime ownership and voice backup also block updater installation. A failed download/install drops the reservation, while a successful installer handoff keeps admission fenced until the application process exits. No updater daemon, worker, dependency, private audio, release or model behavior was added.

**Proof: STATIC_SOURCE.** Exact-head source/owner and static gate-order review; three Rust gate regression cases and two added Node updater source-contract cases authored. Cargo, TypeScript/Node execution, CI, installer/update failure injection, native devices and installed-app behavior were **NOT EXECUTED**. This code does not establish successful Windows installation or guarantee external installer recovery.


### Early Windows single-instance admission — 2026-10-28

Source candidate based on `e93d5600e4b3427cf32ab48c9e732ec794eed1b0`: native startup now acquires a session-scoped named Windows kernel mutex before Tauri setup, application data/recovery, audio and worker owners. A second invocation exits without a Tauri builder, after attempting to show the exact-executable main window; if none is ready it displays a bounded informational notice. A failed mutex acquisition also refuses startup. The primary handle is retained through the event loop and released by the OS on process termination. Existing startup-marker and Meeting ownership policies are unchanged; legacy already-running builds without the mutex cannot be retroactively protected.

**STATIC_SOURCE:** reviewed native API ordering, exact-image/title filtering, mutex lifetime, collision and fail-closed branches, plus two authored Node source-contract tests. No Cargo compilation, Node execution, CI, Windows duplicate-instance/focus tests, clean install or device behavior was executed. In particular foreground focus is subject to Windows restrictions; a static check cannot claim it was raised on a real machine.


### Status/callback ingress coexistence repair — 2026-10-26

The candidate based on `c48b8bb6f2bc9cb00eea852e7f05ca64b17363f9` replaces the
existing ingress mutex with a standard-library read/write lock. Status readers
and callback admission share read access; Start/Stop retain exclusive write
access to the same CPU lease. Callback admission remains nonblocking. No new
state/cache/dependency, queue capacity, VAD/model/prompt or loss policy is added.
An actual queue contention/overflow still invalidates continuity and stays counted.

**Red → green INTEGRATION_FIXTURE:** the synchronized real-status/Silero probe
was first changed to require zero ingress rejection and failed on the old mutex
(one versus zero). After repair, the identical overlap produced zero ingress
rejections, zero in all handoff reasons, no invented source discontinuity, and
106 Silero windows. Stop joined and restart loss remained zero. The existing
47-input WAV replay and lifecycle checks completed in 43.23 seconds; late PCM,
source reset, stale Stop and explicit format-failure checks passed. This is not
a new ASR/MT/TTS quality evaluation. Ordinary tests additionally verify a read
guard permits callback admission while an exclusive lifecycle guard still rejects
it without blocking.

**EXECUTED_SOURCE:** Rust 339 passed/one opt-in ignored; frontend quick suite
408 passed with source contracts/typecheck. Private logs:
`.workspace/reports/pcm-status-repair-red.log`, `pcm-status-repair-green.log`,
`pcm-status-repair-source-final.log` and `pcm-status-repair-frontend.log`.
Changed native capture/status/Stop acceptance is **NOT EXECUTED** on this candidate.
The two historical native drops remain unattributed, and the separate incoming
native error/uncontrolled recognized phrases retain their earlier limits.

**Exact-source admission update:** clean
`ce2beb57c95c7df6d380a11a0bdebe4e90cbd9a9` passed the locked frontend/Rust build
and frontend quick 408 checks. A fresh default-parallel Rust run then returned
**335 passed / 4 failed / 1 ignored**. The first failure was scheduler deadline
test `waiting_meeting_incoming == 0` observing one; the other scheduler and bridge
contract failures followed a poisoned shared scheduler lock. Scheduler tests use
their own serial guard while bridge contracts also access the global scheduler;
this is an isolation hypothesis, not a proved production audio regression.
A separating `cargo test ... helper_bridge -- --test-threads=1` passed 32 tests;
it does not erase or replace the failed full-suite admission result. This
independent helper/scheduler test boundary requires its own bounded diagnosis;
no scheduler/runtime change was made under the audio repair.

**Separately authorized test-isolation repair — 2026-10-27:** candidate based on
`e9ba443c567304e5acb77a15566f6d6746067a8a` removes the scheduler tests' private
mutex and delegates their guard to the existing `runtime_test_guard` used by
bridge contracts. The shared process-global scheduler now has one test isolation
boundary; production scheduler admission, priority, deadlines and audio are
unchanged (`cfg(test)` code only). The ordinary parallel Rust suite passed
339/one opt-in ignored; the helper subset passed 32 with `--test-threads=32`.
No assertions were weakened, no global serial runner was imposed, and no tests
were added merely to assert source shape. Private logs:
`.workspace/reports/scheduler-isolation-fixed-rust.log` and
`scheduler-isolation-fixed-helper-parallel.log`. The earlier full-suite failure
remains historical evidence; the repair is not native proof.

Private exact-source logs: `.workspace/reports/pcm-status-native-ce2beb57-build.log`,
`pcm-status-ce2beb57-rust.log`, `pcm-status-ce2beb57-frontend.log` and
`pcm-status-ce2beb57-helper-serial.log`. Developer launch succeeded, but the user
stopped Computer Use before Start and requested work without PC control. No
native capture/export/Stop outcome was collected for this source. Further UI
interaction is deferred under that instruction; no microphone action or manual
user testing is requested. Goal completion remains unproved after the source-test
repair: native capture/export/Stop acceptance is still outstanding.

### Controlled status/callback contention reproduction — 2026-10-26

The candidate based on `4dae67517a7294339c268659f55779666895c4e0` extends the
existing opt-in public WAV replay/lifecycle fixture only; production behavior is
unchanged. A34's existing hash-verified natural synthetic PCM runs through real
local Silero. The test holds its nested fault mutex, starts the actual
`finalized_speech_fault(true)` observer, confirms that observer owns ingress, and
invokes the actual finalized PCM callback entry. This deliberately forces one
specific overlap; it does not measure ordinary status-lock hold time or natural
failure frequency, and is not a reproduction of the historical native session.

**INTEGRATION_FIXTURE:** one ingress rejection, all handoff reasons zero; the
canonical CPU owner acknowledges discontinuity, then processes 105 Silero windows
after paced fixture input. Stop joins, restart reports zero loss. The existing
opt-in replay completed in 58.31 seconds with 47 input receipts; these receipts
are replay evidence, not a new ASR/translation/TTS quality PASS. The existing
lifecycle controls passed. **EXECUTED_SOURCE:** Rust 339 passed/one opt-in ignored
in the ordinary suite. Private log: `.workspace/reports/pcm-status-contention-fixture.log`;
its `PCM_CONTENTION_RECEIPT_JSON` contains numeric observations only.

The shared ingress lock is a proved mechanism by which status observation can
reject callback PCM independently of queue capacity or model speed. Queue lock
contention and overflow remain separate mechanisms. The two historical microphone
drops and previous incoming native error remain unattributed. This characterization
probe characterized the defect at that candidate; the repair above converts it
to a no-ingress-loss acceptance assertion. Historical failure is not product law.

### PCM loss cause observability — 2026-10-26

The owner approved extending the existing Diagnostics export with numeric-only
drop reasons. The candidate based on `41a0031cc40f050f5e194084744ea3186919921b`
adds per-lane `pcm_drop_breakdown`: `ingress_rejected_callbacks` counts failed
ingress-lock admission (busy or poisoned); `handoff` separates `lock_busy`,
`buffer_full`, `chunk_too_large` and `poisoned`. A null handoff means its current
owner is absent or unavailable, not proof of zero queue loss. Independent atomic
reads are observations, not a transactional sum with `pcm_drop_count`.

The existing loss decisions, discontinuity/reset, queue capacity, VAD, native
error classification and engine remain unchanged. No audio, transcript, session
identity or raw native error is added. Stop rejection remains excluded from loss;
fresh session counters start at zero. No periodic diagnostic task is added.

**EXECUTED_SOURCE:** Rust 339 passed/one existing opt-in test ignored; six existing
Diagnostics privacy/source-contract tests passed. The audio subset passed 95/one
ignored. Tests exercise actual ingress rejection, each queue loss reason, Stop
exclusion, session reset and the numeric serialization whitelist. These source
results do not identify the cause of the historical two microphone drops.
**LIVE_RUNTIME at clean `c0f962902a2f77380401957d53b365e5cc97d038`:** locked
frontend/Rust build passed and the frontend quick suite passed 408. Developer
Start with Built-in Male reached Live; explicit Diagnostics exported the new
breakdown. Microphone reported 10,965,120 received frames, zero native errors,
zero PCM drops, zero ingress rejections and zero in every handoff reason. Incoming
reported an active stream but zero received frames/observations; its zero error
and drop counters do not prove incoming audio reception or a stalled device.
Both speech-pipeline blockers were empty. The current device guard was healthy;
the displayed last incoming-error incident was historical, not a current error.

Normal UI Stop returned Ready, the empty archive finalized, normal app exit was
zero and no canonical worker remained. No controlled speech or recognized phrase
was observed. This verifies export/lifecycle behavior, not acoustic isolation,
translation quality, receiver delivery, or loss-free sustained Meeting use.
The historical two drops did not recur and still have no proved cause; no
transport-policy fix or retrospective attribution is claimed.

Private exact-source evidence: `.workspace/reports/pcm-drop-native-c0f96290.json`
SHA-256 `5d3ca128b3eeebf0da0750918705eed1586a35e9c711679c1c1910222495f8bf`.
The adjacent `.receipt.json` records numeric observations, source/build-log
identity and normal cleanup; no transcript/audio body is retained in the receipt.

### Capture-health separation source repair — 2026-10-26

Parent Local `25f9a96f2f3cbef2ecf4133d0e7e2a7b877403fc`. Both capture-status
builders had added internal PCM drops to native callback counts and could replace
the latest native error with a speech-processing fault. Two tests of those actual
production status projections reproduced the false native counts (3/4 with zero
native errors) before the fix. Reports now retain native count/latest error,
separate PCM drop counts and existing speech-pipeline blockers. No audio admission,
discontinuity discard, model, prompt, threshold or recovery mechanism changed.

Health classifies native errors as before, pipeline faults as capture failure,
and PCM loss as degraded capture without device-loss flags. Required stream
progress/device/route failures retain priority over historical PCM loss. Original
passthrough does not inherit speech-pipeline prerequisites. The existing advisory
notice handles nonhealthy capture; matching incoming snapshots retain real native
or pipeline failure. Diagnostics adds only counts, frame progress, stream-active
flags and sanitized pipeline blockers, never raw callback text or device names.

Pre-delivery source evidence: Rust suite 338 passed/one ignored; frontend suite
408 passed; source/bridge/privacy contracts and Svelte typecheck passed. Tests cover
native versus PCM separation for both lanes, simultaneous native/fault/loss
priority, explicit pipeline-fault recovery, inactive reports and privacy fields.
The same checks and locked frontend/Rust compile gate then passed on clean repair
commit `ba84abf18e0a155e0f7708a8d4ce4d1ceae7e05b`. These are **EXECUTED_SOURCE**
results; unrelated worker/model-quality suites were not rerun.

**Native separating evidence at that exact commit:** Built-in Male Start reached
Live. Explicit existing Diagnostics export reported:

| Lane | Native callback errors | Internal PCM drops | Frames received | Speech-pipeline blocker |
|---|---:|---:|---:|---|
| Microphone | 0 | 2 | 15,102,720 | empty |
| Incoming Meeting Sound | 1 | 0 | 3,006,240 | empty |

The health guard correctly retained `device_loss:meeting_sound_callback_error`,
with optional-device-loss true, required-device-loss false and no required action.
This proves native error evidence is not suppressed by the repair. Microphone PCM
loss remains independently observable; it is not counted as a native failure.
One snapshot's nonzero frames are not a sustained-progress or loss-free claim.
Old unseparated warnings cannot be retrospectively attributed from this new run.

Normal UI Stop returned to Ready, the local archive finalized, normal application
exit returned zero, and the matching canonical worker process query returned zero.
No controlled utterance was supplied; two recognized phrases nevertheless appeared.
Their input origin is unresolved and is not model-quality/receiver evidence. No
transcript body or audio is included in these diagnostic artifacts or this record.
Incoming remained automatic, so outbound-only acceptance is not claimed.

| Private exact-source native evidence | SHA-256 |
|---|---|
| `.workspace/reports/capture-health-native-ba84abf1.json` — original redacted support bytes | `a04e3e4b111a68cd76447da35269ae8aebc36ecc9e6d4a02c6944afa79855e25` |
| `.workspace/reports/capture-health-native-ba84abf1.receipt.json` — source/log hashes and bounded observations | `02ee75ba7d12e3b3900d10372903c919b53b3a703989ab9eeb072d6e69848ec3` |

Proof ceiling: **LIVE_RUNTIME capture-health separation**, not full Meeting,
physical speech quality or installed-product acceptance. The two PCM drops and
actual optional native error still require owning capture diagnosis; no retries,
queue enlargement, VAD change or engine intervention follows automatically.

### No-speech native Start/Stop observation — 2026-10-26

Executed on clean Local `535a3daab89f61985abb4b9bfdc27ec83de86362`; only
documentation differs from the earlier `71a731ef` source gate. `manage.cmd run`
launched the native development app. The owner was explicitly told that no speech
was required. Built-in Male was verified selected; one Start reached Live, but
the required-microphone native callback warning reappeared before any recognized
phrase. The visible transcript remained empty. This reproduces the capture-health
symptom without a spoken translation task; it does not identify the native error
cause or prove the speaker environment was physically silent.

Stop was invoked through the real Meeting UI. The app returned to Ready to
translate with Start available, no active transcript, and the empty session's
archive marked Finalized (entry count checked). The stopped app then closed
normally; `manage.cmd run` exited zero and no matching canonical worker process
remained in the process query. No force termination was needed. These are bounded
native lifecycle observations, not long-duration stability, receiver delivery,
voice naturalness or proof that capture was healthy. Start-to-Live and Stop times
were not precisely measured. Capture acceptance remains **FAIL/UNKNOWN cause**;
normal UI Stop and app exit were observed successfully in this run.

**Correction to the isolation plan:** Settings currently offers a Meeting Sound
device selector, not an Off control (`Settings.svelte`); the native incoming
activation is automatically scheduled for eligible sessions
(`meeting_session/incoming_activation.rs`). No supported Off action was performed,
no hidden configuration was written, and incoming remained active. Therefore this
is a no-speech Start/Stop test, **not an outbound-only baseline**. The runbook and
continuation now reflect that limitation instead of requiring a nonexistent
control. No toggle, fixture injection, threshold change, model/prompt patch or
context mechanism was added. Diagnose the callback symptom at its existing
Windows capture/health owner before further Meeting quality claims.

### Integrated candidate source gate and first native observation — 2026-10-26

Executed candidate: clean Local `71a731efef24d255fb77dac00257adb8a1abf8b1`.
Repository/frontend contracts passed; Svelte reported zero errors/warnings and
407 frontend tests passed. Worker compile/static checks passed. The first worker
suite was blocked by missing symlink privilege (`WinError 1314`) and a separate
access-denied cleanup of an old pytest temporary link. A bounded administrator
process reran the unchanged complete worker suite with a new temporary directory:
326 passed, zero failed, two POSIX-only process-group tests skipped on Windows.
The two symlink-security tests executed successfully; no test was weakened.
The existing audio source profile passed all four validators. `manage.cmd build`
rebuilt the frontend and passed `cargo check --locked` with 16 Rust warnings.
These are **EXECUTED_SOURCE** results, not Rust-unit, linguistic or native-flow
acceptance. No model/engine/dependency change or new model-quality run occurred.

| Private evidence | SHA-256 |
|---|---|
| `.workspace/reports/worker-admission-71a731ef-6a360b3d3eea4657bfcab495a3edb7a3/receipt.json` | `498097af62f11ccbc951f80c4fd86f0b35c13df761541bdb47256d3ba8c261e5` |
| `.workspace/reports/worker-admission-71a731ef-6a360b3d3eea4657bfcab495a3edb7a3/junit.xml` | `cc321bf2187c0bd34d91fac2a68dff45b0c6ff3ffec2883b0524b36196e87197` |
| `.workspace/reports/candidate-audio-source-71a731ef-20261026.json` | `42a11a8c38e87d1a1fbc446432205232c092e119561a11197abac9b9f3f229d6` |
| `.workspace/reports/candidate-compile-71a731ef-f25d8e7a9475485da82a4dc2bcaa6a8c.log` | `3523779d7ea2e44ab2f96ee0ec2cf31071991f618dd22868c783ddaf420cf6fa` |

`manage.cmd run` then actually compiled and launched the native Tauri development
app at that same candidate. Windows UI observations confirmed Built-in Male
selected, Mic Test entering/exiting its owned state, and translated Meeting Start
reaching Live without My Voice training. Microphone quality reported Too quiet.
The observation at approximately 18 seconds after the Start click is not a
measured Start-to-Live latency. No controlled spoken turn or independent receiver
delivery was proved.

This was **not the controlled outbound-only baseline**: optional incoming remained
on Windows Default, and the owner subsequently confirmed YouTube was playing
through speakers. Endpoint-wide Meeting Sound can capture that output, and
speaker sound can also reach the microphone; uncontrolled incoming transcripts
are not fixture/model-quality evidence. UI reported native callback errors first
for optional Meeting Sound and later for the required microphone. The underlying
device/callback cause is **UNKNOWN**; playback alone does not establish that cause
or a source defect. The proposed outbound-only follow-up assumed a supported
incoming Off control; the later no-speech observation above corrects that
assumption. Preserve the existing engine and thresholds while separating the
native callback cause from unrelated speaker playback.

Normal Meeting Stop remains **NOT VERIFIED**: UI automation reported stale state
and concurrent user input, preventing confirmed Stop. Emergency cleanup terminated
only the identified development app; its known worker and WebView processes were
observed absent afterward. This is not Stop-lifecycle acceptance. Automatic local
transcript retention occurred; no ambient text/audio was copied into repository
evidence or deleted. The native flow remains **Perlu pemeriksaan**, not a full
Meeting PASS; subsequent documentation does not promote this revision's proof.

### Owner disposition of six translation diagnostics — 2026-10-26

At Local `f9114530e7172931ff1a165d2544e9a752362753`, the owner reviewed the six
discussed outputs, considered them understandable/minor for this stage, and
authorized continuation without piecemeal engine intervention. This disposition
is limited to the retained outputs identified below; it does not establish that
their meanings are equivalent or that similar future errors are acceptable.

| Retained case | Discussed limitation |
|---|---|
| `en-id-technical-001` | Regenerate rendered as restore |
| `id-en-unicode-001` | Quoted filename translated |
| `id-en-negation-010` | Approve rendered as sign |
| `en-id-meet-010` | Temporary topic deferral rendered as cancellation |
| `en-id-quant-011` | Maximum-five constraint obscured |
| `en-id-budget-purpose-001` | Construction purpose added to unspecified budget |

Evidence identity remains the original captured `81a16a50` outputs and the
`meaning-first-final-87b5e3e.json` regrading receipt below. The diagnostic counts
remain **20 PASS / 2 REVIEW / 6 FAIL**, with zero missing. No new inference,
linguistic improvement, independent bilingual acceptance or native acceptance
was executed by recording this decision. The two lexical REVIEW cases remain
unresolved diagnostics, not an instruction to modify the engine.

Continue Meeting verification with these known development limitations visible.
Keep the model, prompt, decoding and current-source-only boundary unchanged; no
context saver, style/glossary injection, phrase patch, corrective model or grading
override is introduced. Automated promotion guards and foundation fidelity goals
remain intact. This is owner disposition for continued development, not a passing
quality receipt or release authorization.

### Meaning-first quality verdict calibration — 2026-10-26

Parent Local `87b5e3e870750d71867d90835f169ddde56793eb`. Owner clarified that
clear equivalent meaning, rather than reference similarity or preferred grammar,
owns acceptance. The existing evaluator now reports `quality_status` and grouped
`quality_counts`: absent lexical/order cues require REVIEW; empty output,
explicit contrary-pattern hits or missing declared strict identity literals FAIL.
PASS is limited to declared checks. Missing/provenance-incomplete evidence remains
UNVERIFIED. N-gram similarity is not a verdict input. Existing raw critical-check
fields remain conservative regression signals, not confirmed semantic-error counts.
Direct worker summaries preserve REVIEW as distinct and nonzero-exit; transport,
completion/EOS/model identity and fast-promotion guards are unchanged.
Full comparison now also refuses promotion with any unresolved candidate REVIEW
or detected FAIL, even if baseline and candidate have identical flags; source
controls verify this independently of new-regression detection.

Eleven existing contracts accept clear target-language alternatives and improve
mixed-meaning protection. Hantarkan/tetap buka, branch/cabang with silakan,
hold off, get back and untuk saat ini pass; changed action, quoted filename,
approval/signature, defer/cancel, upper bound and added construction purpose remain
detected. Required/diperlukan alone cannot accept a lost Meeting-sound concept;
that actual wording remains REVIEW rather than a claimed proved mistranslation.
Fixture aliases are not emitted to the model. No corpus case, source text,
direction, fast membership, reference or frozen held-out text was changed.

Executed source evidence: evaluator self-tests passed 191 regression/40 held-out;
39 audio/translation source-contract tests passed. Faithful controls with
similarity below 1 pass, contrary and strict-literal controls fail, an ambiguous
complete worker run reports REVIEW, and REVIEW cannot exit successfully. Three
actual CLI synthetic controls produced PASS/exit 0, REVIEW/exit 1 and FAIL/exit 1.
Runner/test Ruff pass; evaluator retains the parent's two TRY004 recommendations.

Explicit offline regrading preserves original `81a16a50` captured model identity
and receipt: the same 28 outputs yield 20 PASS, 2 REVIEW, 6 detected pattern/literal
FAIL, zero missing. Five faithful variants recover from lexical false alarms.
The two review cases are `en-id-omit-011` and `id-en-informal-workplace-005`.
Historical 191-output regrading yields 137/48/6 respectively; these are declared
decision classes, not measured true translation accuracy. All 191 emitted worker
requests and selected requests were proven unchanged; no inference was rerun.
Original runtime receipts were not rewritten or relabeled as new inference.

| Private evidence | SHA-256 |
|---|---|
| `.workspace/reports/meaning-first-final-87b5e3e.json` | `88ff36993860efe9a8b744fb074bdc1947116d7986f1b54591ab606913be29d7` |
| `.workspace/reports/meaning-first-cli-87b5e3e/receipt.json` | `59e685008164c58ba3dd4ac9180a11d233da1afb2f177d8696dd26ca7c348a59` |

Source log: `.workspace/reports/meaning-first-source-final.log`. New full/fast
fingerprints are `9f4508a0b24e2f0555ff2ad00c53f02f409f3390653af15d636020e10569bd67`
and `84282fbde4df68e5c3fde5801e8020354b4929c22887a00d2a5a595d4b35191f`.
Proof ceiling: **EXECUTED_SOURCE / historical-output regrading**. No production,
model, dependency, audio algorithm, fresh runtime or native/perceptual proof changed.
Unexpected paraphrases still need review; these literal checks are not a general
semantic oracle, and previous model-error counts must not be inferred from them.

### Lean testing-system completion audit — 2026-10-26

Executable source: clean Local `81a16a50d8ccfd4c51453e3ba69109081b1c4a7b`.
This completes the requested lean test-system delivery, not acceptance of MiLMMT
quality or the native product. Four exact-source validators passed, including
38 contract tests and 191/40 translation evaluator controls. The first real
Unicode failure drove a proved transport correction, not a changed fixture/model.

| Requirement | Observed evidence and limits |
|---|---|
| Efficient risk-based checks | Existing corpus selects 28/191 cases with reasons and 16 declared risk tags checked per direction. No full/held-out cases deleted. Reference-control grading medians 3.97/22.78 ms; fresh fast worker wall 15,781 ms, not an inference speedup versus an unexecuted full run. |
| Effective quality calibration | Controls accept faithful alternatives and reject reversed day/number roles, changed monetary values/signs and separate positive forbidden instructions. Actual fast run completed 28 canonical CUDA/BF16 responses: 15 passes, 13 review flags, zero unverified; worker 22712 terminated. Failures are retained, not tuned away. |
| Modular, maintainable, lean | Existing runner/transport, manifest and evaluators are reused; corpus owns membership/reasons and semantic expectations. Changed paths since the goal are test tooling/docs only; production source and model/dependencies unchanged. Gate selection and maintenance policy remain in existing READMEs. |
| Strict evidence/admission | Nine envelope controls, stale-source prevention, incomplete-report rejection, immutable output and process cleanup are protected. Real disposable-child code-point test defeats symmetric encoding corruption. Actual stale fixture-manifest attempt was refused before pipeline inference. Fast comparison cannot authorize full promotion. |
| Integrated fixture path | Fresh current-source Rust VAD/writer processed 48 hash-bound inputs; downstream core acceptance covers its selected 13 scenarios × 2 rounds, all 26 PASS, worker 5184 terminated. Selected stages executed 22 ASR, 22 translation, 20 TTS and 20 spoken-content ASR calls. This is not all 48 downstream scenarios, device/native delivery or perceptual naturalness. |
| Reusable evidence | Actual CLI offline regrading matched the fast report's 13 flags and the core report's complete checks/counts/verdict. Audit verified all original 191 emitted worker requests, frozen held-out bytes and production source unchanged. No repeated inference was needed for regrading. |

The core probe initially used an old fixture manifest (same audio bytes, obsolete
scenario fingerprint) and was correctly rejected; corrected invocation used the
existing `.workspace/reports/neural-fixtures-m1-silero-v1` manifest matching
`a7e191bb...`. Neither source expectations nor WAVs were regenerated. Final core
report/sidecar bind exact source, VAD, segment hashes, model/actor identity and
scope. Fresh worker timing p95: ASR 403 ms, translation 308 ms, TTS 528 ms,
spoken-content ASR 389 ms. These exclude native device/meeting delivery and do not
prove long-duration stability; a fresh endurance rerun was not justified by a
transport-encoding repair with no resource-lifecycle policy change.

| Private receipt | SHA-256 |
|---|---|
| `.workspace/reports/quality-system-source-81a16a50.json` | `bd81f553daa488962c432f94e976e2153d22728d275c4418ab108c1d4a40f71b` |
| `.workspace/reports/quality-system-fast-81a16a50.json` | `b02957ef802e169453ba9a981cc8f901b67b6199c5db08a49a2f87f9633831ba` |
| `.workspace/reports/quality-system-core-current-81a16a50.json` | `648a217685cb5292c2fe8c6e2b27815fc68ba6c7c1a1356abbb8b254c642ad6f` |
| `.workspace/reports/quality-system-core-current-81a16a50.json.artifacts/vad.json` | `9f90156afba0159f446b89d5124c90203690c8851be79a8456c166d1f767e0ae` |
| `.workspace/reports/quality-system-acceptance-81a16a50.json` | `823f472a33e2e736ce449415f65c48e76bb1e0ff7cbd19569ca6817e3a2f4d79` |

The audit binds the independently invoked `*-rescore.json` receipts as well.
Proof ceiling is **EXECUTED_SOURCE / INTEGRATION_FIXTURE**. Current full-model
and held-out inference were not repeated: no inference candidate changed.
Held-out bilingual adjudication, naturalness, installed/native/physical capture
and independent receiver remain **NOT EXECUTED** by this delivery. MiLMMT quality
remains **FAIL / OPEN**; thirteen flags are review signals, not thirteen confirmed
model errors. Completing the test system does not waive these product gates.

### Fresh worker execution exposed host-encoding defect — 2026-10-26

Clean `d7007e4b2b60319220ae30ac53e8e1f76ff5a2ce` passed all four existing
source-profile validators. The actual 28-case fast invocation produced eleven
complete outputs before the Unicode case returned
`translation:input_token_count_unavailable`; 8 checks passed, 4 failed (three
quality checks and one envelope failure), 16 remained unverified. Fixture wall
time was 10,171 ms, process 21288 was terminated and execution_complete is false.
This is a correctly rejected incomplete run, not current model-quality acceptance.

The first separating probe used the actual canonical interpreter with a disposable
stdin/stdout child, not model inference. Inherited cp1252 decoded the Unicode
source differently and introduced a surrogate; explicit UTF-8 preserved exact
source. A naive echo control initially passed because symmetric decode/encode
errors canceled on the wire. The persistent transport test now asserts Unicode
code points computed inside the child; it failed before repair and passed after
the existing Worker pinned PYTHONUTF8/PYTHONIOENCODING. This changes test transport
only. All 38 source-contract tests and runner/test Ruff pass; no production
provider or model code changed. A fresh run on the corrected clean source remains
required. The initial failed inference receipt is preserved unchanged.

Private receipts: `.workspace/reports/quality-system-source-d7007e4b.json`,
`.workspace/reports/quality-system-fast-d7007e4b.json`,
`.workspace/reports/quality-worker-unicode-d7007e4b.json`;
corrected source controls: `.workspace/reports/quality-worker-utf8-source.log`.
Proof is **EXECUTED_SOURCE**, real disposable-child transport, and an explicitly
incomplete model fixture. No native/perceptual or full linguistic PASS is implied.

### Direct fast/full worker source candidate — 2026-10-26

Parent Local `034be0df31d599c776fdf4ce3869d5085a8aa922`. Existing worker runner
now accepts `--translation-suite fast|full`; transport, manifest and grading are
reused, with no parallel worker framework or inference implementation. The source
candidate binds exact clean source before/after execution, corpus selection,
manifest hash, model/revision, directions, complete/EOS flags, nonempty output,
process ID and child termination. Invalid responses stop remaining inference.
Per-case transport/output violations are separated from declared quality checks;
the latter require linguistic review and are not automatically model-error claims.
Incomplete worker reports cannot be loaded as quality bundles; complete flagged
runs remain inspectable/comparable without changing their verdict.

Six new focused simulated-worker tests exercise both entire selections with one
worker, exact emitted requests, nine invalid-envelope variants, transport loss,
cleanup refusal, source mismatch before spawn, output immutability, CLI exit codes
and complete-but-linguistically-failed output. Together with existing audio
contracts, 37 source tests passed. Translation evaluator self-tests passed 191/40.
Runner and new test module pass Ruff; the evaluator retains the parent's two
TRY004 recommendations, not introduced by this guard. Logs:
`.workspace/reports/quality-worker-source.log` and
`.workspace/reports/quality-worker-controls-final.log`.
Proof is **EXECUTED_SOURCE**, not real model or native execution. Fresh exact-SHA
source admission plus one fast worker run remains the explicit next proof step.

### Risk-based fast translation selection — 2026-10-26

Parent Local `a710ee3367a86f832cbc45d0dd0f181c2971b3b3`. The existing regression
corpus now owns an explicit 28-case selection (16 ID→EN, 12 EN→ID), reasons and
16 required common risk tags. Selection validates each tag in each direction,
known IDs and nonempty reasons. All 191 regression cases remain; frozen held-out
content is unchanged. Tags are not proof of complete semantic coverage or minimal
set size. The selection retains observed approval/signature, park/cancel,
previous-version reference and budget-purpose failures. Count-only self-test
thresholds were removed while semantic/category/coverage controls remain.

Existing evaluator CLI supports `--suite fast`; default full behavior remains.
Emitted requests and reports bind the selected fingerprint plus full-corpus
identity. Fast-for-full and full-for-fast results are incomplete, and even a
complete clean fast comparison cannot set the promotion-safe flag. New source
controls reject malformed selection, unknown IDs and uncovered risks, and verify
selected worker requests exactly equal their full counterparts. Evaluator and
ten audio-pipeline source tests pass. Seven actual UTF-8 CLI controls pass for
stats, emit, successful reference-control grading, missing output, mismatched
selection sizes and non-promotable fast comparison. Reference strings used for
these controls are explicitly labeled, not presented as model output. The first
CLI probe used Windows default output encoding and failed UTF-8 decoding; the
successful bounded rerun explicitly used Python `-X utf8`.

Seven in-process reference-control grading samples per selection gave median
3.97 ms fast versus 22.78 ms full. This measures evaluator work only, excluding
model/startup/transport/native latency. Regrading retained `723fc247` output
finds 13 flags in the fast subset and 59 in full; it still catches the named
historical failures. Different denominators are not a quality improvement.
No new model inference or fast worker execution occurred.

Private receipt `.workspace/reports/quality-fast-a710ee33-utf8/receipt.json`,
SHA-256 `561c57367d189f69ca97f1aee6d0b50aab0b0164f91c3f744a844918e247c5db`,
binds actual CLI reports/exit codes, grading samples, original result identity
and source hashes. Full fingerprint:
`8a6834f1f4937b0492fec6bbe45e9d4d86c7dac0c82b4ba836794bde3c6e240e`;
fast fingerprint:
`b86eaf0bfdb0ff4414baa2310f3cdd22ec471ce6e5e773f93f28ed808b24bc8c`.
Pipeline log: `.workspace/reports/quality-fast-pipeline.log`.
Proof ceiling: **EXECUTED_SOURCE** and historical-output analysis. Fast direct
worker execution remains unimplemented; overall linguistic/native/perceptual
quality is not accepted by selection or green controls.

### Numeric correction roles and declared formats — 2026-10-26

Parent Local `2eeaf46c9568fc89dfc5de760cc11f096413de37`. Executable controls
first reproduced a false pass for reversed budget values and, separately, for
incoming `lima puluh, bukan lima belas`. Existing cases now bind accepted/rejected
roles with existing ordered-cue alternatives. The budget case replaces redundant
exact-punctuation literal checks with explicit Rp dot/comma thousands forms in
those sequences; arbitrary number normalization is not introduced. Ordered cue
and output matching preserve grouping/decimal punctuation, signs and numeric
boundaries, including currency-prefixed amounts. Word-hyphen normalization stays
aligned for the existing `one hundred and twenty-five` audio case.

Executed source controls accept both declared currency formats and both faithful
clause orders, reject reversed roles, minus signs/words, extra digits and decimal
extensions, and protect incoming and hyphenated spoken-number roles. Regression
and held-out evaluator self-tests (191/40) and ten audio-pipeline source tests pass.
An initial retained-output audit exposed an unintended hyphen-normalization
regression in A11; it was corrected and protected before final regrading.

Final explicit regrading proves all 191 worker requests unchanged relative to
parent and original captured corpus. Only `id-en-number-001` changes grade: the
faithful English thousands format passes. Flags fall from 60 to 59 due to repaired
measurement, not better model output. Every other grade detail is unchanged.
Original model bundle is immutable and remains `723fc247` canonical greedy BF16;
there was no model execution, held-out content change or production change.
New contract fingerprint:
`83ef2acd612d6c818fe1dbf74239ad7c1651474042327213eb5d1d2b5e71aea1`.

Private receipt `.workspace/reports/quality-numbers-2eeaf46c.json`, SHA-256
`08bf07fff112a089efd0dbdfbdefa9f3e5d419d1c3dbb3ba164bbabdb574c460`,
binds old/new contract fingerprints, source hashes, original result hash and the
single changed grade. Pipeline source-test log:
`.workspace/reports/quality-numbers-pipeline.log`. Proof is **EXECUTED_SOURCE**
plus historical-output analysis; overall linguistic quality is still **FAIL / OPEN**.

### Case-local forbidden-phrase calibration — 2026-10-26

Parent Local `e62c2e4a0e336876017b5fb040addf6acc7f4323`. A faithful controlled
`Maybe hold this one for now; don't send it now.` failed before repair because
the forbidden cue matched inside its negation. The existing evaluator now accepts
optional case-local `forbidden_exceptions`: only explicitly declared complete
phrases are excluded from that cue's occurrence search. The disfluency case
declares four negative forms, including the typographic apostrophe variant;
other cases retain their original forbidden semantics.

Executed controls accept those four faithful forms, reject an additional positive
instruction before or after each, preserve forbidden `cloud` even inside a
negative sentence when no exception is declared, and reject unknown-cue, bare-cue
and malformed exception contracts. The 191-regression/40-held-out evaluator
self-test and ten audio-pipeline source tests pass. This is bounded literal
handling, not general understanding of negation, double negation, quotation or
semantic scope. No corpus case was added and no runtime dependency was introduced.

Retained-output regrading proves all 191 emitted requests unchanged relative to
both parent and the original baseline corpus. Only `id-en-disfluency-001` changes:
its false forbidden hit disappears; missing `yet`/`for now` remains. Before/after
flags are both 60. Other case details are identical. Original output/source
identity remains `723fc247` canonical greedy BF16; no fresh model inference or
candidate promotion follows. Frozen held-out content and production behavior
are unchanged. Corpus fingerprint:
`ebdd84197d1500bae818140668f0291a67755ecd676c6019ae85bbb4ba2d60a7`.

Private receipt `.workspace/reports/quality-negation-e62c2e4a.json`, SHA-256
`3bfc7898b01eb83c9ec1819627ae6e7969b4bce4a70025ebdbff57ebdb2bc95e`,
binds old/new fingerprints, source hashes, original bundle hash and changed
case details. Pipeline source-test log is
`.workspace/reports/quality-negation-pipeline.log`; simulated FAIL/UNVERIFIED
outputs within it are intentional negative CLI/admission controls, not real
model executions. Proof ceiling is **EXECUTED_SOURCE** plus historical-output
analysis; linguistic quality remains **FAIL / OPEN**, and native/perceptual
acceptance is not executed by this repair.

### Quality-test calibration, first bounded stage — 2026-10-26

Source parent: Local `b9308d229ebef03613cb75ac617b9e3507a0c273`.
The retained 191-case model bundle belongs to `723fc247` and is not fresh inference.
AI-assisted source/output desk review covers exactly its 60 flagged cases:
32 provisional checker mismatches, 17 provisional meaning errors and 11 unresolved
interpretations. Each entry records the original flag, source, output and reason.
These classifications are not independent bilingual acceptance, a complete review
of the 191 outputs, or a measured model error rate. Mixed errors were not accepted
merely because one missing cue was a valid synonym. No flagged-case contract was
relaxed in this stage.

An executable negative control failed before repair because `I mean Tuesday, not
Thursday.` passed the existing Thursday/not Tuesday correction case. That same
case now declares two valid ordered-cue sequences using existing evaluator support.
The self-test accepts both faithful clause orders and rejects both reversed
variants. No case was added, no evaluator runtime was duplicated and no model,
prompt, inference setting, audio, dependency or held-out content changed.

Executed source proof: translation evaluator self-tests passed for 191 regression
and 40 held-out cases; all ten `tools.tests.test_audio_pipeline` source tests passed
(their simulated admission/CLI controls are not model execution). Explicit
retained-output regrading verified all 191 emitted worker requests identical to
the parent corpus and every existing grade unchanged, with 60 flags still present.
The new corpus fingerprint is
`dbc40e730d270932bd5136824580f4db72e2fd1e646ec968ff51c3f094fb2401`;
the original model bundle/fingerprint was not rewritten or relabeled. Regrading
is contract-only evidence, not fresh inference or candidate-promotion evidence.

| Private evidence receipt | SHA-256 |
|---|---|
| `.workspace/reports/quality-triage-b9308d22.json` | `deaa4fd9096f7b63d35988725fc7340faf3dad521e2429ec22f5e7d33f74db83` |
| `.workspace/reports/quality-contract-regrade-b9308d22.json` | `6881f5d344b6a1afe78d77435865a0806c1ad35359a0a36fea3d50b2d2b23849` |

Proof ceiling: **EXECUTED_SOURCE** plus historical-output analysis. General
semantic quality, newly executed held-out inference, perceptual voice quality and
native/receiver acceptance remain unproved; the known MiLMMT quality gate remains
**FAIL / OPEN**. The triage/regrading procedure is maintained in the existing
`tools/translation_quality/HUMAN_EVALUATION.md`, not a new status system.

### Standalone Text application removal — 2026-10-26

Owner decision: retain MiLMMT and focus the application on Meeting voice
translation; direct text requests are an internal verification mechanism. The
replacement-model proposal below is superseded, not awaiting approval.

The repair removes Text navigation/header/page/hidden mounting, setup entry,
product readiness/status branch, frontend facade/API/cache/result policy, and the
registered native `translate_text` command plus its Rust module. Normal routes
are Meeting / My Voice / Settings. Meeting caption policies reject legacy Text
payloads. Schema 13 accepts older settings and omits retired Text direction keys
on ordinary save/clone; incoming Meeting language preferences and audio/setup
preferences remain. Existing private settings were not rewritten for proof.

Executed on the repair working tree: strict Svelte check zero errors/warnings,
source-contract validation, production frontend build, native developer build and all 407 frontend tests
passed. All 335 Rust tests passed with one opt-in replay ignored. All 75 affected
ASR/translation/worker tests passed; evaluator self-tests passed 191 regression
and 40 held-out cases. Removed positive Text UI/command tests are replaced by
retired-surface/internal-worker boundary assertions; lower counts are not a claim
of expanded coverage. The initial negative surface test was red before removal;
subsequent compile/test failures exposed stale sidebar copy, a controller argument,
validator input and a settings-only test fixture, all repaired in the final source.
The source-contract validator still checks parity of every remaining registered
native invocation; no blanket exclusion was introduced.

All 16 impact-tool tests passed using a temporary hash-recorded copy of current
tracked working-tree source without generated environments. Direct execution in
this installed workspace reached the existing 360-file source-graph capacity;
the parent has 352 tracked eligible source files, and ignored environment source
is also traversed by the current analyzer. This is an environment/tooling boundary,
not proof that dependency closure is complete. No analyzer capacity change was
made. The source-view copy is not shipped; automatic policy review rejected its
recursive deletion (`blocked by policy`) and it remains in the ignored
`.workspace/tmp/remove-text-impact-source-view` directory. Its source hashes are
retained in `.workspace/reports/remove-text-impact-view.json`.

Logs/JUnit remain under `.workspace/reports/remove-text-*`. Proof ceiling is
**EXECUTED_SOURCE**. Rendered native navigation, setup deferral, installed delivery
and full speech-to-Meeting behavior are **NOT EXECUTED** for this repair. No worker
inference source, model, voice, audio algorithm or dependency changed; existing
internal standalone/chunk/EOS tests remain. Native helper translation-probe
scheduling is retained as an internal mechanism, not a public Text action.

Semantic fidelity is still **FAIL / OPEN**: the earlier construction-purpose error
is reproducible on the raw provider path and is not cured by retiring the UI.
Remaining integrated native acceptance therefore stays stopped. The owner has
chosen further current-model fidelity work rather than an alternate-model download.

### Translation inference experiments and comparison repair — 2026-10-26

Runtime source: clean Local `723fc247` (the subsequent working-tree edits affect
quality tooling/docs only). Exact pinned MiLMMT weights, official source-only
prompt and CUDA device were used. No alternate model, dependency or application
inference change was installed/promoted.

| Experiment | Observed outcome | Decision |
|---|---|---|
| Greedy vs four-beam decoding, three minimized/original inputs each | Both retain unsupported construction purpose in the multi-sentence input | Reject as cure; no full beam corpus promotion |
| FP32 CUDA computation from the pinned weights, three inputs | Original construction purpose remains; `park this topic` still becomes cancellation. Contract approval improves in this one probe | Reject as cure; no general precision-quality claim |
| Sentence-only Text inference, all 190 unchanged regression requests plus original native case | Removes construction purpose but loses `previous version` reference in an existing case; only two corpus outputs changed. Both sets had 59 declared flags, mean character n-gram delta -0.0009 | Reject; incoming/raw Meeting policy was not changed or accepted |

The comparator had counted a regression only when a formerly passing case failed.
Consequently an already-failed case could lose a further meaning cue and still
leave `promotion_safe_on_declared_critical_invariants=true`. A self-test using the
actual previous-version baseline/candidate strings failed before repair. The
canonical comparator now compares new missing literal/concept entries, forbidden
hits and ordering failures in every case. It adds `critical_regression_details`
without a second grading owner. Regrading the same fingerprint-bound 190-case
real outputs now identifies `id-en-informal-workplace-005` losing `previous` and
the actual comparison CLI exits 1. Improving another defect cannot cancel that
new failure. This is executed comparison behavior over real model receipts, not
a universal semantic oracle.

The touched evaluator's unused `sys` import is removed. Ruff leaves one
pre-existing TRY004 recommendation in the legacy result-bundle type-error path;
its exception contract was not changed as part of fidelity work.

The regression corpus appends `en-id-budget-purpose-001` with the actual native
three-sentence source, faithful reference and forbidden unsupported construction/
project purpose. Existing case content and the frozen held-out corpus remain
unchanged. Fresh canonical worker execution completed all 191 requests; the new
case correctly fails with `forbidden_hits=["pembangun"]`. There are 60 declared
flags, including known narrow-matcher false positives; do not equate them with
60 reviewed translation errors. Evaluator self-tests passed for 191 regression
and 40 held-out cases; held-out **model inference was not executed**, because no
candidate was accepted. Repository validation and corpus integrity passed.
The owned fixture worker was intentionally terminated by the existing runner;
this does not prove native Stop/Close or audio delivery.

Proof ceiling: **INTEGRATION_FIXTURE** for local model experiments/regrading,
**EXECUTED_SOURCE** for the comparison/corpus repair. **Semantic fidelity remains
FAIL / OPEN.** No runtime source was replaced by rejected experiments, and no
throwaway production code/dependency/cache was introduced for them. Reports and
the frozen old corpus copy are private evidence under
`.workspace/reports/translation-fidelity-723fc247/`:

| Receipt | SHA-256 |
|---|---|
| `beam-probe.json` | `c8b9664fbda1e55bebd05b72eaf74dc85bb33d05bb04ce164d0ae28c21f478d3` |
| `fp32-probe.json` | `78850c2a8f396b3a04e1db3ba0b0501ac446b894ee4fe8b9dfe863be28a1aa0f` |
| `sentence-regression.json` | `7bfa87fba34f636182b45d5536cf28b62f0a44201ad7dba6897c6cb7a32678f6` |
| `sentence-comparison-fixed-gate.json` | `b5acf12510d130348276a5a1e3aed6e21bad84c39055343d6e3c29df5faeb3c4` |
| `baseline-191.json` | `9a58ad074148c6650f5811960b6adfed4cec2688fb98e55a01ae272632ca6488` |
| `baseline-191-score.json` | `126da83508e8b7585fb5a63b2f429361a730832a861c5202f07b9a30025d6a57` |

A model-replacement evaluation is proposed, not accepted or executed. Official
[Tencent Hy-MT2-1.8B model card](https://huggingface.co/tencent/Hy-MT2-1.8B)
lists Indonesian/English, Apache-2.0 and recommends Transformers >=5.6.0;
[official files](https://huggingface.co/tencent/Hy-MT2-1.8B/tree/main) list a
4.08 GB weight file. Current installed Transformers 4.57.6 contains the named
Hunyuan architecture mapping, and the upstream config itself names 4.57.6;
that static evidence does not settle runtime compatibility. No weight download,
experimental dependency installation, replacement, quality or coexistence claim
follows before the pending owner choice and actual local comparison.

### Retired context and hint removal — 2026-10-26

The current Local repair removes the dormant `translation_style`, `terminology`
and `spoken_terms` settings/DTO/default/sanitizer surface and unused Rust ASR
hotword builder. Schema 12 accepts legacy JSON, retains active preferences and
omits retired keys on ordinary save; frontend cloning projects the active shape
rather than retaining unknown top-level keys. The worker's ASR `hotwords` path
was still callable even though production Meeting sent none: it is now removed,
along with its response capability flag. Initial-prompt/prefix payloads remain
ignored and previous-text conditioning stays false. Actual UserData was not
read or rewritten for migration testing.

Production Text, outbound and incoming payloads and the canonical prompt were
inspected: only the current source and language direction provide translation
content. Transcript/export retention, session/timing metadata and neighboring
sentences within the current Text input are not prior-turn memory. No automatic
rewrite/alternative-generation pass was found. Negative legacy-input tests and
immutable historical corpus metadata deliberately retain forbidden terms; they
are not active features. The local-AI skill and its routing/procedure examples,
skill map and activation matrix no longer instruct last-three-pairs injection.

Executed on this repair working tree: strict Svelte check (zero errors/warnings),
418 frontend tests and source-contract validators passed; 75 affected ASR,
translation and worker tests passed, including malicious legacy ASR hints;
six Rust settings tests passed, including legacy-file load/save and active
preference preservation; 15 agent-corpus/scorer tests and repository verification
passed. Removing obsolete positive feature tests explains lower total test
counts than the earlier complete-suite receipt. Initial red controls failed on
retained defaults/cloning and active ASR hints. A pytest temp-root permission
error and an absent private report/temp parent were execution-setup failures;
using fresh workspace temp paths resolved them. The final affected worker run
has no failures/errors/skips. This is **EXECUTED_SOURCE**, not a new full-suite
or installed-app acceptance claim.

Twelve real pinned local MiLMMT requests form six matched request pairs: both
language directions with standalone Text, outgoing and incoming lane markers.
Each pair produced identical output with and without legacy context, style,
glossary, spoken-term, alternative, system-prompt and conversation-history keys.
The private receipt `.workspace/reports/context-removal-model.json`, SHA-256
`0ca85f1ae6cf1515a39ee350dca554407b57cff8203aebba850ffb0a79769d39`, binds the
base SHA, modified source-file hashes, complete requests/responses and outcomes.
The owned diagnostic worker was intentionally terminated by the existing runner
(exit code 1 on Windows); this is not native application Stop/Close proof.
This is **INTEGRATION_FIXTURE** context-isolation proof only.

Semantic fidelity remains **FAIL / OPEN**. On clean base
`ec5c1df3716eba359535525e68b5b948b846f00b`, the original EN→ID addition was
reproduced with the exact standalone request and raw provider path; the original
input occupies one chunk and repeated deterministic inference returns the same
construction-purpose addition without history. The repair's matched-pair model
probe retains that error. A real-model 190-case baseline was also retained in
`.workspace/reports/translation-fidelity-ec5c1df3/`; all responses completed, but
59 declared-invariant flags include false positives such as numeric punctuation
and valid synonymous wording. Those flags are not adjudicated mistranslation
counts. No candidate decoding/model/prompt change or real-model held-out run
has been accepted. Removing external hints cannot guarantee that a generative
model never invents meaning; quality work and the remaining integrated native
flows stay open.

### Integrated developer acceptance — stopped 2026-10-26

Pinned clean candidate: `8582fc2a34f9bc277d792ad39471c1cd6e8777da`, branch
`Local`. The owner authorized the grouped acceptance phase and approved the
dedicated Windows UAC worker-test child. No cloud inference, CI, release build
or private-data restoration occurred; npm used the existing frozen lockfile.

| Gate / flow | Observed evidence | Limit |
|---|---|---|
| Source admission | Repository verification, npm ci, source contracts, strict Svelte check and 421 frontend tests passed | Source evidence only |
| Worker suite | 326 passed; 2 POSIX process-group cases skipped; zero failures/errors | Elevated child exit 0 and JUnit XML retained; skips are not PASS |
| Evaluator self-tests | Translation 190 regression + 40 held-out; ASR 60; TTS 30 passed | Evaluator behavior, not general model quality |
| Rust / canonical build | 344 Rust tests passed, 1 replay ignored; canonical frontend rebuild and locked Cargo check passed with externally pinned expected SHA | No installed-runtime proof |
| Native launch / Text | Real Tauri developer app launched and Text navigation worked. Three-sentence ID→EN output preserved negation, time and amount; command duration 5,715 ms. Copy acknowledged success; Swap preserved both panes and disabled Copy until a current result | One public sample; clipboard contents not independently read; pending-edit races and Clear not tested |
| EN→ID fidelity | **FAIL**: successful native command (727 ms) translated `The budget is fifteen million rupiah.` within the three-sentence input as `Anggaran pembangunannya adalah lima belas juta rupiah.` | Construction purpose is absent from the source. PR-041 failed despite complete output and visible review hints; internal request/provider/model cause remains unseparated |
| Cleanup | Native Close exited the idle TranslateIT process and the owned frontend listener | Not Meeting Stop, orphan-writer recovery or active audio teardown |
| Remaining native acceptance | **NOT EXECUTED** after the first semantic failure | Microphone capture, Meeting Start/output, independent receiver, Mute/source switching, repeated sessions, transcript/export and installed app remain outstanding |

Control initially encountered stale window identities and competing user input;
the owner yielded control and native UI observation then proceeded. This is not
classified as an application defect. Acceptance stopped at the semantic failure;
no inference retry, runtime edit, model change or wording replacement followed.

Private receipt `.workspace/reports/acceptance-8582fc2a/summary.json` has SHA-256
`366bf374ac723e9ace8391b47336c2f10d8c0ab8d3811d6af22edd9f744febef`.
It retains the public inputs/outputs, exact source, gate counts and source-log
hashes. Matching JUnit, build, evaluator and native trace files remain in that
directory. The native failure is **LIVE_RUNTIME** Text evidence; the source
gates are **EXECUTED_SOURCE**, and neither proves Meeting or installed delivery.

### Native Stop/Close release gate — bounded repair 2026-10-26

`commands/application_runtime/shutdown.rs` now checks successful Meeting Stop,
then fresh runtime ownership and the existing Meeting cleanup-pending fact
before helper shutdown and recovery-marker removal. Targeted offline Cargo
execution passed all four shutdown tests; controlled callbacks verify the
orphan-cleanup → verified-release transition, retained/unavailable ownership,
helper failure, marker failure and shutdown order. These tests do not terminate
a real worker, operate native windows or reproduce native audio teardown.
The source wiring/close policy Node tests passed 12/12, ApplicationRuntime
architecture and repository validation passed, and the real frontend production
build passed. Current continuation preserves the canonical branch/execution
boundaries; the earlier missing planning-text markers are reconciled.
The initial Cargo attempt stopped because cleanup had removed `dist`; the
ordinary frontend build regenerated it before successful test execution.
No dependencies, models, audio algorithms, updater or installed runtime changed.

Proof ceiling: **EXECUTED_SOURCE**, on the bounded Local working-tree repair;
actual Close/Stop/Retry Stop with devices, independent receiver delivery and
the integrated current-candidate build/acceptance gate remain **NOT EXECUTED**.
The earlier regression receipts below keep their own exact source and scope.

### Maintained automatic regression system — accepted 2026-10-26

The requested modular, maintainable regression-system delivery is complete for the agreed vital package, with executable proof on clean `0d571fb472741df02179cdb739ea5379070ec6ef`. This closes the test-system goal, not full-product/native acceptance. `worker_runner.py --profile source|core|endurance` is the single maintained entry point; `scenarios.json` owns independent package/round policy, `harness.py` fixture identity, `worker_runner.py` execution/transport, and `pipeline_checks.py` the decision path shared with offline `harness.py score`. Canonical ASR/translation/TTS evaluators retain their semantic owners. No new runtime engine, dependency, cloud service, CI or parallel test framework was introduced.

Completion was audited against the original delivery criteria:

| Requirement | Authoritative evidence |
|---|---|
| One repeatable core command | Actual CLI source/core/endurance runs all exited 0. Source ran four validators; core ran fresh same-source VAD and 13 scenarios × 2 rounds, all 26 PASS. |
| Stable scenarios with complete mandatory coverage | Profile membership is a fixed 13-case data contract; ten speech contracts cover all speech members. Profiles cannot accept custom IDs/rounds. Unit proof changes execution metadata without changing acoustic identity. Original immutable fixtures were reused; named profiles regenerate only the canonical VAD exports at the current SHA. |
| Meaningful positive/negative gates | 31 audio contract/fault/pipeline tests plus four subtests passed. Actual offline score CLI controls accepted fifteen/15 equivalence and rejected reversed numbers, missing negation, missing EOS, wrong actor and 125th ordinal substitution; missing segment evidence stayed UNVERIFIED. PASS exited 0; FAIL/UNVERIFIED exited 2. Controls are synthetic receipt mutations, not fabricated model failures. |
| Actionable reports and one grading decision | Execution and offline regrading produced identical checks/counts/verdicts for core and endurance. Non-PASS checks carry first wrong stage, expected/actual and JSON evidence pointers. Real temporary-WAV orchestration tests proved hash-bound failed-audio retention and successful-output cleanup, using a simulated worker rather than claiming model fault injection. |
| Modular maintenance without replacing scenarios | Execution and checks are separate modules; protocol adapters preserve full returned text and reject nontext data. Canonical TTS grading consumes explicitly declared numeric aliases from existing contracts, while signs/ordinals remain distinct. README maintenance boundaries and source ownership identify which owner changes for protocol, semantic, fixture or execution-policy changes. |

The final `source` profile passed existing audio checks plus 60-case ASR, 190-regression/40-held-out translation and 30-case TTS evaluator self-tests. New checker/runner/pipeline/VAD test modules passed Ruff. Existing harness and canonical quality-evaluator lint findings remained unchanged against baseline; this is not an all-repository lint claim. An earlier completion probe exposed false UNVERIFIED for a declared fifteen/15 variant; final canonical TTS normalization now accepts that variant without changing audio, scenario text or corpus expectations. Another completion correction made offline score failures return nonzero, matching execution rather than merely printing a failed JSON verdict.

Fresh core and endurance VAD replays each passed all 48 VAD expectations at the exact executable source. Downstream scope remains the selected 13-case package. Core executed 22 input-ASR, 22 translation, 20 TTS and 20 spoken-content ASR calls; six non-speech observations invoked none of those stages. Endurance executed 220 input-ASR, 220 translation, 200 TTS and 200 content-ASR calls over 20 rounds, plus 60 suppressed non-speech observations; all 260 scenario/round checks passed. One worker per profile remained alive with ASR, both translation directions and selected MaleVoice on CUDA; cleanup confirmed child termination. Endurance allocator observations ranged 2,663–2,666 MiB allocated and 2,812–2,814 MiB reserved. Worker timing p95 was 382 ms input ASR, 317 ms translation, 532 ms TTS and 385 ms content ASR. These are bounded worker samples, not native latency, device-accounting ownership, naturalness or proof of no long-term leak.

Private exact receipts:

| Receipt | SHA-256 |
|---|---|
| `.workspace/reports/regression-source-final.json` | `3b80653c63722c0e3bc8cfc7fdde54dc602946f912eee292243411ec56b10996` |
| `.workspace/reports/regression-core-final.json` | `1bb6904d756e0c19685d366e1c0c7aa278ac2e3c62a0f7657dfbf2262e2f70b9` |
| `.workspace/reports/regression-endurance-final.json` | `d12abbf79cdf8d8ebd8a8daf0d333f2d767fe9e3c8a90cacf8d97a86fb5fbd56` |
| `.workspace/reports/regression-final-controls.json` | `913575cb1f931c8a820f1c47e059c942954a17adda50f2eb27219416975e03e0` |

Core/endurance sidecar `.json.artifacts/` directories retain actual current-source VAD receipts and hash-verified finalized WAVs. Corresponding `*-rescore.json` files contain the independently invoked offline decisions; the control receipt binds each explicit mutation to the original core report. All originals remained unchanged. V2 pipeline receipts bind expected segment coverage/order/hashes, profile/semantic identities and complete source admission; older V1 receipts stay historical rather than being silently upgraded. The immutable acoustic scenario/audio identity is unchanged by execution-profile metadata. Scope remains `EXECUTED_SOURCE` and `INTEGRATION_FIXTURE`: all 48 downstream cases, the remaining planned fault injectors, perceptual naturalness, FemaleVoice, adaptive pace, Meeting context/native Stop, installed app, physical capture and independent receiver are not accepted by this delivery. Documentation-only continuation commits do not create fresh runtime proof. No further test work is automatically scheduled for this completed goal.

Use the latest completed matching verifier for the exact `Local` SHA and changed domain. Ancestor proof applies only to unchanged domains. Queued/running/skipped/cancelled/unrelated jobs are not PASS.

Do not preserve mutable “latest PASS” snapshots here. Historic GitHub Actions runs are not proof of the current `Local` commit.

The Batch 2B.1/2B.2 PCM handoff and canonical-VAD replay adapter retain a **test-only/no production cutover** boundary. The manual full Windows source CI can compile and execute the Rust unit suite, but targeted push CI does not; neither proves that the experimental consumer is wired into live capture or reduces callback latency; real-device performance remains TARGET_WINDOWS-only.

## Local neural fixture preview — 2026-10-26

After confirming the physical microphone issue, the owner authorized a separate local neural generator for public test WAVs. Supertonic 3 archive revision `aafc6e32416a594460b32413efc49d7fe4ce6d46` and both M1/F1 preset assets were downloaded locally; ten pinned model/config/voice/license files passed size and SHA-256 checks. The separate SDK 1.3.1 wheel passed its published SHA-256 check. Developer packages were installed in an isolated workspace environment; application lockfiles, models, selected Meeting voice and personal data were unchanged. Generation used CPU, offline Hub flags and disabled automatic model downloads.

**Executed fixture evidence:** ten Indonesian public utterances (A01/A03/A04/A06/A08 × M1/F1) were synthesized with 16 steps, speed 1.0 and seed 42. Each has a preserved 44.1-kHz PCM16 original and an anti-aliased 16-kHz PCM16 copy. Generation rejected silence/nonfinite/clipping; the completed preview passed these numeric guards. Three admission/download regression tests and ten existing fixture/generator tests passed; changed Python Ruff checks passed. These are source/fixture results, not ASR, translation, selected-voice or Meeting acceptance.

**Owner voice-profile acceptance — 2026-10-26:** the owner accepted the M1/F1 sound after receiving both A04 examples and authorized continuation. This is acceptance of the sound of those profiles, not evidence that all ten utterances were listened to or pronounced correctly. The immutable `comparison.json` retains its generation-time PENDING field and exact generator/pin-file hashes, model revision, package versions, synthesis settings, text and WAV hashes. The preview itself is not the canonical 48-case fixture set. Its success does not prove the missing VAD-segment→ASR connection, semantic translation correctness, receiver delivery or physical microphone behavior. Upstream Supertonic is archived; it is developer-only, not a product dependency.

Private local outputs: `.workspace/reports/neural-preview-id-v2/` (`listen.html`, `comparison.json`, original and converted WAVs, model license), its ZIP, and matching generation/environment/download logs. No audio/model bytes are uploaded to GitHub. The canonical harness now accepts the verified neural model directory and M1/F1 choice, reusing the preview synthesis and existing acoustic transformations. Twenty-one relevant fixture/admission/VAD-receipt source tests passed, and the shared neural module/changed tests passed Ruff. Full harness Ruff has pre-existing findings; no claim of a full clean Ruff result is made.

**Canonical generation/inspection executed:** `.workspace/reports/neural-fixtures-m1-v2/` contains 48 M1 fixtures, 271.606 seconds total (including the existing 61-second stress tone), fingerprint `31fb53ed1a82db4e63cf74a5e00a386414d40344f37c04045de1e09163a30981`. Every WAV/reference/metric and both generator-source hashes matched. The first full generate→inspect execution exposed a harness bug: valid scenario IDs were compared as a set against a dict and always rejected. The owning comparison now uses the dict's key set, with a complete 48-case publication/inspection and tamper-rejection regression. This repairs fixture admission only; no VAD thresholds, model identities or application behavior changed. Canonical Rust replay and AI evidence remain required before any stage acceptance.

**Canonical Rust replay executed on clean `b02738cc5c03a677c1ab40a8a7db208b6b561fc1`:** `vad_runner.py` explicitly ran the ignored incoming-finalizer/PCM16-writer test with locked offline Cargo and emitted 48 hash-bound receipts. Forty-five met the current VAD expectations; A14/A15/A34 did not. Successful Cargo exit means the replay executed, not that every scenario passed. The run remains an offline incoming-lane fixture, not production outbound capture or Meeting acceptance.

| Case | Observed stage evidence | Interpretation / missing separating evidence |
|---|---|---|
| A14 · 2.1-second seeded noise | One finalized segment; one valid WAV; no overflow/eviction | First failing expectation requires zero VAD finalizations. Energy/quality gating can admit this waveform, but no ASR ran, so false recognized speech or TTS is unproven. Keep this failure; the next separating proof is the actual emitted segment reaching canonical ASR and downstream suppression. Do not tune VAD solely to satisfy this fixture. |
| A15 · 61-second uninterrupted tone | Zero finalized segments; zero observed overflow-drop count; no added quiet tail | Current source increments the overflow-drop counter when a closing boundary resolves the utterance. The fixture deliberately supplies no closing tail, so its demand for an already-incremented counter needs owner review. This neither proves nor disproves eventual overflow handling or long natural-speech quality. |
| A34 · speech at −24 dB | Zero finalized segments; no writer attempt | Low-level speech was not finalized in this replay. No ASR ran; audibility and actual gate evidence still need separation before any threshold change or claimed missed human speech. |

The AI lane stopped at this gate. No finalized segment was passed to ASR; the existing Rust replay deletes its temporary emitted WAVs. A connected fixture needs an explicit test-only segment handoff with hashes, not a direct-worker run on the original WAV or merged stage reports. Private evidence is `.workspace/reports/neural-fixtures-m1-v2-vad.json` and the matching `.log`. No runtime code or scenario expectation was weakened. The full application remains unaccepted.

**Source handoff prepared after explicit continuation:** the existing ignored Rust replay can now opt into exporting its actual finalized WAVs and receipts; `vad_runner.py --segments-dir` verifies/publishes their complete hashes/PCM identities. The existing worker runner accepts matching VAD observations plus that explicit directory for an ASR-only diagnostic. It stages a byte-identical finalized segment in the worker's allowed CacheData root, checks hashes before/after consumption, retains the original exports and records the real ASR response/model/device. A01 is the positive control and A14 is the first noise diagnostic. Empty-transcript rejection is distinct from model/transport failure; no original-WAV substitution or fake downstream suppression is accepted. Twenty-three relevant source tests, changed Python Ruff and locked offline Rust test-binary compilation passed. This preparation alone does not claim inference; execution is recorded below.

**Connected diagnostic executed on clean `7c0fbf85eda2ae2d5cc47aeb25252e150fe5c676`:** the real Rust replay exported 57 canonical PCM16 segments across the 48 fixtures, with segment fingerprint `7105ed8c946d39a3cc8f673a35f1e74e270dc9123d8466f946493ecca82731f6`. Its 45/3 VAD expectation outcomes were unchanged. Exported A01/A14 WAVs were staged into the canonical worker's allowed CacheData root; input hashes matched the Rust receipts before and after consumption. Source/scenario/audio/export identities remained matched, and original exports were retained after the owned worker and temporary staging were closed.

**ASR acceptance failed before recognition:** worker status and ASR preload returned success, but both A01 and A14 returned `asr:runtime_failed:TypeError`, with no model/device inference result. A01 was correctly classified `SPEECH_ASR_FAILED`; A14 remained `UNVERIFIED_ASR_FAILURE`, never safe noise. One bounded follow-up on the same finalized A01 input captured the exact message: `open() got an unexpected keyword argument 'metadata_errors'`.

**First wrong owner — worker dependency compatibility:** the locked/installed PyAV is `19.0.2`; faster-whisper is `1.2.1` and declares only `av>=11`. Its installed `faster_whisper/audio.py` calls `av.open(input_file, mode="r", metadata_errors="ignore")`. The installed PyAV rejects that keyword. ASR preload/import success did not exercise WAV decoding, so the dependency source upgrade is not functionally accepted. This is not a VAD threshold, waveform-quality, CUDA-inference or noise-recognition diagnosis. Do not patch installed site-packages, substitute a decoder/model or count the empty failed result as successful suppression.

The audio-handoff partition stops here for an evidence-bounded ASR/dependency-owner repair. Required next proof: a compatible locked dependency boundary, successful decode of the exact finalized WAV, a positive A01 recognition result, then the same A14 noise diagnostic. A15/A34 and translation/TTS/production suppression/receiver acceptance remain unexecuted or unresolved. Private evidence: `.workspace/reports/finalized-vad-7c0fbf85.json`, `finalized-segments-7c0fbf85/`, `finalized-asr-7c0fbf85.json`, `finalized-asr-typeerror-7c0fbf85.json` and their logs. No audio, model bytes or raw private paths were committed.

**Authorized decoder dependency repair — source/decode proof:** `pyproject.toml` now constrains `av>=11,<19` while stable faster-whisper 1.2.1 still sends the removed keyword. Regenerated `uv.lock` and frozen sync changed only PyAV, from 19.0.2 to 18.1.0. No installed wheel, decoder adapter, ASR/model identity or other dependency was patched. A new real faster-whisper/PyAV decode test failed on both mono 16-kHz and stereo 48-kHz public PCM with 19.0.2, then both passed with 18.1.0. The targeted ASR decode/alignment/hotword tests and changed-test Ruff passed. This proves WAV decode, not recognition.

The finalized diagnostic also uses canonical ASR WER and critical-word scoring, emits an overall PASS/FAIL/UNVERIFIED verdict and exits nonzero on anything other than PASS. Missing ASR/control evidence and real failure cannot become a successful command just because JSON was written. Twenty-four harness source tests passed, including lost-negation rejection despite low WER and no false safe-noise classification on ASR failure. Bounded actual ASR error notes are retained. Clean-pinned connected inference must still establish whether A01/A14 pass after the decoder repair.

**Repair accepted at the decoder/positive-ASR boundary — clean `40d2c4e7ca4cf40d40e20cd72679b3aa585f5039`:** the unchanged 48-fixture set was replayed/exported, and A01/A14 consumed the same finalized WAV hashes as the failing run. A01 returned exactly its public reference, `Tolong mulai presentasinya sekarang.`, with WER 0 and all canonical critical checks passing. Actual runtime reported `faster-whisper-large-v3-turbo`, CUDA, `int8_float16`; command elapsed time was 478 ms. Thus the earlier PyAV keyword failure is resolved for actual decode and positive recognition, not merely imports/preload. This one elapsed observation is not a Meeting latency benchmark.

**New first runtime-quality failure retained:** A14's pure synthetic noise returned `Terima kasih.` (266 ms), classified `NON_SPEECH_TRANSCRIPT`. Its ASR input SHA-256 `0e25c4c600989bf0a2fed1dd50817c7e533a7a3819d8fa260ceb6cf284d82243` still matched the actual Rust-finalized segment; no original-WAV substitution occurred. The positive control proves ASR was working, so this is an observed false recognition for this specific noise fixture, not safe noise rejection or a generic decoder failure. Translation/TTS/Meeting were not invoked; false spoken delivery is not claimed.

The diagnostic correctly emitted overall **FAIL**, counts PASS=1/FAIL=1/UNVERIFIED=0 and exit code **2**. This is an executed machine-readable software regression gate, without a human microphone/listening step; it is not a full app acceptance pass. The acceptance lane stopped at the new noise-recognition failure. Do not blacklist a legitimate phrase or tune thresholds solely to make this one fixture pass; inspect the canonical ASR/no-speech evidence and separate runtime rejection from test assumptions first. A15/A34 and later translation/TTS/lifecycle/receiver cases remain unresolved. Private receipts/logs: `.workspace/reports/finalized-vad-40d2c4e7.json`, `finalized-segments-40d2c4e7/`, `finalized-asr-40d2c4e7.json`, `finalized-asr-40d2c4e7.log`, `asr-decode-before-repair.log` and `asr-repair-source-tests.log`.

**ASR confidence diagnosis — clean `e5aa9223876f05bb0e3b9d07fc293dca1a794d17`:** an offline, direct canonical-model diagnostic loaded the existing worker's `get_asr_runtime({})` on CUDA and consumed retained finalized WAVs with matching hashes. It used the handler's relevant deterministic options, including `beam_size=1`, `temperature=0`, `condition_on_previous_text=False`, `without_timestamps=True` and `vad_filter=False`. This is model-level diagnostic evidence, not another protocol/worker/Meeting acceptance run. Current production incoming/outbound callers also set `FINALIZED_MEETING_ASR_WORKER_VAD=false`; the diagnostic did not disable a production noise filter just for testing.

All 12 observed outputs across A01/A02/A03/A04/A06/A09/A10/A11/A12/A14 had `no_speech_prob=0.0`. A14 again yielded `Terima kasih.`, average log probability -0.20885696. A09's three retained segments yielded `Bisa dengar suara.`, `Terima kasih.` and `di las`, with averages -0.21720415, -0.23202820 and -1.49251302. Thus neither the observed no-speech field nor a simple average-log-probability cutoff distinguishes this noise from quiet speech without risking valid speech. This does not establish why the model reports zero, or a model-wide statistical guarantee. An [upstream report](https://github.com/SYSTRAN/faster-whisper/issues/1028) describes similar turbo-model probabilities; it is supporting context, not a diagnosis of our pinned artifact. Do not patch installed packages, blacklist a legitimate phrase, change model identity or tune an ASR threshold from these observations.

**Evidence-bounded audio handoff:** the existing A09 WAV segmentation and A14 admission need per-window canonical gate/finalizer evidence before repair. `engine/audio/vad.rs` currently gates RMS, peak, active ratio and clipping; these energy/quality checks do not themselves establish linguistic speech. This inspection does not prove that directly trusting the richer evidence reason would repair noise safely. A34's missing quiet utterance still needs audibility-versus-gate separation; A15's overflow count still needs the closing-boundary lifecycle checked. No audio behavior or fixture expectation changed. ASR diagnosis stops at this owner boundary under AGENTS.md; downstream translation/TTS/lifecycle/receiver remain unexecuted. Private diagnostic: `.workspace/reports/asr-no-speech-diagnostic-e5aa9223.json` and `.log`; original finalized WAVs/hashes remain intact. No new inference-quality acceptance is claimed.

**Canonical audio-owner source repair — candidate based on `c39aca0b`:** per-window gate/evidence receipts reproduced A09 cuts at 1.0 and 1.6 seconds: RMS fell just below 0.006 while the existing acoustic evidence did not classify those windows as silence. The finalizer had treated every non-clipping gate rejection as a quiet boundary. It now accumulates end-of-speech silence only for the existing `rejected_silence`, `rejected_low_energy` or `rejected_low_snr` evidence reasons, preserving other rejected windows inside an active bounded utterance without counting them as accepted speech or allowing them to start speech. The same distinction protects the closing-quiet overflow exemption. Start/final whole-utterance gates and thresholds remain unchanged.

A regression with strong→quiet→strong→silence failed before repair with `quality rejection must not invent an end of speech`; after repair it verifies one complete output with the quiet PCM retained byte-for-byte. Targeted Rust audio checks passed: 89 passed, 1 explicit fixture replay ignored. Explicit candidate replay then processed all 48 unchanged WAVs: 46 VAD expectations matched, A14/A34 failed. A09 became one 1,740-ms speech span instead of three (540/400/200 ms); positive A01 remained one 2,140-ms span. This is candidate source/integration-fixture evidence, not clean-pinned worker recognition or native device acceptance.

A15's fixture oracle was premature: the existing lifecycle reports an overflow drop only after the rejected overlong utterance reaches a closing quiet boundary. The replay now records `overflow_before_quiet=0`, appends a disclosed 1,000-ms quiet tail after the unchanged full 61-second tone, and requires exactly one final drop with no emitted utterance. That expectation passed. This repairs the test lifecycle, not production overflow behavior. Private candidate evidence: `.workspace/reports/audio-gates-before-c39aca0b.log`, `audio-gates-candidate-c39aca0b.log`, `audio-quiet-regression-before.log`, `audio-owner-repair-tests.log`. A14 admitted noise in every window; A34 never reached the start RMS gate. Those admission defects are not repaired by this segmentation change; no single-fixture threshold tuning or second detector was introduced.

**Clean-pinned repair validation — `26758e5039dfab76c92d57cc250a5f768e93bced`:** the existing VAD runner replayed all 48 unchanged public neural fixtures and exported verified actual writer output. VAD expectations matched for 46 cases; A14/A34 remained failures. Segment fingerprint: `b50cb2c5eb16bf6e76fdb2deb6855de3aaea1c684330b3d875636a88d329f717`. The existing worker runner consumed those exact exported files for A01/A02/A03/A04/A06/A09/A10/A11/A12/A14. A09 returned `Bisa dengar suara saya dengan jelas?`, WER 0, all critical checks passed, proving the repaired single segment reaches real ASR intact for this fixture. A01/A02/A03/A06/A12 also had WER 0; A10 passed with WER 1/7. This is actual local finalized-WAV→worker-ASR evidence, not production microphone/Meeting delivery.

The ten-case machine verdict correctly remains **FAIL**, PASS=7/FAIL=3/UNVERIFIED=0, exit 2. A14 again produced `Terima kasih.`. A04 returned `Maksud saya 15, bukan 50.` and A11 `Angka yang benar 125, bukan 150.`; the current canonical scorer nevertheless reports missing spelled-out number concepts and WER 0.5714/0.6364. These are observed digit-versus-word scoring discrepancies with negation/numeric values preserved in the outputs, not demonstrated loss of those meanings. The scorer was not loosened, its failures were retained, and its repair belongs to the ASR-quality owner rather than this audio boundary. Fifteen existing Python harness/receipt tests passed using the worker test environment; the general bundled Python has no pytest and provided no test proof. Documentation graph and whitespace checks passed.

Private matching receipts: `.workspace/reports/finalized-vad-26758e50.json`/`.log`, `finalized-segments-26758e50/`, `finalized-asr-26758e50.json`/`.log`. A09 segmentation and the A15 fixture lifecycle repair are accepted at their named boundaries. A14 noise admission, A34 quiet onset, numeric-equivalence grading and all later translation/TTS/lifecycle/receiver acceptance remain unresolved. No test was relabeled PASS manually; no CI, physical microphone, cloud inference or model substitution ran.

**Admission separating evidence — clean `e4aee665a0b3ff1c8beb964d345b7b61e4af88f5`:** a developer-only offline comparison invoked installed faster-whisper `get_speech_timestamps`/Silero CPU inference on 130 public synthetic controls. Original 48 PCM16 WAV hashes matched their manifest. Extra controls comprised the ten existing M1/F1 preview WAVs at gains 0/-12/-19/-24/-30 dB (50 in-memory float32 signals), 27 independently seeded white/low-pass/high-pass noise signals (seeds 7/109/2026, RMS 0.003/0.03/0.08), four tones (110/220/440/1000 Hz) and one silence. Derived controls were not re-quantized to PCM16 and are not new accepted canonical fixtures. Parameters remained library defaults (speech threshold 0.5, default segmentation/padding) except minimum speech duration 140 ms, matching the existing canonical minimum. No tuning sweep or dependency/download was performed.

Observed speech presence: all 45 original speech cases and all 50 gain controls detected; all 35 non-speech controls rejected, including A14 and the 61-second A15 tone. A34 was detected despite having no accepted current Rust energy-gate window. Earlier actual Rust receipts establish the production admission failures; extra energy-gate window counts in this Python comparison reimplement the inspected formulas and are not Rust execution evidence. The result supports a speech-presence detector rather than an energy-threshold-only repair, but does not certify boundary accuracy, intelligibility, A34 ASR quality, real fan/music/keyboard noise, human voices, streaming continuity, device sample rates or target latency. It is not a 130-case canonical application pass.

The existing asset was `silero_vad_v6.onnx`, 1,245,151 bytes, SHA-256 `4cbf549b8326f60f80f2536d9eefeb450a9abe83365a098031c89719f1be17d2`, loaded through the installed faster-whisper/ONNX Runtime CPU implementation. The [upstream Silero project](https://github.com/snakers4/silero-vad) documents the detector and MIT license; that does not establish provenance/native packaging completion for a future integration. No production VAD, worker request, finalizer, lockfile or asset policy changed. A native detector needs a resolved canonical-owner design: no second VAD pass after finalization, no inference in device callbacks, explicit load failure and correct state reset. A15's numeric safety test must remain independently covered if a future detector correctly rejects its tone.

Private evidence: `.workspace/reports/vad-admission-comparison-e4aee665.py`, `.json`, `.log`. Script SHA-256 `d7803793808fd904d50869ae35d1be012cb5696335bb44596b028e9011d63e54`; JSON SHA-256 `27d7af8f8fd2095abcd944af5113899ca8bc11e237a282aa57ba5965137da2c8`. Source remained clean/unchanged before and after inference. Approximately 658 ms summed bulk processing excludes startup and is not a live latency benchmark. The bounded diagnosis ends at a material architecture decision under development-brief; no A14/A34 runtime repair is claimed.

**Owner-approved native Silero implementation — source candidate based on `defa6392` (2026-10-26):** the owner explicitly approved one local CPU detector before finalization. Production microphone/Meeting Sound startup now loads the pinned Silero v6 and ONNX Runtime 1.31.0 DLL via Rust `ort=2.0.0-rc.15` (default binary downloads disabled). Model/DLL sizes and SHA-256 are verified before use; missing/mismatched assets fail capture startup. Each lane owns one session-bound CPU thread and the existing fixed-capacity PCM handoff. Device callbacks enqueue PCM with nonblocking ring admission; neural inference stays on the CPU thread. The private installed package root is flat `PythonRuntime` (confirmed from the existing staging source); development uses the verified worker `.venv/Lib/site-packages`. This is path/source proof plus actual development loading, not installed-package acceptance.

Speech presence uses model probability (0.5 onset / 0.35 continuation), 512-sample 16-kHz windows and persistent per-lane context/h/c. A rational sample clock preserves callback-partition-independent interpolation and original native samples/provenance, with source tests at 8/16/44.1/48/96/192 kHz. The existing finalizer retains pre-roll, minimum speech, quiet completion, clipping rejection, bounded queue/overflow and stale-generation checks; whole-utterance loudness no longer rejects neural-admitted quiet speech. Source/transport/error discontinuities reset model/window/phrase state. Stop revokes publication before a bounded CPU join and retains a timed-out lease. Fatal format/inference faults become explicit capture status errors; callback drops remain counted. There is no production energy-only fallback, second Python process or post-finalization VAD pass. Existing source-unit energy observations are explicitly test-only finalizer mechanics, not Silero quality proof.

Candidate native replay passed all 48 VAD expectations and wrote 57 actual segments: A14 produced zero, A34 one (1,996-ms speech span), A09 one (1,900 ms). A15 now specifies `no_speech` for its non-speech 61-second tone, while `incoming_lane_segmentation_eviction_and_overflow_are_observable` independently preserves the >60-second classified-utterance drop test. A stale prototype unit test raced a legal `try_push` lock-contention drop; its drain/Stop input is now prefilled before consumer start, with contention still separately tested. No runtime retry/threshold workaround was introduced. Native fixture checks also passed source discontinuity acknowledgement, stale-session Stop isolation, joined CPU cleanup, late PCM rejection and explicit format failure. The separate bounded-join test proves an unconfirmed worker lease stays owned until join.

Ordinary source proof: 341 Rust tests passed, 1 explicit model fixture test ignored; locked offline production Cargo check passed; 19 affected Node audio/source contracts and 16 Python harness/receipt tests passed; changed Python Ruff and dependency-usage/MSRV checks passed. Rustfmt is absent from the prepared toolchain, so no formatting-tool PASS is claimed. Neural fixture regeneration changed the scenario fingerprint to `a7e191bb2196b63f0c1c486f41e7ff92f9134283b2f61f849dacc5129b55bb14` but reproduced every original WAV byte: audio fingerprint remains `31fb53ed1a82db4e63cf74a5e00a386414d40344f37c04045de1e09163a30981`. Clean-pinned exported-WAV→worker ASR remains required before A34 recognition is claimed. Private candidate logs: `silero-native-final-candidate.log`, `silero-source-acceptance.log`, `silero-production-acceptance.log`, `silero-audio-source-contracts.log`, `neural-fixtures-m1-silero-v1.log` under `.workspace/reports/`. D-047 owns the approved rationale; package/device/receiver/translation/TTS acceptance remains unexecuted.

**Clean native replay did not pass on `42d53d62f9b5a13c65a79d5fe7fbd36206b16771`:** the VAD runner stopped with `canonical speech CPU drain timed out`; no exported segment receipt or ASR run was published. This failure is retained rather than counted as an accepted native-model run. Source diagnosis found the fixture's accelerated nonblocking input/completion contract unsound: a legitimate discontinuity can discard previously accepted queued PCM, leaving its accepted-versus-processed counter waiting indefinitely. The captured timeout alone does not measure physical-device callback QoS or establish a failed Silero inference.

**Fixture-boundary correction:** explicit offline input now holds the test-only ring admission lock while calling the same bounded/sanitized writer used by production `try_push`, then waits for the CPU owner. No blind runtime retry, longer deadline, disabled drop check, model threshold or fallback was introduced. Production `try_push` remains nonblocking and loss invalidates continuity. Each fixture declares `OFFLINE_PACED_SHARED_BOUNDED_PCM_WRITER_NOT_CPAL_CALLBACK`; completion timeouts now include bounded queue/counter diagnostics. This controlled multi-component fixture can establish model/finalizer/WAV output, not natural callback admission, sustained real-time reliability or device timing. Matching clean-source replay/ASR proof remains pending.

**Canonical Silero validation accepted at its controlled boundary — clean `0d7fd523d091b816e6b3437b501573a7667ad938`:** the existing VAD runner completed the explicit native model/CPU/finalizer/writer replay with all 48 scenario expectations passing. It exported 57 actual finalized WAVs, checked their full coverage/PCM/hash identity, and published segment fingerprint `cfbdedee56859a873c86492c7500bbd1d7479e438b5d707f118f8b2ade9f043a`. A14 noise and A15 tone each produced zero segments; A34 quiet speech produced one 1,996-ms speech span and A09 one 1,900-ms span. Audio fingerprint stayed `31fb53ed1a82db4e63cf74a5e00a386414d40344f37c04045de1e09163a30981`, with the updated A15 scenario fingerprint recorded above. The exact ignored test also completed its native source-discontinuity/reset, stale Stop, joined-cleanup, late PCM and format-failure assertions.

The existing worker runner then consumed actual verified finalized WAVs on that same clean SHA for A01/A02/A03/A04/A06/A09/A10/A11/A12/A13/A14/A15/A34. A34 returned exactly `Versi ini belum siap untuk dirilis.` and A09 `Bisa dengar suara saya dengan jelas?`; both had WER 0 and all critical checks passing. A01/A02/A03/A06/A10/A12 also had WER 0. A13/A14/A15 were explicitly graded `CANONICAL_VAD_REJECTED_BEFORE_ASR`, with `asr_executed=false`; this is not an empty-transcript or ASR-rejection claim. Zero segments for genuine speech remain unverified/failing, never safe-noise success.

The thirteen-scenario machine result remains **FAIL**, PASS=11/FAIL=2/UNVERIFIED=0, exit 2. A04 returned `Maksud saya 15, bukan 50.`; A11's two ordered finalized segments returned `Angka yang benar 125.` then `Bukan 150.`. The scorer still requires spelled-out number concepts and reports WER 0.5714/0.6364. Numeric values and negation appear in the ordered outputs; their digit-versus-word scorer discrepancy is retained for the ASR-quality owner. This does not certify every sentence boundary or later translation quality. No reference, threshold or score was changed to promote these failures.

Private matching proof: `.workspace/reports/finalized-vad-0d7fd523.json`/`.log`, `finalized-segments-0d7fd523/`, `finalized-asr-0d7fd523.json`/`.log`, and `silero-final-node-contracts.log`. Final source checks passed 341 Rust tests (1 explicit model fixture ignored), production Cargo check, 19 affected Node contracts, 16 harness/receipt tests, changed Python Ruff and knowledge/whitespace validation. Native fixture input uses controlled offline admission through the shared bounded writer, not CPAL callback contention. Physical input, actual callback QoS, sustained real-time reliability, installed assets/notices, independent receiver, translation/TTS and full Meeting lifecycle acceptance remain unexecuted. No cloud inference, CI dispatch, main promotion or release ran.

**Canonical numeric-scoring repair — candidate based on `919398d4` (2026-10-26):** the first wrong owner was evaluator normalization/critical checking, not ASR inference. A regression reproduced A04's nonzero WER before repair. The canonical evaluator now applies `id-standalone-cardinal-integers-v1` consistently to WER, CER, edit operations, reference denominator and invariant cues; the report declares that policy. Stored reference/transcript text is unchanged. Standalone signed integers up to 15 digits render as standard Indonesian cardinal words; padded identifiers and decimal/grouped/time/version formats are not inferred as equivalent cardinals. For numeric references, ordered numeric/polarity tokens must also match: a bag of both numbers plus `bukan` cannot pass a reversed correction or negation attached to the other value. Existing literal-stem and embedded-negation/digit guards remain intact.

The evaluator self-test passed all 60 references and the new A04/A11 scenario controls, including equivalent digit output, wrong values, reversal, moved/missing negation, sign changes, leading zeros and unsupported decimal representation. Six synthetic mutations through the actual finalized-ASR grading function correctly produced FAIL, including low-WER semantic errors. Sixteen existing harness/receipt tests passed. Ruff reported the same pre-existing `TRY004` type-validation finding on both original and candidate evaluator source, with no new diagnostics; its self-test file passed Ruff. No unrelated exception-type cleanup or lint-policy weakening was performed.

**Retained-inference regrade, not fresh model proof:** the immutable real inference report from clean `0d7fd523d091b816e6b3437b501573a7667ad938` was scored again with evaluator SHA-256 `235ad5f62a8bbfda6f52963b64cb1752a695836fb502d82d0a2990719084b786`. All thirteen selected checks now PASS (FAIL=0/UNVERIFIED=0); A04 and A11 have normalized WER 0, preserved cues and `numeric_sequence_match=true`. Original capture/report hashes and source/model/audio identities remain unchanged; the old 11-PASS/2-FAIL report remains preserved. This repairs representation-aware grading and strengthens numeric ordering checks; it does not claim improved model recognition or a fresh current-source ASR/VAD execution.

Private proof: `.workspace/reports/asr-numeric-before.log`, `asr-numeric-regrade-919398d4.json`, `asr-numeric-negative-controls-919398d4.json`. Regrade/negative receipts identify the original inference-report hash; the regrade pins the evaluator bytes and labels its replay-only proof. Canonical corpus, waveforms, model/dependency/runtime source and scenario thresholds were not changed. Actual translation/TTS, physical callback/device, installed asset/notices and receiver acceptance remain unexecuted; CI remains paused.

**Fresh local translation over retained ASR — clean `9adf28e7b3292cb26668cb883efdd5639ef0ffa6` (2026-10-26):** the existing persistent canonical worker translated the actual retained finalized-ASR transcripts from `0d7fd523`, with offline flags and no model downloads. Ten speech scenarios (A01/A02/A03/A04/A06/A09/A10/A11/A12/A34; nine ID→EN and one EN→ID) all returned successful, nonempty, complete EOS output in the requested direction. MiLMMT `milmmt-46-1b-v1.0`, revision `4fc480b6c58dec29c159dcdf9fde0f6d5c354995`, executed on CUDA/bf16 without degradation. Explicit agent review of each public input/output pair found preserved meaning, numbers, correction polarity, negation and temporal ordering: A04 produced `I mean 15, not 50.`, A34 `This version is not ready for release yet.` A12's Indonesian output preserved the request and before-meeting-end timing. No production source repair or threshold change was necessary.

A11 first used the chronological joined transcript, then its two actual segments were additionally translated as independent requests: `The correct number is 125.` and `Not 150.` Both completed with EOS and correct direction, preserving the correction without relying on concatenation or injected context. Private receipt `.workspace/reports/translation-retained-9adf28e7.json` (SHA-256 `a38a79707cf73f777572f2e5e5fa5abc7598d913ee0044c7a5760ee0a41395de`) records original ASR-report hash, original source/audio/segment identities, actual requests/responses, envelope checks and explicit semantic-review findings. The original ASR receipt remained byte-identical. This is `INTEGRATION_FIXTURE` fresh translation proof over retained inference, not fresh capture/VAD/ASR. Semantic PASS is explicit agent review, not a reusable automated semantic oracle. Short fixtures do not establish general translation quality, sustained latency, outbound history behavior or a full Meeting session. Selected-voice TTS, installed/device/callback and independent receiver acceptance remain unexecuted; CI remains paused.

**Selected built-in voice synthesis — clean `75fdf525a66a086c624bfa2a630810be173689f4` (2026-10-26): FAIL at A11 spoken-number semantics.** The existing selected actor manifest identifies packaged `MaleVoice` (`builtin_voice=true`); no actor replacement, training or personal-voice input occurred. Canonical worker `voice_actor_preflight` loaded GPT-SoVITS on CUDA with cached reference. Its legacy response label remains `voice_id=MyVoice`; the actual built-in identity is established by the unchanged manifest, not that label. Nine retained actual ID→EN translation outputs produced valid non-silent mono PCM16 32-kHz WAVs with exact requested output paths and the preflight actor token. Durations were 1.86–4.14 seconds. Preflight took 22.666 seconds; first synthesis 2.914 seconds, later calls 0.479–0.723 seconds. These are individual worker timings, not application latency or sustained performance acceptance.

Fresh canonical English ASR on the hashed output WAVs recovered eight of nine inputs exactly after punctuation/case normalization, preserving their negations and A04's `15, not 50`. A11's joined text `The correct number is 125. Not 150.` instead yielded `The correct number is 125th, not 150.` with both beam 1 and beam 5. Direct execution of the pinned upstream `text/en_normalization/expend.py` reproduced `one hundred and twenty-fifth, Not one hundred fifty`: its `_ordinal_number_re` interprets digits followed by a period and space as an ordinal list marker even inside a sentence. The same number before a comma or a terminal period without a following space remained cardinal; explicit `125th` remained ordinal. First wrong owner is the English TTS text-normalization boundary reached through `voice_lab_gpt_sovits.english_tts_inputs`, not ASR scoring or translation. Keep this as FAIL; do not dismiss the ordinal as numeric equivalence or patch the pinned vendor assets.

Private receipt `.workspace/reports/voice-translation-75fdf525.json` (SHA-256 `5a46a8aa6333383296817d07ef716c2eb8b6864d2471ede3d7e026501d0065a5`) pins translation receipt, selected actor manifest, preflight token, all output hashes/metrics, actual synthesis/ASR responses and upstream normalizer controls. Public output WAVs are retained in `UserData/CacheData/audio_validation/voice-translation-75fdf525/`. Input translation receipt and actor manifest remained byte-identical. This is `INTEGRATION_FIXTURE` worker synthesis plus content-proxy evidence, not perceptual listening/naturalness, FemaleVoice, pace-factor, model coexistence, Meeting context, physical device, installed-product or independent receiver proof. A11 independent-segment TTS remains unexecuted; the joined-sentence defect is independently reproduced in the real normalizer. No runtime source repair, cloud inference, CI or release ran.

**A11 cardinal repair — executed clean `ca9c637f3ea2c251d80ae9d1d6ef516f169ce3b3` (2026-10-26): PASS for selected built-in synthesis/content proxy.** The shared `voice_lab_gpt_sovits.english_tts_inputs` boundary expands standalone numeric expressions immediately followed by a sentence period and whitespace into English words via the already locked `inflect`. It also protects reference prompt text from the same upstream rule. Sentence punctuation and stored translations remain unchanged; explicit ordinal suffixes are untouched, decimal/grouped/signed expressions retain their value and padded digit strings preserve their leading zeros as spoken digits. It does not add a general date/version/identifier parser or reinterpret embedded identifiers. Five regression examples failed before repair; all 45 selected actor/embedding/zero-shot-preview tests and changed-file Ruff passed after repair. Direct execution of the actual pinned upstream normalizer confirmed A11 remains cardinal and explicit `125th` stays ordinal. No model/dependency lock or vendored runtime asset was changed.

Fresh selected MaleVoice synthesis then ran on the clean repair revision, reusing the exact retained actual translation receipt and unchanged actor manifest/token. All nine outputs passed actor/path identity and valid non-silent 32-kHz WAV checks. Fresh canonical English ASR recovered all nine input texts exactly after punctuation/case normalization, including `The correct number is 125, not 150.`; the old `125th` transcript still fails this content check. The eight unaffected WAVs are byte-identical to the pre-repair baseline. Private receipt `.workspace/reports/voice-repair-ca9c637f.json` (SHA-256 `8ca5a38d2536336cf645dabf6cdef8a6902fb76d8c3816723975b8cbf56502ca`) pins source, baseline/translation/actor hashes, actual model responses, WAV metrics/hashes and explicit content checks. WAVs are in `UserData/CacheData/audio_validation/voice-repair-ca9c637f/`. This `INTEGRATION_FIXTURE` result repairs the observed defect; separate worker lifetimes still do not prove simultaneous model residency, native session/receiver behavior or perceptual naturalness. FemaleVoice, pace and independent-segment A11 TTS remain unexecuted. No CI/cloud/release ran.

**Persistent canonical worker integration — clean `4d1939b645b807c14701a2ad4ca63876aa80d9f1` (2026-10-26): PASS for the selected short fixture run.** One actual worker process preloaded canonical ASR, MiLMMT and the unchanged selected MaleVoice actor, then consumed 11 hash-verified retained finalized WAVs from ten speech scenarios in chronological scenario order and reversed order on a second pass. This is fresh ASR/translation/TTS execution on old immutable VAD outputs; it does not relabel `0d7fd523` capture/finalization as current-source execution. All 22 fresh input transcripts matched retained ASR after punctuation/case normalization; all 22 translations matched previously explicitly reviewed public input/output anchors and completed with EOS in the correct direction. Twenty outbound requests synthesized valid non-silent, actor-token-bound WAVs whose fresh English ASR matched spoken input after punctuation/case normalization. The two EN→ID requests omitted English-voice TTS. Both actual A11 segments (`125` and `Not 150`) passed independently in both orders; critical negations/numbers remained present. No model/actor replacement, production source change or scorer relaxation occurred.

Both post-round status responses confirmed the same worker remained alive with ASR on CUDA, both MiLMMT directions resident and voice inference on CUDA:0. PyTorch allocated memory was 2,664 MiB in both snapshots; reserved was 2,812/2,816 MiB. CUDA device used/free snapshots were 5,047/3,144 and 5,051/3,140 MiB out of 8,191 MiB (approximately 4.93 GiB used, 3.07 GiB free). Device totals include CTranslate2 and other processes; they are not PyTorch-only ownership or independently verified WDDM accounting. Input-ASR per-call elapsed was 359–413 ms, translation 119–378 ms and TTS 327–2,741 ms; these exclude full capture/queue/route/receiver latency and include a cold first synthesis. Two short rounds do not prove sustained performance, concurrency or absence of a memory leak. The fixture explicitly terminated its worker and confirmed exit; this is cleanup evidence, not native Meeting Stop acceptance.

Private receipt `.workspace/reports/persistent-pipeline-4d1939b6.json` (SHA-256 `ff43e1f9712113808ba2b3c906835b4bc403443bb4d13a08fab980478fad3675`) records the exact current source, older VAD/ASR and translation identities, original segment hashes, unchanged actor-manifest hash, one worker PID, full actual responses, fail-closed checks, status/memory snapshots and termination result. Staged public input/output WAVs are retained under `UserData/CacheData/audio_validation/persistent-pipeline-4d1939b6/`; original segment/receipt/actor hashes were rechecked. This is `INTEGRATION_FIXTURE` proof, not naturalness, FemaleVoice, adaptive pace, outbound history context, full Meeting/native device/installed-app/independent receiver acceptance. The private diagnostic is not yet an option in the reusable public runner; that bounded tooling gap is the next continuation. CI/cloud/release remain unused.

**Reusable semantic pipeline — clean `e14d3c769a46ecc67793b986d0c511e38c6ebf6a` (2026-10-26): PASS for 13 explicitly selected scenarios, not the entire suite.** The existing worker runner now supports opt-in `--finalized-pipeline`, 1–20 rounds, and an explicit `--vad-source-sha` for retained inputs. Same-SHA remains the default; original scenario/audio/segment identities and VAD-receipt hash are checked, including after execution. Current worker and original VAD sources are recorded separately. Actual CLI execution used unchanged Supertonic/Silero fixtures and `0d7fd523` finalized exports, with A01/A02/A03/A04/A06/A09/A10/A11/A12/A13/A14/A15/A34 selected for two opposite-order rounds. All 26 scenario/round checks passed, with 22 actual input-ASR calls, 22 complete/EOS translations, 20 actor-bound non-silent TTS outputs and 20 English-ASR content proxies. The six non-speech observations had no ASR/translation/TTS invocation. Actual CLI exit was 0. One worker stayed alive through the selected requests, and snapshots retained ASR, both translation directions and the unchanged selected MaleVoice on CUDA. Cleanup confirmed fixture child termination; it does not prove product Stop.

Semantic checks use ten source/direction-bound audio contracts in the existing canonical translation corpus (190 regression cases total; 40 held-out unchanged), separately fingerprinted as `d245a69acfc64f39115b9a5ba61c38efc0347fe1fbcb6d58768b7febaf5f461a`. This does not change canonical audio/scenario fingerprints or regenerate WAVs. Optional validated `ordered_any` cue sequences/aliases enforce correction roles while accepting declared variants such as `15 rather than 50`, `not 50 but 15` and mixed digit/word forms. Reversed values, missing negation, signed/decimal substitutions in the cardinal controls, incomplete EOS, wrong actor/output, missing rounds and forbidden downstream work on noise cannot pass. Missing semantic contracts and noncritical roundtrip wording differences remain UNVERIFIED. Declared invariant success is not arbitrary semantic equivalence or naturalness acceptance. The CLI exits 2 for FAIL/UNVERIFIED; selected coverage is explicit, and unselected scenarios are outside its verdict.

Executed source checks: 19 harness/receipt tests plus four subtests passed, including real temporary-WAV orchestration with a simulated worker (not model proof), retained-source admission guards, semantic paraphrases/negative controls and nonzero CLI contracts. Canonical evaluator self-tests passed 190 translation regression/40 held-out and 60 ASR cases. Changed runner/test/self-test Ruff passed; translation evaluator retained the same pre-existing F401/TRY004 findings. A broader fault-suite run failed its existing eleven-injector expectation: unchanged baseline matrix/source has only ten matching registrations because F09's `context_isolation` slug differs from `translation_context_leak`. This baseline defect is unresolved, not an all-tests-PASS claim.

Private real-execution receipt `.workspace/reports/reusable-pipeline-v1.json` (SHA-256 `78dbbd29953a715a9698e1ca49dac55558615a32ec2e31654b830bc913b93826`) pins the exact source/input/contract identities and contains actual stage outputs, model metadata, actor/output checks, per-scenario first-wrong-stage results, GPU snapshots and cleanup. Temporary staged/output WAVs were removed by the runner; original exports remain available and unchanged. This is `INTEGRATION_FIXTURE` proof of the reusable CLI, not fresh VAD, all 48 cases, sustained/concurrent operation, Meeting history/context, FemaleVoice, adaptive pace, installed app, device/receiver or perceptual voice quality. No CI/cloud/release ran.

**F09 registration repair — candidate based on `17d7df22286dbca481e189753c6c11b7fd4927bb` (2026-10-26): PASS for existing offline fault coverage.** Before repair the existing fault test reproduced 10 matched registrations versus the required 11. The first wrong owner was `fault_runner.RUNNERS`: `translation_context_leak` did not bind to canonical F09 matrix slug `context_isolation`. The registration now uses that canonical slug, without changing the matrix, injecting a compatibility alias or lowering expected coverage. The existing coverage test additionally asserts every registered injector has a matrix owner and that F09 actually returns the canonical prompt-builder owner and source-only proof boundary.

Combined audio fixture/receipt/fault execution passed 24 tests plus four subtests. Actual fault CLI execution returned 11 PASS / 0 FAIL / 17 NOT_EXECUTED; F09 exercised `milmmt_translation_provider.build_prompt` for repeat determinism, reordered utterances, current-only input and reverse direction. It does not exercise model inference, Meeting history payloads or native session isolation. The 17 remaining faults stay explicitly unexecuted. Ruff diagnostics on both edited files matched their original baseline exactly (runner I001/BLE001; test two RUF100); no new diagnostics or unrelated lint-policy edits occurred.

Private receipt `.workspace/reports/fault-registration-f09-17d7df22.json` (SHA-256 `3c529450c852e72d3d128faaef09fd7ab089a2431c2060fcd2ba4b14f34bceef`) records actual injector execution. Tested candidate SHA-256: `fault_runner.py` = `a14c18534b12979590dd9f54253e18bd6c4074184de3ec9ad6b06ff59573e321`; `test_audio_fault_runner.py` = `286a4669e906c318ddf8bd4d15026af2d004c35b54949ba8906b46b6256b09b4`. This is `EXECUTED_SOURCE` offline fault proof, not a fresh ASR/translation/TTS pipeline or native acceptance. Existing model, audio, semantic contracts and production behavior were unchanged; no CI/cloud/release ran.

## Integrated developer acceptance — 2026-10-26

Fixed tested revision: `25eb19defc727d728810da3c326b180c36c24df5` on `Local`, with a clean tracked checkout and unchanged npm/Cargo/uv lockfiles. The user authorized comprehensive functional acceptance after the dependency refresh. This is partial developer-mode evidence, not installed-product or full functional acceptance.

| Check / scenario | Observed result | Proof and limit |
|---|---|---|
| Source admission and canonical build | PASS | Matching dependency-candidate source contracts, strict Svelte check, 421 frontend tests, production bundle, locked Cargo check and 337 Rust tests; one Rust audio replay remains ignored. The canonical exact-SHA compile gate passed before application launch. |
| Worker pytest | PASS, platform limited | A separate UAC-approved elevated child first passed both symlink-security cases, then the full suite: 316 passed, 2 POSIX-only cases skipped. No test or Windows security policy was weakened. |
| Translation / ASR / TTS evaluator self-tests | PASS | Translation: 180 regression + 40 held-out cases; ASR: 60; TTS: 30. These test evaluators, not model or listening quality. |
| Developer launch and navigation | PASS, bounded | Canonical `manage.ps1 run` built and opened the native app. Meeting, Text and Settings rendered and navigation responded. This is one launch, not cold-start reliability or complete UI acceptance. |
| Live Text EN→ID / ID→EN and Swap | PASS, bounded | A public three-sentence prompt translated through the local worker in both directions; native command durations were 5,147 ms and 448 ms respectively. Swap changed direction and exchanged both pane contents; Copy stayed disabled until a new current result. Copy, Clear, keyboard shortcuts, pending-edit races and full quality acceptance remain untested. |
| Physical microphone / Meeting admission | ENVIRONMENT FAILURE | Settings listed only the virtual CABLE Output input and correctly blocked spoken-microphone readiness. Windows present-only audio endpoints contained no physical microphone; the historical Triton USB microphone was non-present (`Unknown`). This does not establish a CPAL regression or authorize selecting VB-CABLE as a physical microphone. No microphone capture or Meeting session was started. |
| GPU and assets | Inventory only | RTX 3070 / driver 610.62 detected, and local Text inference succeeded. Five asset markers were present, but complete model/voice hash integrity, ASR/TTS execution, listening quality and coexistence/latency were not revalidated. |
| Remaining functional and distribution acceptance | NOT EXECUTED | Built-in/My Voice, physical capture/VAD, real receiver delivery, incoming isolation, repeated Start/Stop, captions/transcript/export, fault injection, packaging/install and clean-machine acceptance remain outstanding. CI stayed paused. |

The session stopped at the first confirmed device-admission blocker under the existing Windows runbook. The idle application and port-1420 frontend server were closed. No archived personal data was restored, no recording/training occurred, and no runtime source, model identity or deadline was changed. Resuming the audio lane requires an available physical microphone and the same bounded integrated acceptance gate; software changes require reproduced software evidence first.

Private local evidence: `.workspace/reports/acceptance-25eb19d-worker-elevated-result.json`, matching worker targeted/suite XML and stdout/stderr logs, `acceptance-25eb19d-tauri-dev.log`, and the matching dependency source/build logs. These files are not uploaded. Later documentation-only commits do not rerun or upgrade this fixed-revision evidence.

## Post-acceptance targeted source repair — 2026-10-19

Repository-level repairs occurred after the user paused the native session. Selected hosted CI source checks succeeded on fixed commits `0c99a4bb61e480d9508550fab7540b6dca968180` and `d24fa26b509279f918344ac26ff22ade7077c1bb`; consult those run/job identities for the exact tests performed. The native Tauri commands for ApplicationRuntime, standalone Text, worker status, model inventory, and built-in/saved/versioned voice selection now offload potentially blocking work. A manual worker recovery intent is restricted to a blocked helper without active audio/Meeting/build owners and uses the canonical native session check. Its emitted event forces refreshed worker and voice observations.

**Proof limit:** No new on-device cold-start, Windows UI freeze, recovery fault injection, Meeting microphone, receiver, CUDA/VRAM performance, or installer test was executed. The latest targeted run did not compile/test Rust or exercise the real Python AI worker. Previously observed startup delay and transient Windows Not Responding remain open native-verification questions, not claimed fixes. A production-only npm audit previously found zero vulnerabilities, while the development dependency advisory reported by npm install remains untriaged.

## Integrated developer acceptance — 2026-10-19

Fixed tested revision: `a2614711b51983161230910a0e0eec0d3364bc35` on `Local`, with a clean tracked checkout and unchanged npm/Cargo/uv lockfiles. The user explicitly authorized this integrated Windows developer session, then requested that its progress be recorded before further testing. These observations do not establish release or full native acceptance. This documentation-only update does not rerun or transfer proof to changed runtime source.

### Preparation and executed proof

- Old application data was moved to a private archive outside the checkout: 18 entries / 1,635 files, with file counts, sizes and SHA-256 verified. Tracked structural README files, engine/model assets and existing backups were retained. Archive contents and absolute machine paths are not repository inputs or public evidence.
- Canonical `manage.ps1 prepare` passed with pinned workspace Node 22.23.3, Git 2.56.0.windows.2, uv 0.12.1, Python 3.12.10 and Rust 1.88.0. Locked frontend and 121 worker packages were installed. npm reported one high-severity advisory; exploitability/remediation was not established and no dependency upgrade was performed.
- Bootstrap corrections are `952f511c` (accept exact uv version plus official build metadata), `91d83516` (preserve informational rustup stderr without bypassing exit/version checks), and `a2614711` (retain selector commands after nested forced imports). The offline bootstrap fixture and real tool version checks passed in Windows PowerShell 5.1. An earlier `Get-FileHash` autoload failure was not reproduced in isolated probes; subsequent preparation explicitly loaded the Utility module. Do not claim the original launcher autoload condition was repaired.

| Check / scenario | Observed result | Proof and limit |
|---|---|---|
| Repository and frontend source contracts | PASS | EXECUTED_SOURCE on fixed revision |
| Strict Svelte check | PASS | 0 errors, 0 warnings |
| Frontend runtime tests | PASS | 418 passed |
| Worker compile / Ruff | PASS | Source checks, not inference |
| Worker pytest | 316 passed, 2 skipped | Initially two fixture symlink creations failed with Windows error 1314. A dedicated UAC-approved elevated child ran those same two tests successfully, then the full suite. Two POSIX process-group cleanup tests were skipped on Windows. Tests and OS security policy were not weakened; skipped cases are not PASS. |
| Translation / ASR / TTS evaluator self-tests | PASS | 180 regression + 40 held-out / 60 / 30 cases; evaluator correctness only |
| `manage.ps1 build` with expected SHA | PASS | Frontend bundle and `cargo check --locked`; 11 Rust warnings. Rust unit suite was NOT EXECUTED in this session. |
| Required ASR and MiLMMT assets | PASS, bounded integrity | Existing manifest revisions and 7 ASR / 10 translation SHA-256 pins matched; no model substitution. GPT-SoVITS source/checkpoint integrity was not revalidated in this session. |
| Developer application launch / clean setup | PASS, bounded | Native app built and opened first setup without restoring archived settings or personal voice. Deferred setup survived one controlled restart. A transient unresponsive startup window was observed. |
| Live Text ID→EN and EN→ID | PASS, bounded | Public synthetic prompts translated by the canonical local model and rendered in native UI. ID→EN Rust command trace: 6,329 ms; this is not Meeting latency or a quality benchmark. Swap was observed; Copy, Clear, in-flight rejection and full settings persistence were not accepted in this session. |
| Cold worker startup | UNRESOLVED | Initial status exceeded the existing 30,000 ms response deadline. Isolated worker ping/status completed in 28.36 seconds; captured stacks showed dependency imports through faster-whisper / CTranslate2 / Transformers / PyTorch. One controlled application restart reached Text ready. This does not prove cold-start reliability or a permanent fix. Runtime code and deadlines were not changed. |
| Meeting first setup and Built-in Male readiness | PASS, bounded | Saved Windows Default resolved to the physical Triton USB microphone; setup reported Ready. Standard CABLE Input/Output route was available. Selecting Built-in Male loaded Text2Semantic, VITS and CNHuBERT; synthesis and ASR probe activity was observed and the application reported voice Ready. Setup completed and Meeting exposed Start Translation. Windows marked the app Not Responding during voice preparation, then it recovered. No listening-quality or receiver-delivery claim. Runtime source still matches the tested revision; intervening commit `aaa8a04b` changed documentation only. |
| Live Meeting, real audio and personal voice | NOT EXECUTED | Awaiting the user's readiness to speak a public test sentence; Meeting has not been started. No separate calling-app receiver was observed. Physical speech/VAD, remote delivery, incoming isolation, five Start/Stop cycles, measured resource/latency baseline and My Voice approval remain unverified. |
| Signing / Setup / installed app / clean machine / release | NOT EXECUTED — out of scope | Developer build and Text results do not prove these boundaries. No hosted CI was executed for this local session. |

### Diagnosis boundary and private evidence

The user ended this acceptance session on 2026-10-19 before the public microphone test because they were not ready to proceed. Meeting remained Ready with Start Translation available and no saved conversation entries; no Meeting session was started. The idle developer app was closed. Further native acceptance requires a new explicit user instruction; completed evidence remains limited to the rows above.

Diagnostics Check Again refreshes a snapshot; the inspected recovery intent does not start a worker already marked `blocked`. The isolated status response reported `voice_actor:approved_actor_missing` with fresh data, while the restarted application independently reached Text ready. This is not evidence that optional My Voice must be restored or trained for Text. Cold dependency import timing and blocked-worker recovery remain distinct unresolved observations; do not hide them with increased deadlines, automatic retries or archived-data restoration.

Private evidence remains on the tested PC: `.workspace/reports/worker-targeted.xml`, `worker-suite.xml`, `worker-elevated-result.json`, the associated stdout/stderr logs, and `worker-status-probe.*.log`; `.tmp/validation/RuntimeTestReports/local-tauri-cargo-check.log` holds compile output. The archive retains its manifest and a local result receipt. These are retrieval hints, not uploaded GitHub artifacts. No raw logs, recordings, settings, credentials or personal paths are committed. Current continuation is owned only by `planning/development.md`.

## Meeting Performance Foundation — source audit (2026-10-12)

- Audited source baseline: `77f95bc65630a46c7f86d6a79667fc64b1ea1044` on `Local`.
- Searched 92 Rust source files for `MeetingOutboundTiming`, `MeetingOutputDeliveryReport`, `OutboundTimingContext`, and `HelperTaskPermit` constructors/references; no overlooked constructor was identified by static inspection.
- Reviewed scheduler-wait plumbing, per-utterance latency metrics, and diagnostic/frontend fields. **Source inspection only: not compilation or runtime verification.**
- No matching GitHub Actions run was found for that SHA. Available execution context lacks Cargo and direct GitHub clone networking. The frontend, Rust, and worker suites require an appropriate clean build environment.
- GPU VRAM residency, actual Chrome/Meet delivery, real native output latency, translation fidelity, P50/P95 and 30-minute stability remain **NOT EXECUTED** for this revision.
- Later documentation-only commits do not upgrade the proof ceiling of this source revision.


## Developer acceptance evidence — 2026-10-12

Fixed tested revision: `230ca94faae178659a7738fa82399485c267c991` on `Local`; initial requested candidate: `497041064aee4e797db57dc8a97a67974e5577aa`. Execution occurred in the user-authorized integrated developer phase on the target Windows PC. The user subsequently paused testing to continue source work in ChatGPT. This is partial acceptance, not a release or full-test completion. Documentation-only descendants retain these observations for unchanged runtime source; source changes require their own relevant proof.

### Environment and prepared inputs

Windows developer runtime, RTX 3070 8 GB, approximately 32 GB system RAM, NVIDIA driver 610.62; Node 22.23.3 in a dedicated test directory, Rust 1.96, Python 3.12.10, uv 0.12.5 and Visual Studio 2022 Build Tools. Initial `DEV.cmd doctor` and locked `DEV.cmd setup` passed. Existing dependency/model pins were preserved. Setup reported two high-severity npm advisories; remediation/exploitability was not established and no automatic dependency upgrade was performed.

| Input | Verified identity / preparation |
|---|---|
| Faster Whisper large-v3-turbo | `dropbox-dash/faster-whisper-large-v3-turbo` revision `0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf`; 7 manifest hashes verified |
| MiLMMT | `xiaomi-research/MiLMMT-46-1B-v1.0` revision `4fc480b6c58dec29c159dcdf9fde0f6d5c354995`; 10 manifest hashes verified |
| GPT-SoVITS | upstream source `d523079fc05d9a8028d6085bffe4a2757c32abb6`; pretrained snapshot `336b2ec4e8d4ac74740798dd40af44e74659ecaf`; 4 manifest checkpoint hashes verified, pinned HuBERT/RoBERTa resources staged |
| NLTK | data revision `550b6625bcef1f2abff2ff770a5a0d272c9c6b2a`; cmudict and both required tagger archives hash-verified and extracted |
| FFmpeg | BtbN `autobuild-2026-09-30-13-08`, archive SHA-256 `4a7642b2264c03e8a0ce8a3825b933ee5580656f45695a086fe7e294045ffc0a`; binary/version/LGPL provenance checks passed |
| Built-in voices | existing licensed Male/Female reference hashes verified; both actors exercised through the canonical selected-voice worker smoke |
| Device/data baseline | UI discovered Triton physical USB microphone and standard CABLE Output; discovery is not capture/delivery proof. Public ASR WAV and 20-prompt subsets per evaluator domain prepared locally; canonical full corpora unchanged. Optional ASR medium was not required/staged. No personal voice dataset supplied. |

Public pinned downloads succeeded with implicit Hugging Face token use disabled for that session after an expired-token authentication failure. No credentials were changed or exported. Engine assets and local settings are not included in Git source.

### Executed checks and live observations

| Scenario | Result | Evidence ceiling / limit |
|---|---|---|
| Repository verification and frontend/source contracts | PASS | EXECUTED_SOURCE on tested revision |
| Strict Svelte/typecheck | PASS | 0 errors, 0 warnings |
| Frontend runtime tests | PASS | 289 passed, 0 failed/skipped |
| Rust `cargo test --locked` | PASS | 205 passed, 0 failed |
| Worker Ruff/pytest | PASS | 161 passed; final worker contents match tested revision |
| Translation / ASR / TTS evaluator self-tests | PASS | 180 regression + 40 held-out / 60 / 30 cases; evaluator correctness only, not real quality scores |
| `DEV.cmd build` with expected SHA | PASS | frontend bundle and locked Cargo check; developer compile admission, not installer |
| Canonical persistent-worker CUDA smoke | PASS | actual ASR, complete ID→EN and EN→ID, intentional overlength rejection and persistent translation lifecycle |
| Built-in Male and Female synthesis | PASS | separate selected-voice smoke runs produced WAV artifacts on CUDA; auditory quality/playback not accepted |
| Setup defer → Text, checkpoint after restart | PASS | native developer UI observation |
| Male selection after restart; Male→Female | PASS | native UI selection and persisted actor identity; Female restart persistence not observed |
| Text ID↔EN, Swap | PASS | actual canonical results rendered in native UI for public prompts |
| Copy | PASS | UI copy followed by paste into source matched translated text |
| Clear / Undo | PASS | disposable public source and result emptied, then restored during app session |
| Focused Ctrl+Enter / Ctrl+K | PASS | translation triggered from focused editor; Quick Actions opened |
| Terms and style save | PASS | spoken term, preferred translation and Formal style save acknowledged; preferred English wording appeared in actual result |
| Alternative wording | PASS (bounded no-change case) | explicit no meaning-preserving alternative response preserved original result; a distinct successful alternative was not observed |
| Existing result invalidated after source/style change | PASS (limited) | Copy/wording/caption actions disabled for stale displayed result; in-flight stale response rejection not exercised |
| Settings restart persistence | NOT EXECUTED | save acknowledged in-session only; final restart not performed |
| Quick Translate / floating-caption delivery | NOT EXECUTED (attempt incomplete) | search/click attempted; later disabled command and completed 34-character translation trace observed, without caption result. Escape interrupted observation. No confirmed root cause or failure verdict. |
| Physical microphone Meeting, short/normal speech, Stop during processing | NOT EXECUTED | initial pending-readiness message only; no Meeting Start in final session |
| Google Meet route and separate receiver | NOT EXECUTED | no receiver observation; delivery unverified |
| Incoming captions, self-TTS suppression and outbound isolation | NOT EXECUTED | outbound prerequisite not accepted |
| My Voice recording, preview/cancel, training, 8 held-out samples, approval/reuse/recovery | NOT EXECUTED | authorized recording dataset and human listening/approval missing; built-in engine smoke does not prove training |
| Full keyboard/accessibility/minimum-size/DPI/caption hide-show/privacy matrix | NOT EXECUTED | only named interactions and Settings scrolling observed; no full visual/privacy acceptance |
| Cold/warm Start→Live, 10–20 utterance latency, five cycles, 30-minute stability | NOT EXECUTED | no session-level CPU/RAM/GPU/VRAM/drop/restart/capture/late-output measurements |
| Installer/signing/installed updater/clean-machine/main promotion/hosted CI | NOT EXECUTED — out of scope | developer execution proves none of these |

### Resolved admission failures and remaining diagnosis

The initial candidate did not pass all executable gates. Its first failing owners were corrected on `Local`, then relevant checks were repeated; earlier failed runs are not current PASS:

- `b07bc10c`, `4e24c81a`, `37549fe4`, `285b5043`: missing governance contracts, removed microphone diagnostic read, strict Svelte/Windows launcher errors and a capability validator ignoring separate main/overlay ownership.
- `c4d5cf2f`, `88945eea`, `5fa0fc21`, `cc4487b0`, `8fc1bed4`: stale frontend/worker assertions, worker lint, Rust built-in voice/preview references and shared audio/session fixture isolation.
- `0023e6ee`: Windows npm launcher EINVAL; canonical build now invokes the current Node npm CLI directly.
- `c4145afd`: Tauri updater startup null-configuration panic; inert developer config supplied while release signing gates remain unchanged.
- `e6b34865`, `d3a54b4a`, `230ca94f`: upstream output polluting JSON stdout, reference decoding demanding absent TorchCodec, its stale static protocol assertion and bundled NLTK resources absent from the cached search path. Canonical headless shims isolate provider output, use locked SoundFile reference decoding and register the pinned NLTK path; no upstream/model/dependency substitution.

Do not infer a Quick Translate defect solely from the interrupted UI attempt. Inspect `src/components/runtime/ProductivityActions.svelte`, `CommandPalette.svelte`, `src/app/runtime/translationOverlayRuntime.ts` and their existing bridge/Rust consumers under `EngineData/Frontend/RustApp` to separate clipboard access, command execution, busy/readiness mapping and overlay publication. The early Meeting session-readiness notice likewise remains an observation, not a diagnosed capture failure. Native testing stays paused per the user; continuation belongs to `planning/development.md`.

### Private evidence retention

Raw logs, local paths, settings and media remain outside tracked source. Locally retained ignored receipts include `.cache/acceptance-source-230ca94f.log`, `acceptance-cargo-230ca94f.log`, `acceptance-repo-230ca94f.log`, `acceptance-build-230ca94f.log`, `acceptance-nltk-root-worker.log`, the three `acceptance-*-self-test.log` files, separate Male/Female smoke logs, and `acceptance-text-settings-run-230ca94f.log`. The local baseline summary is `.cache/acceptance-result-230ca94f.json`; final interrupted UI observations are recorded in the fixed-revision table above. Public prompt inventory is `.cache/acceptance-test-data/engine-and-public-data-230ca94f.json`. `UserData/LogData/RustAppValidation/latest_worker_smoke_result.json` is mutable and was overwritten by Female; separate smoke logs preserve both runs. These paths are retrieval hints on the tested PC, not downloadable GitHub proof artifacts. No conversations, recordings, credentials or raw machine paths were committed.

## Current Source Claims

Subject to matching proof, current source establishes:

- one generation-bound Meeting lifecycle authority with fail-closed Start/Stop, cleanup, route binding, stale-work rejection, bounded helper recovery, and at-most-once output;
- explicit ownership for Meeting, Mic Test, My Voice, helper scheduling/transport/readiness, Windows audio, storage/promotion, and shared-resource arbitration;
- one canonical MiLMMT ID↔EN pipeline with explicit bounded degraded fallback and no silent cloud/parallel fallback;
- bounded capture/context/queues/transcripts/diagnostics/incidents/cache/feedback/temp data;
- latency hardening through bounded finalized-audio preparation, deterministic MiLMMT generation, warm actor reuse, and reference-speaker embedding caching;
- quality tooling for Translation, ASR, and TTS/My Voice with baseline-vs-candidate regression contracts plus cross-domain Quality Readiness aggregation;
- one-shot signed updater source contract with no updater polling daemon;
- bounded productivity contracts: temporary last-session review, in-memory Text cache, and opt-in historical translation feedback; Quick Translate, Meeting Presets and user-driven terminology are retired;
- application-level coordination source contracts: typed ApplicationRuntime snapshot/intent/mutation boundaries, distinct Meeting/Mic Test/My Voice owners, event-first Meeting reconciliation with slow fallback, centralized shutdown, modular frontend controllers, split facades/state policies, and guarded compatibility surfaces.

Static Skill admission and procedure-case validation are repository-source checks only. `--score-procedures` validates the consistency of externally supplied agent receipts; it does not observe or verify agent decisions itself. Unrun checks remain unverified.

## Verification surfaces

Current policy: **GitHub source management only; no cloud/hosted execution**. The eight existing manual-only GitHub Actions definitions are retained as inactive source files and must not be dispatched. A workflow definition, historical status or queued job is not a current executable PASS.

```text
GitHub Local
→ canonical source ownership, stable docs, bridge contracts, dependency/lock inspection
→ static review and bounded candidate proof planning only

Repository Verify / Code Health / MiLMMT / ASR / TTS / Quality Readiness /
WorkerRuntime Lock / R3 Release
→ existing workflow definitions, not active runners
→ their results are NOT EXECUTED for current Local until actually observed

Single integrated local build/acceptance phase (only after source candidate admission)
→ npm/frontend checks and build, Cargo/Rust checks, Python worker tests
→ controlled package creation, install, native microphone/GPU/meeting evaluation
→ each claim requires an exact revision and actual measured output
```

There is no hosted CI fallback or required default-branch change. Source inspection is sufficient only for source claims; executable status stays unknown until the integrated local phase. Missing proof does not authorize fabricated CI, extra branches or repeated user-PC tests.

## Proof Boundaries

### REMOTE_GITHUB

Can establish source ownership, source-defined contracts, bounded history and static consistency only. No hosted execution is authorized.

It does **not** prove physical Windows microphone/device behavior, real CUDA throughput, installed-package behavior, meeting-app reception, audio fidelity, or practical latency.

### LOCAL_CODE

Can additionally prove the exact checkout/toolchain/filesystem/build that actually ran locally. It still does not automatically prove target-device acceptance.

## Target Windows

Native acceptance is not an incremental user-PC request. Build/package and native claims remain `UNKNOWN / NOT EXECUTED` until a source-integrated candidate reaches the grouped local build/install/test phase in `docs/knowledge/operations/target-windows-performance.md` and the relevant steps actually succeed.

### TARGET_WINDOWS / NATIVE_ACCEPTANCE

Required for:

- physical microphone and VAD behavior;
- VB-CABLE / TranslateIT Meeting Microphone routing;
- actual meeting-application reception;
- real GPU/CPU/RAM/VRAM behavior;
- Start → Live timing;
- end-of-speech → first translated playback timing;
- audible My Voice quality and practical stability.

## Evidence Rule

```text
claim
→ semantic owner
→ matching verifier/scenario
→ exact source identity
→ completed matching evidence
→ only then PASS
```
