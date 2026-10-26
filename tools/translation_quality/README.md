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
