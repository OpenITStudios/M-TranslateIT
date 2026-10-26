# Offline Audio Regression Harness (developer tool)

**Scope:** Reproducible public audio fixtures and evidence-scoped regression checks. This is **not** a new production VAD, ASR, translator, or TTS engine. The 60-case corpus and all canonical ASR reference/invariant logic remain in `tools/asr_quality/`. This harness selects 48 versioned scenarios and adds audio-file provenance.

## Safety / execution boundary

- **GitHub CI is paused by owner.** No GitHub Actions job, model inference, Windows microphone, device capture or cloud service runs automatically.
- `harness.py validate/inspect/score/compare` never starts a model or microphone. Explicit `generate` uses the selected offline eSpeak executable or approved local neural fixture model; it never uses a cloud fallback.
- `worker_runner.py` is **explicit opt-in**, requires a clean checkout at an externally pinned `Local` SHA and an existing pinned WorkerRuntime Python interpreter/model assets. It starts only the canonical persistent worker, with forced offline Hugging Face flags. It **does not** run Meeting, native VAD, virtual microphone, speaker playback, receiver delivery or training.
- Use **public test utterances** only. Do not commit user speech, private recordings, selected personal-voice data, model binaries, or generated WAVs. Use `UserData/CacheData/audio_validation/<unique-run>` for generated fixtures and `UserData/LogData/` for local observations.
- Production capture hands PCM to one native CPU Silero speech detector before the existing Rust finalizer. `pcm_handoff_replay.rs` remains a source-unit fixture for transport/finalizer mechanics; its classified energy inputs are not neural speech-quality evidence. Direct worker smoke cannot prove VAD or Meeting delivery.

## Workflow

### Official regression entry point

The maintained acceptance entry point is `worker_runner.py --profile source|core|endurance`. It uses existing validators and the canonical VAD/worker; it never installs dependencies, regenerates audio, uses cloud inference or triggers CI. Run it from the prepared local development shell with the existing Python/Cargo/model prerequisites. Reports and their `.json.artifacts/` directories must be new; original evidence is never overwritten.

```powershell
python tools/audio_validation/worker_runner.py --profile source --source-sha <clean-Local-SHA> --output <new-source-report.json>
python tools/audio_validation/worker_runner.py --profile core --source-sha <clean-Local-SHA> --fixtures <immutable-fixtures> --python <existing-worker-python> --output <new-core-report.json>
python tools/audio_validation/worker_runner.py --profile endurance --source-sha <clean-Local-SHA> --fixtures <immutable-fixtures> --python <existing-worker-python> --output <new-endurance-report.json>
```

`source` runs the existing audio contract/fault tests and ASR/translation evaluator self-tests without loading models. `core` first requires that admission, then runs fresh canonical Rust VAD/WAV export at the exact same source and two opposite-order worker rounds over the fixed 13-scenario package. `endurance` uses the same package for 20 rounds. The existing VAD verifier reads all 48 inputs; the named regression verdict covers only the explicitly listed core package, not all 48 downstream scenarios. Missing prerequisites fail with a report before worker inference; there is no automatic fallback to old VAD results. Custom retained-input diagnostics remain available below and are explicitly labeled as replay.

Package membership and round policy have one owner: `scenarios.json` → `regression`. Their fingerprint is independent of the acoustic scenario/audio identity, so execution-policy changes do not force WAV regeneration. Named profiles cannot be reduced by `--ids` or `--rounds`. Current downstream contracts cover every speech member of the package. `pipeline_checks.py` owns pipeline acceptance; both execution and `harness.py score` call its same decision function. V2 receipts explicitly bind expected segment coverage/order/hashes, current VAD requirements, semantic corpus and source admission; older V1 diagnostics remain historical and are not silently migrated.

PASS requires every mandatory selected check in every round. FAIL is a violated contract; UNVERIFIED is missing evidence or unresolved content difference. Both exit 2. Each unsuccessful check includes expected behavior, actual stages and JSON evidence pointers; the terminal shows the first five plus the full report path. Failed synthesized WAVs are retained with hash-bound paths; successful temporary outputs are removed. Current VAD receipt and finalized exports remain in the report's artifact directory. Per-stage p50/p95/max worker elapsed times (nearest-rank p95) and per-round allocator samples are observations, never native latency or leak-free acceptance. Child termination is fixture cleanup, not product Stop proof.

