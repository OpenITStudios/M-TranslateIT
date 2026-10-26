# System-first audio testing: deep acceptance plan

This file is a **planned acceptance matrix**, not evidence that any scenario passed. It complements the canonical 60-case ASR corpus, 48 synthetic waveform scenarios, existing Rust VAD/WAV writer, and the Windows acceptance contract. No new production engine or microphone input path is introduced.

## Gates

| Gate | What runs | Pass evidence | Not proven |
|---|---|---|---|
| G0 | Corpus/schema/hash/negative-proof tests | Original WAV SHA, full scenario identity | Speech accuracy |
| G1 | Canonical test-only Rust VAD + writer replay | Finalized counts, writer status, overflow/drop | Real input capture |
| G2 | Actual local ASR → MiLMMT → optional TTS | ID/model/device, WER/CER, critical cues, EOS, actor and non-silent WAV | Meeting delivery |
| G3 | Owner-specific fault/recovery tests | Exact failed stage, authoritative state, bounded cleanup | Physical hardware |
| G4 | Windows audio injected from public sample through independent receiver | Captured receiver audio, session/generation and route receipt | Human spontaneous speech |
| G5 | Very short final user acceptance | Real mic, translated playback heard independently, Stop and incoming check | Every hardware combination |

A gate may advance **only** after all mandatory cases have been executed on a matching source/audio/model identity with no critical failures. `UNVERIFIED` never counts as PASS. Retain the first wrong owner and explicit blocker for every failure.

## Coverage

- **Semantic criticals:** preserve negation, corrections, numbers, dates, units, acronyms, and ID↔EN direction even when aggregate WER looks acceptable. Reject outputs without generation completion or EOS.
- **Acoustic and boundary:** normal/brief/fast/quiet speech, pauses, clipping, additive noise, silence, tone, VAD short-segment rejection and >60-second overflow; distinguish VAD missed finalization from WAV writer rejection from ASR empty result.
- **Reliability:** cold/warm starts, worker process loss, request/deadline mismatch, Stop races, session source switching, device removal, archive failure, scheduler precedence, buffer discontinuity, repeated start/stop, and stale synthesized output.
- **Long-run:** repeated playback, memory/latency P50/P95 and budget limits, transcript isolation, large Unicode output, retry behavior, unexpected reset/reconnect; never claim sustained stability from one short fixture.
- **Provenance:** hash of audio, corpus, generator/settings, source SHA, model revision and the actual stage receipts. A hash match means unchanged bytes, not realistic human speech.
- **Known simulation limits:** text in ASR corpus labeled real accents, Zoom/Teams/Meet compression, packet loss or overlapping people does **not** become real accent/compression/overlap evidence when read by eSpeak with additive noise.
- **Privacy:** keep generated WAVs in ignored local UserData, not Git; no personal voice or meeting transcripts in fixtures.

## Fault isolation map

| Domain | IDs | First investigation owner |
|---|---|---|
| Worker startup/protocol | F01–F07 | worker transport, deadline, request ID |
| Translation/TTS | F08–F10 | EOS, context, voice actor and audio WAV |
| VAD/PCM | F11–F15 | canonical finalizer/writer/drop counters |
| Windows Meeting route | F16–F22 | Stop generation, route, independent receiving app |
| Persistence/performance/priority | F23–F28 | archive, device memory/timing, scheduler, retry |

See [fault_matrix.json](fault_matrix.json) for machine-readable planned faults. Actual injector selection is derived from exact matrix/runner bindings at execution; absent injectors remain NOT_EXECUTED. Queue/protocol/source-function checks never prove actual model/device recovery. Revision-scoped results belong to Current Validation, not this specification.

## Final manual workload

Once G0–G4 have *actually passed*, a user only needs a short integrated acceptance: launch and confirm Ready, speak one Indonesian negation+number sentence, verify English speech arrives at a **separate receiver**, verify one incoming English phrase appears as Indonesian text, press Stop and check audio immediately stops. Optionally rate voice intelligibility. Reserve more manual testing for proven device-specific gaps rather than repeating every synthetic case.

## Maintained regression contract

The single developer entry point is `worker_runner.py --profile source|core|endurance`; see [the command and maintenance boundaries](README.md#official-regression-entry-point). The core package and execution policy are owned by `scenarios.json` → `regression`, independent of acoustic fixture identity. Source admission runs existing negative-proof/protocol/evaluator checks. Named model profiles execute fresh canonical VAD at the same source, then one persistent worker; custom retained-WAV diagnostics explicitly retain older input-source identity.

`pipeline_checks.py` owns the decision path used by execution and offline scoring. Canonical ASR/translation evaluators remain semantic owners. Mandatory segment coverage/order/hash, EOS/direction, correction/negation, actor/output, silence rejection and absent downstream work on noise are checked. Missing evidence is UNVERIFIED; no mandatory failure or unknown may pass. Reports contain expected/actual/evidence and retain failed synthesized audio. Tests of protocol changes must not require changing stable scenario expectations.

Version-one system delivery is complete when one core command covers the fixed vital package, all its mandatory checks and negative controls execute, failures identify the first wrong stage, offline regrading agrees, and maintenance boundaries explain which owner changes while fixtures remain stable. This does not require claiming all 48 downstream scenarios, all planned faults or G4/G5 acceptance. Expanding coverage requires a product-risk gap or observed failure, not test-count growth.

Audio is generated separately and reused by immutable hashes. Synthetic fixture quality is not human accent/meeting-compression/perceptual proof. Current executed evidence lives in `docs/knowledge/current-validation.md`; source presence or this planned matrix never creates PASS. Naturalness, real capture/route/receiver and installed-product gates remain explicit and separate. CI remains paused.
