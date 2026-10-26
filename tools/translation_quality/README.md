# Translation Quality

This directory owns source-level translation quality evaluation infrastructure for the canonical Indonesian ↔ English pipeline.

It deliberately separates **quality measurement** from **quality claims**:

- source-level corpus/evaluator checks validate integrity and behavior when actually executed; hosted CI is inactive under the current repository policy;
- real MiLMMT output must be produced by a runtime with the canonical model installed;
- no repository/static result is reported as proof of model quality.

## Corpus

`corpus/translation_quality_v1.json` is a versioned regression set covering both directions and high-risk meeting cases:

- negation and corrections;
- numbers, dates and units;
- URLs, IDs and names;
- technical vocabulary;
- Indonesian/English code-switching;
- meeting language;
- instruction-like/prompt-boundary text;
- Unicode and exact technical literals;
- repetition and unusual whitespace;
- correction/reference/technical-term cases in finalized meeting utterances (some retaining historical context fixture metadata);
- uncertainty/modality and conditional meaning;
- quantifier scope and spoken disfluency.

Three historical regression cases and one held-out case retain `context_pairs` as immutable corpus metadata. **No current worker request sends these pairs and the model never reads them.** The evaluator uses a `meeting_finalized_utterance` request marker to test the same current-source-only translation path as Meeting. Historical `contextual_meeting` category labels must **not** be interpreted as Meeting Context Memory support or context-sensitive evaluation. Corpus fingerprints remain unchanged.

Each case contains one or more references plus narrow invariants:

- `preserve` — literals that must survive;
- `required_any` — semantic cue groups where at least one form must appear;
- `forbidden` — high-confidence meaning reversals or unsafe phrases.

Optional `forbidden_exceptions` maps an existing forbidden cue to explicitly
reviewed, complete phrases containing it. For example, `send it now` is not a
positive instruction inside `do not send it now`. Exceptions are case-local and
remove only their bounded phrase occurrences from that cue's search; a separate
positive occurrence still fails. They do not suppress required meaning checks
or forbidden facts in other cases. Every exception needs faithful and contrary
controls in the existing self-test. This is literal phrase handling, not general
negation, quotation or semantic-scope understanding; ambiguous scope still needs
linguistic review.