Maintenance boundaries:

| Change | Owner to update | Stable inputs |
|---|---|---|
| Audio condition / package membership | `scenarios.json`, fixture generator when acoustic inputs change | Unaffected scenarios and semantic contracts |
| Expected ASR / translated meaning | Existing ASR / translation corpus and evaluator | Audio and worker protocol |
| Worker protocol or stage invocation | `worker_runner.py` transport/stage adapter | Scenarios, audio and meaning |
| Pipeline evidence / decision | `pipeline_checks.py` and its negative controls | Model implementation |
| Fixture schema or identity | `harness.py`; existing `vad_runner.py` for exported WAV evidence | Product inference semantics |

Only change scenarios when product requirements or a demonstrated test-contract defect requires it. Model/dependency upgrades do not authorize replacing reference text, regenerating fixtures or accepting a new baseline automatically. Source/model/audio/contract identities must remain explicit. Naturalness, real devices, installed app, Meeting context and independent receiver acceptance retain their separate gates.

### Fixture preparation and diagnostics

For natural Indonesian input, use the owner-accepted M1/F1 profiles through the existing [local neural fixture workflow](NATURAL_VOICES.md#current-selected-method-local-neural-listening-preview). The listening preview preserves 44.1-kHz originals; the canonical generator publishes separate hashed 16-kHz fixtures using the same synthesis. Profile acceptance does not certify every sentence or any ASR/VAD/Meeting result. The eSpeak route below remains mechanical smoke tooling.

From repository root, with Python 3.12 and an installed offline eSpeak-compatible synthesizer:

```powershell
python tools/audio_validation/harness.py validate
python tools/audio_validation/harness.py generate --tts-exe espeak-ng --fixtures UserData/CacheData/audio_validation/public-v1
python tools/audio_validation/harness.py inspect --fixtures UserData/CacheData/audio_validation/public-v1
```

Use `espeak` instead of `espeak-ng` **only** if it is the explicitly installed, reviewed offline executable supporting `id` and `en`. Fixture generation records the version, deterministic seed, canonical corpus/scenario fingerprint, SHA-256 of reference text and every WAV, plus PCM metrics. It refuses to overwrite prior output. The deterministic linear resampler is solely for smoke-fixture conversion, **not** an audio-quality benchmark of the application.

To exercise the existing AI worker on the user's **already prepared** local Windows test environment after explicit acceptance authorization (no downloads, no microphone), supply the pinned interpreter and SHA:

```powershell
python tools/audio_validation/worker_runner.py --fixtures UserData/CacheData/audio_validation/public-v1 --python EngineData/Backend/LocalWorker/WorkerRuntime/.venv/Scripts/python.exe --source-sha <40-char-clean-Local-SHA> --ids A01 A03 A04 A12 --output UserData/LogData/audio_validation-worker-v1.json
python tools/audio_validation/harness.py score --fixtures UserData/CacheData/audio_validation/public-v1 --observations UserData/LogData/audio_validation-worker-v1.json
```

Add `--include-voice` only to run selected voice synthesis; it does not replace actual receiver listening checks. `score` refuses unbound audio hashes, missing scenario coverage, mismatched source/scenario fingerprints, wrong translation direction, missing EOS, invalid TTS receipts and declared ASR critical cue regressions. A `PASS` label applies only to the *named observed stages*; the report always records the explicit proof ceiling. `vad_asr`, `no_speech` and `numeric_overflow` remain **UNVERIFIED** until a real offline Rust VAD/WAV-writer replay receipt exists.

To exercise the **actual canonical Rust finalizer and PCM16 writer**, without starting a microphone or changing production capture, use the explicit ignored Rust test via the pinned local runner. This requires the usual existing locked Cargo build prerequisites and a clean `Local` checkout; it is **not CI**:

```powershell
python tools/audio_validation/vad_runner.py --fixtures UserData/CacheData/audio_validation/public-v1 --source-sha <40-char-clean-Local-SHA> --output UserData/LogData/audio-validation-vad-v1.json
python tools/audio_validation/harness.py score --fixtures UserData/CacheData/audio_validation/public-v1 --observations UserData/LogData/audio-validation-vad-v1.json
```

To **combine Rust VAD evidence and real local worker output** from the exact same source SHA and byte-identical fixtures, set `--worker-observations UserData/LogData/audio_validation-worker-v1.json` and a new `--output`. The runner refuses a duplicate VAD stage or mismatched identities. The explicit Rust replay loads the pinned native Silero model/DLL and exercises the same bounded PCM CPU transport, finalizer and PCM16 writer as production. It adds a disclosed one-second quiet tail after each original WAV. A15's full 61-second tone must now be rejected as non-speech; >60-second buffer safety remains independently tested by `incoming_lane_segmentation_eviction_and_overflow_are_observable` with explicitly classified observations. Tone rejection is never overflow proof. `silero_windows` records actual model probability/gate observations; `window_gates` retains energy-only diagnostics for comparison. Neither proves physical microphone/receiver acceptance.

The Rust test has `#[ignore]` and therefore does not run in routine Rust tests. Offline input pacing uses the same bounded ring writer under a test-only blocking admission lock, then waits for CPU processing. This avoids manufacturing callback contention by feeding a whole WAV faster than the device clock. Production callbacks still use nonblocking `try_push`, with real drops counted and continuity invalidated. Receipts explicitly identify `OFFLINE_PACED_SHARED_BOUNDED_PCM_WRITER_NOT_CPAL_CALLBACK`; they prove neural/finalizer/writer behavior, never callback QoS. The replay does not exercise CPAL acquisition, outbound routing, complete production path admission or user speech quality. Unselected stages stay `UNVERIFIED`; preserve actual classification failures.

### Repeatable finalized-WAV pipeline

`worker_runner.py --finalized-pipeline` extends the same finalized-input runner through real ASR → translation → selected English voice → ASR content proxy, in one persistent worker. Incoming EN→ID omits English TTS. Add `--rounds 2` for forward/reversed scenario order (bounded 1–20); segment order within each utterance stays chronological. Choose a small critical selection with `--ids`, or a broader explicit selection using the same scenarios. No inference runs automatically on source edits or CI.

```powershell
python tools/audio_validation/worker_runner.py --fixtures <immutable-fixtures> --python EngineData/Backend/LocalWorker/WorkerRuntime/.venv/Scripts/python.exe --source-sha <clean-current-Local-SHA> --vad-observations <vad-receipt.json> --segments-dir <exported-segments> --finalized-pipeline --rounds 2 --ids A03 A04 A11 A12 A13 A14 A15 A34 --output <new-result.json>
```

When explicitly reusing older finalized WAVs, add `--vad-source-sha <exact-original-VAD-SHA>`. The runner verifies the full original scenario/audio/segment identities and records that input source separately from current worker source. Without this option the original same-SHA gate stays enforced. Old input is not fresh VAD proof. Current VAD acceptance requires a current matching VAD run; results from different revisions are never composed into full current application acceptance.

Semantic contracts live in the existing translation-quality corpus, linked by `audio_scenario_id` and checked against the canonical audio source text/direction. Its independent fingerprint is recorded in the pipeline receipt: changing semantic checks does not regenerate audio. Missing contracts stay UNVERIFIED. Ten public speech scenarios currently have declared contracts; these check concepts, negation and ordered numeric-correction cues without requiring the exact previous translation. The canonical translation evaluator validates optional `ordered_any` critical cue sequences. Different valid forms such as `15 rather than 50` can pass; reversed values cannot. These are bounded declared invariants, not proof of arbitrary semantic equivalence. New accepted linguistic forms require a reviewed contract change, never automatic baseline replacement.

Each selected scenario/round receives PASS/FAIL/UNVERIFIED and `first_wrong_stage`. Missing rounds, failed envelopes, wrong actor/output, invalid/silent audio and critical meaning regressions cannot pass. Noise must be rejected before ASR with no downstream stage. TTS content is compared with its actual translation input; differing noncritical wording in roundtrip ASR stays UNVERIFIED rather than silently passing. Overall exit is 0 only for all selected checks PASS, otherwise 2. Unselected scenarios are outside that verdict; it never means all 48 scenarios or the full application passed. Full translated/spoken text and actual model IDs/revisions, preflight, snapshots and hashes are retained in the private receipt. Successful temporary WAVs are removed after cleanup; failed synthesized WAVs are retained with hash-bound paths beside the report, and original finalized exports remain unchanged. The fixture terminates its child on cleanup; this does not test product Meeting Stop. No naturalness, native audio, sustained latency or receiver acceptance is implied.

### Explicit finalized-WAV → ASR diagnostic

Native VAD assets resolve only from the verified worker `.venv/Lib/site-packages` in repository development or the canonical flat private `PythonRuntime` package root when installed. `silero.rs` pins both model and ONNX Runtime 1.31.0 DLL sizes/hashes. Missing or mismatched assets block capture startup explicitly. The `ort` binding has binary downloads disabled; this detector never downloads models or launches another Python worker. The explicit replay checks source discontinuity reset, stale Stop isolation, joined CPU cleanup, late PCM rejection and explicit format failure. Installed layout and device timing remain separate acceptance.

Use `--segments-dir` on `vad_runner.py` to retain the actual writer outputs in a **new** directory. Each exported WAV carries its filename, SHA-256, PCM16 frame count and 16-kHz rate in the VAD observation. The runner verifies the complete exported file set before publishing it. Existing export/result destinations are refused. Without this option, temporary replay WAVs are still removed.

```powershell
python tools/audio_validation/vad_runner.py --fixtures <canonical-fixture-directory> --source-sha <clean-Local-SHA> --segments-dir .workspace/reports/finalized-segments-001 --output .workspace/reports/finalized-vad-001.json
python tools/audio_validation/worker_runner.py --fixtures <canonical-fixture-directory> --source-sha <same-clean-Local-SHA> --python EngineData/Backend/LocalWorker/WorkerRuntime/.venv/Scripts/python.exe --vad-observations .workspace/reports/finalized-vad-001.json --segments-dir .workspace/reports/finalized-segments-001 --ids A01 A14 --output .workspace/reports/finalized-asr-001.json
```

This is a bounded diagnostic of the first noise failure with a positive speech control. It deliberately preserves the failing VAD expectation; it does not bypass an application readiness gate or certify all 48 scenarios. The worker input is a hash-verified copy of the **exported finalized segment** staged temporarily under the canonical allowed CacheData input root, never the original fixture WAV. Source/audio/scenario/segment identities must match, and retained segment hashes are rechecked after worker cleanup. ASR uses the existing worker protocol with `vad_filter=False` because Rust already supplied finalized input; no second worker/model/provider is added.

The separate `translateit.finalized_asr_probe.v1` receipt distinguishes actual non-speech transcription, an explicit `asr:empty_transcript` rejection and other ASR/transport failures. It is a diagnostic receipt, not a replacement product DTO or canonical 48-case score. Full outbound status can remain blocked by an unavailable voice; only successful executed ASR preload admits this ASR-only probe. No translation, TTS, native session, receiver or production suppression is claimed. Do not use `--include-voice` with this probe. Generated public segments remain local for reproduction; private user recordings are never inputs.

The finalized probe now emits `verdict`, per-case `checks` and `PASS/FAIL/UNVERIFIED` counts. Speech is graded by the existing canonical ASR WER limit **and critical-word invariants** over chronological segment transcripts. Noise passes only an explicit empty-transcript rejection; model/transport failures or missing controls remain unverified/failed. The command exits **0 only for PASS**, otherwise **2**, so automation need not treat successful JSON publication as successful recognition. Bounded ASR error notes are retained in the private receipt to identify decoder/dependency failures. This exit contract applies to the finalized probe; the older direct-worker output remains observation-only.

Compare results from an identical fixture set:

```powershell
python tools/audio_validation/harness.py compare --baseline <scored-baseline.json> --candidate <scored-candidate.json>
```

A change of generator, WAV bytes, corpus or scenario version blocks a direct comparison rather than silently presenting unrelated measurements as an improvement.

## Proof matrix (strict)

| Layer | Source of evidence | Current status |
|---|---|---|
| L0 | Schema and case validation | Source implemented; must be executed to claim PASS |
| L1 | PCM16 mono 16-kHz WAV, SHA-256 fixture identity | Explicit offline generation/inspection |
| L2 | Canonical Rust incoming VAD/finalizer and PCM16 writer | Explicit ignored Rust test + `vad_runner.py`; no native Meeting proof |
| L3 | Real pinned worker ASR → MiLMMT → optional selected TTS | Explicit local `worker_runner.py`; never automatic |
| L4 | Windows virtual route → separate actual meeting-app receiver | NOT IMPLEMENTED / NOT EXECUTED in harness |
| L5 | Real physical microphone and ambient conditions | Native/manual acceptance still required |

A15 is a 61-second non-speech tone-rejection scenario; explicitly classified source fixtures separately exercise overflow safety. Neither proves natural long-speech ASR quality. ASR TTS synthesized read speech does not prove human accents or spontaneous Meeting comprehension. No green source test overrides these distinctions.

## Extension rules

Add scenarios to `scenarios.json` with a unique ID and explicit expectation; prefer referencing `tools/asr_quality/corpus/asr_quality_v1.json` instead of duplicating speech text. Keep the generator deterministic, provenance append-only, outputs bounded and private, and model/receiver proof independently labeled. Do not inject test audio into the production capture path or change VAD thresholds simply to pass synthetic sound fixtures. L4 automation still requires separate target-lane ownership and receiver/device evidence review.

## Deep test strategy

The full G0–G5 proof ladder, scenario boundaries and 28 planned fault cases are documented in [DEEP_ACCEPTANCE.md](DEEP_ACCEPTANCE.md) and [fault_matrix.json](fault_matrix.json). These are test specifications, not executed proof. The original 16-WAV preview is not the expanded 48-case canonical fixture set. Only generate the new set on explicit local tooling execution, preserving SHA/model identity.

## Fault execution (source-only, CI remains disabled)

The additional `fault_runner.py` executes **three** deterministic failure contracts without AI model, audio device, or GitHub Actions: F07 malformed worker protocol, F06 empty ASR transport result, and F14 stereo/truncated WAV rejection. Other planned F01–F28 cases remain explicitly `NOT_EXECUTED`; no synthetic or stub outcome is presented as a production resilience pass.

```powershell
python tools/audio_validation/fault_runner.py --output UserData/LogData/audio_validation_faults_001.json
python -m unittest tools.tests.test_audio_fault_runner
```

This uses the canonical worker wire-response parser and WAV validator but does not exercise the real worker lifecycle or model. A `PASS` means only its named offline contract executed. The fault matrix is an implementation backlog with owners, not 28 completed injections.

## Bounded worker transport fault coverage

F01–F05 use simulated queue timeout/EOF to test the same bounded wait helper as the local worker runner. This checks transport classification only, not actual child-process recovery, model imports or native UI responsiveness. F06/F07/F14 cover protocol and WAV rejection. Eight fault IDs have offline contract implementations; twenty require owner-specific injectors. Execution remains unverified until these tests actually run.

## Critical translation/TTS result gate

F08 and F10 now exercise fail-closed contract helpers for missing EOS, wrong direction, empty translation, silent WAV receipts and missing preflight. These are *offline result-envelope checks*, not real model quality or selected-actor identity tests. Ten cases have offline contract code, eighteen require owner-level fault injectors. Run manually with `python tools/audio_validation/fault_runner.py` when an appropriate checkout exists; CI stays disabled.

## Canonical prompt isolation and isolated process faults

F09 now invokes the *actual pinned MiLMMT source prompt builder* on reordered public utterances and checks that the current request contains no previous sentence. This is a real source-function test, not executed model inference. F02 uses a real short-lived **disposable Python child** that exits before replying; F01/F03/F04/F05 still exercise bounded queue timeout/EOF classification, not a real AI worker crash. Eleven fault cases have offline implementations, seventeen remain owner-specific plans. No Windows behavior is inferred.

## Pre-local-test source hardening

The Python worker dispatcher emits `stage=worker_error` on a raised handler exception and retains the original request ID. The harness now classifies this envelope as an explicit unsuccessful stage (with a `worker:handler_failed:...` blocker), not as corrupt transport, while still refusing a forged successful `worker_error`. This behavior is source-derived and protected by an authored regression; no model or Windows host test has been performed.

**Preflight boundaries before any user testing:** the local host must have an exact clean Local checkout, pinned Python/Cargo, real model assets and permitted output paths. A source-only contract result cannot certify native Meeting reception. The canonical fixture manifest/hash identity is required; a preview ZIP of 16 WAVs cannot substitute for the 48-case fixture set. Actual latency, power, device ownership and receiver results remain unknown until executable evidence exists.

**Generation hardening:** `generate` now enforces the doctor gate automatically (an explicit separate doctor command is useful for diagnosis, but not required for safety). The required ID/EN voices must exist; existing fixture outputs are immutable and no partial synthesis directory is published as accepted.
