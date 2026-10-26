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
