# TTS / My Voice Quality

This directory owns source-level regression evaluation for TranslateIT's selected English Meeting voice.

It does **not** claim audible quality from CI. Real evidence requires generated WAVs from the exact selected voice/runtime and an independent intelligibility/artifact review process.

Before expensive My Voice training, the build path rejects clearly unusable guided recordings: non-canonical WAV format, extreme silence/clipping, very low active signal, large DC offset, and extreme level mismatch across accepted takes. Build readiness also requires at least one accepted 3–10 second take suitable for the canonical GPT-SoVITS reference contract. Reference selection prefers the cleanest eligible 3–10 second take and uses distance from the 5-second target only as a later tie-breaker. Candidate selection rejects gross synthesis artifacts first (including clipping, near-total silence, and multi-second internal dropout), then compares held-out ASR intelligibility and speaker similarity; user approval requires every held-out preview to be listened through, a completed reviewable build state, and an exact evaluation-to-candidate identity match. Successful approval also clears disposable build/evaluation artifacts while retaining accepted source takes for an optional rebuild. These remain conservative quality gates, not substitutes for native listening quality.

## Native zero-shot migration acceptance

The maintainer requires My Voice to preserve or improve the resemblance to the authorized user's real voice, not merely reduce build time or remove Python. Native Quick Preview is only an early listening aid: it uses the same 3–10 s reference eligibility, gross signal thresholds and clean-reference ranking as the former Quick Preview, with quiet-edge energy compared only when all eligible takes have it. It does not train or approve a profile.

For Create/Improve, retain the authorized saved takes, freeze exact source identity, shortlist a small bounded set of clean references, and compare actual native synthesis over the same eight held-out review lines. Reuse an independent speaker verifier (not the TTS conditioning embedding), an ASR intelligibility measurement, and artifact checks. Require explicit listening/approval bound to the exact reference hash, model/projector identity and all preview hashes; never reuse a GPT-SoVITS approval or write over its checkpoints. Compare old approved voice and native candidate on the existing 30-case corpus with the same independent evaluation method: no new critical-word/artifact failure, no material intelligibility or speaker-similarity regression, and user-confirmed naturalness/resemblance. Track build wall time, Meeting latency and peak VRAM separately. If no candidate meets quality acceptance, retain the old approved profile unchanged and keep cutover blocked; never auto-select another voice. Native speaker-verifier parity and full review/approval are not implemented by Quick Preview. The explicitly enabled candidate build blocks full Create/Improve until that review owner is implemented; the default legacy build preserves the existing workflow.

## What it measures

The evaluator combines four independent signals:

- speaker similarity supplied by the exact voice-evaluation runtime;
- intelligibility WER from an independently captured transcript of the generated speech;
- declared critical word/meaning preservation;
- explicit artifact flags: clipping, dropout, repetition, truncation, unexpected silence, noise burst, or unstable pitch.

A higher speaker-similarity score does not override an intelligibility or artifact regression. If every bounded training checkpoint produces a gross clipping/silence artifact, the build fails closed instead of offering a "least bad" candidate.

## Corpus

`corpus/tts_quality_v1.json` contains 30 English Meeting synthesis cases covering meeting language, technical terms, acronyms, names, numbers/dates, questions, lists, negation/corrections, longer speech, repeated-synthesis stability and punctuation/prosody challenges.

## Commands

Validate the corpus:

```powershell
python tools/tts_quality/evaluate_tts_quality.py validate-corpus --corpus tools/tts_quality/corpus/tts_quality_v1.json
```

Generate the exact fixture plan:

```powershell
python tools/tts_quality/evaluate_tts_quality.py fixture-plan --corpus tools/tts_quality/corpus/tts_quality_v1.json
```

Evaluate captured evidence:

```powershell
python tools/tts_quality/evaluate_tts_quality.py evaluate --corpus tools/tts_quality/corpus/tts_quality_v1.json --results <results.json>
```

Compare baseline and candidate:

```powershell
python tools/tts_quality/evaluate_tts_quality.py compare --corpus tools/tts_quality/corpus/tts_quality_v1.json --baseline <baseline.json> --candidate <candidate.json>
```

Result bundles use:

```json
{
  "corpus_fingerprint": "<fixture-plan sha256>",
  "source_identity": "<exact actor/runtime/build identity>",
  "results": [
    {
      "case_id": "tts-meeting-001",
      "intelligibility_text": "...",
      "speaker_similarity": 0.91,
      "artifact_flags": [],
      "wav_sha256": "<64 hex>"
    }
  ]
}
```

Promotion comparison is fail-closed when provenance is missing/mismatched or when a new critical/artifact regression appears. It is deliberately not a standalone verdict on naturalness or speaker fidelity; listening acceptance remains TARGET_WINDOWS evidence.


Repeated-synthesis cases are specifications for future native evidence. They do not claim deterministic audible stability from CI; compare multiple generated samples only when actual WAV evidence is available.
