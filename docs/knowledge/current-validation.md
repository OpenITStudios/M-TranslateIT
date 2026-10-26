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