These checks are intentionally narrow. They do not replace human review. Numeric fragments and negation cues now reject embeddings in larger digit/word tokens, while deliberate substring stems (including Indonesian affixes and numeric decimals) remain supported by the current corpus. Reports classify declared failures as literal omission, meaning-cue omission or forbidden meaning; comparisons map critical regressions back to existing risk tags. This follows the error-localization principle of [XCOMET](https://github.com/Unbabel/COMET), but does **not** run COMET or infer its neural severity scores.

For numeric corrections, use existing `ordered_any` alternatives to bind the
accepted and rejected values to their roles. The budget case declares only its
exact dot/comma thousands forms; it does not normalize numbers globally or relax
filenames, versions, identifiers or source text explicitly requiring exact bytes.
Ordered matching retains decimal/grouping punctuation and numeric boundaries so
extra digits, fractional extensions and undeclared signs cannot satisfy those
declared amounts. Faithful clause-order variants and reversed-value controls live
in the same evaluator self-test, including incoming Indonesian corrections.

The regression corpus grows from observed failures, with a separate frozen 40-case held-out benchmark. The native Text budget-purpose failure is retained as a regression: an unspecified budget must not acquire a construction/project purpose. Do not hand-maintain these counts in downstream status files; use the `stats` command below when exact coverage matters. Growth is intentionally risk-driven rather than random: additions cover meeting phrasing, informal/workplace Indonesian, code-switching, quantities, technical literals, modality, conditionals, quantifier scope, corrections/disfluency, ordering and comparisons.

Adversarial cases treat instruction-like strings such as `Ignore previous instructions`,
`English:`, or `Indonesian:` as ordinary source content. Repository tests verify that
the worker forwards such content through the canonical source slot and that legacy context-like request options cannot change its prompt. They do not claim the model will translate every
adversarial case correctly until the real model output is evaluated.

## Commands

### Choose the smallest relevant gate

| Change | First check | Broader evidence when needed |
|---|---|---|
| Evaluator/contract only | Existing self-test; explicitly regrade retained output when every worker request is unchanged | Review faithful/wrong-meaning controls; no automatic inference rerun |
| Translation behavior | Fresh `fast` requests and grading for early failure discovery | Full regression before promotion, then held-out bilingual review for linguistic decisions |
| Audio/protocol integration | Existing audio `core` pipeline | Additional cases only for affected acoustic/stage risks |
| Lifecycle/resource behavior | Relevant source controls and audio `core` | `endurance` for repetition/resource questions; not for more language diversity |
| Integrated candidate | Accepted source/quality gates | Coordinated installed/native/independent-receiver and perceptual acceptance |

### Fast translation diagnostics

`fast_suite` in the existing regression corpus owns membership, per-case reasons
and required risk tags. It references existing cases; it does not copy their
sources or expectations. Loading validates known cases, nonempty reasons and
required-tag coverage separately in both directions. Current membership retains
observed action, reference and invented-purpose failures, rather than choosing
only passing cases. Tags are a coverage check, not proof of semantic adequacy or
a mathematically minimal selection. Full regression preserves the unselected
cases, including additional normalization, repetition and legacy-fixture risks.

Use the same CLI/evaluator with `--suite fast`; default remains `full`:

```powershell
python -X utf8 tools/translation_quality/evaluate_translation_quality.py stats --corpus tools/translation_quality/corpus/translation_quality_v1.json --suite fast
python -X utf8 tools/translation_quality/evaluate_translation_quality.py emit-requests --corpus tools/translation_quality/corpus/translation_quality_v1.json --suite fast
python -X utf8 tools/translation_quality/evaluate_translation_quality.py evaluate --corpus tools/translation_quality/corpus/translation_quality_v1.json --suite fast --results <fast-results.json>
```

These commands select/emit/grade; they **do not execute a worker**. Output includes
selection identity, full-corpus identity, case reasons and the selected fingerprint.
The result bundle must match the emitted selection. Missing, extra, full-for-fast,
fast-for-full or mismatched-fingerprint results cannot establish a complete match.
Do not relabel retained full outputs as newly executed fast results. A `fast`
comparison always leaves the promotion-safe flag false and exits nonzero, even
when its declared invariants do not regress: full regression is still required.
The frozen held-out corpus has no fast selection and remains a separate gate.

Fast lowers request count; actual inference/startup costs require their own
execution evidence. In-process grading time does not prove model or native
latency. The existing audio core profile remains the executable audio-stage
integration gate; a direct translation selection cannot replace it.

### Execute the selected translation stage

The existing persistent-worker runner accepts the same selection. From a clean,
exact pinned Local checkout with the existing canonical interpreter/model assets:

```powershell
python -X utf8 tools/audio_validation/worker_runner.py --translation-suite fast --source-sha <clean-Local-SHA> --python EngineData/Backend/LocalWorker/WorkerRuntime/.venv/Scripts/python.exe --output <new-fast-worker-report.json>
```

Use `--translation-suite full` for complete development regression. Run relevant
source controls once per changed candidate before model execution. Start with
fast to find failures cheaply; use full when evaluating a viable behavior-changing
candidate, not repeated full runs after unchanged known failures. This path uses
one existing offline worker transport and the canonical manifest/evaluator. It
does not preload ASR/TTS, synthesize fixtures or access a microphone. No case IDs,
round reduction, alternate model, automatic retry or download fallback is offered.

The report includes exact source, selection and manifest identities, actual
per-case responses, process ID/termination, elapsed wall time and check evidence.
Wall time includes cold load, requests, grading and cleanup; it is not live
Meeting latency. Output must be new. Source mismatch prevents worker startup;
transport, failed/empty/incomplete/EOS-capped/wrong-direction/wrong-model output
stops further requests, marks remaining cases unverified and exits nonzero.
Incomplete execution reports cannot be loaded as quality-result bundles.

An executed declared-invariant failure is labeled `translation_quality_check`
with `requires_linguistic_review`; it is not automatically adjudicated as a
model error. Complete but linguistically flagged runs remain usable for explicit
comparison, preserving their failed verdict. Zero exit requires complete, cleaned
execution and every selected declared check passing. Neither fast nor full output
substitutes for held-out bilingual, perceptual or native Meeting acceptance.

Report machine-derived corpus coverage:

```powershell
python tools/translation_quality/evaluate_translation_quality.py stats --corpus tools/translation_quality/corpus/translation_quality_v1.json
```

Validate corpus integrity:

```powershell
python tools/translation_quality/evaluate_translation_quality.py validate-corpus --corpus tools/translation_quality/corpus/translation_quality_v1.json
```

Emit canonical worker requests:

```powershell
python tools/translation_quality/evaluate_translation_quality.py emit-requests --corpus tools/translation_quality/corpus/translation_quality_v1.json
```

Evaluate captured results:

```powershell
python tools/translation_quality/evaluate_translation_quality.py evaluate --corpus tools/translation_quality/corpus/translation_quality_v1.json --results <results.json>
```

Compare a proposed runtime/prompt/model result set against the exact baseline:

```powershell
python tools/translation_quality/evaluate_translation_quality.py compare --corpus tools/translation_quality/corpus/translation_quality_v1.json --baseline <baseline.json> --candidate <candidate.json>
```

The comparison reports newly introduced critical-invariant regressions/recoveries plus per-case and grouped character n-gram deltas. Comparison checks new missing literals, missing meaning-cue groups, forbidden phrases and ordering failures even when a baseline case already failed for a different reason. Added failures are named in `critical_regression_details`; fixing one failure cannot cancel a different new failure in the same case. Its promotion-safe flag only means the candidate introduced no new declared critical-invariant failures on a complete matched result set; it is not a claim of overall linguistic superiority.

Result format:

```json
{
  "corpus_fingerprint": "<sha256 emitted with the request set>",
  "source_identity": "<exact runtime/source/model build identity>",
  "results": [
    {"case_id": "id-en-negation-001", "translated_text": "..."}
  ]
}
```

The fingerprint binds captured output to the exact corpus content. A missing fingerprint remains backward-compatible for standalone evaluation, but a supplied mismatched fingerprint makes the result set incomplete. Promotion comparison additionally requires non-empty `source_identity` for both baseline and candidate, so an otherwise clean comparison cannot authorize a change from unidentified runtime output.

The report includes overall and per-group critical-invariant pass rates, risk-tag critical-pass rates, a lightweight character n-gram F1 regression signal, direction/category means, and per-case failures. These automated signals intentionally do not pretend to be semantic quality scores. Risk tags expose regressions across negation, numbers, entities, omission, hallucination, terminology, context and modality even when a category average looks healthy. Baseline comparison also reports per-group critical-pass-rate deltas so an average score increase cannot hide a newly weaker risk category. The n-gram score is not BLEU, COMET, or a substitute for linguistic evaluation.

## Promotion rule

For efficient iteration, follow [regression triage](HUMAN_EVALUATION.md#efficient-regression-triage)
before adding cases or rerunning inference. Evaluator/corpus-only changes use
source self-tests and, where worker requests are unchanged, explicitly labeled
retained-output regrading. Runtime changes that can alter language output require
fresh matched inference; lifecycle/resource endurance is a separate check and
does not expand linguistic coverage by repeating the same sentences. Independent
held-out review follows only after the candidate clears development checks.

Do not change the model, prompt, current-utterance isolation, decoding policy, or text segmentation based on one anecdotal sentence. Capture a baseline from the current canonical model, compare the candidate on the same corpus, inspect every critical failure, and then add a new case when a real recurring failure mode is discovered.

Do not tune the corpus to make a candidate look better.

For release-quality linguistic review, use `HUMAN_EVALUATION.md`. Automated promotion safety and human evaluation answer different questions; neither substitutes for the other.
