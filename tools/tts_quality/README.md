# TTS / My Voice Quality

This directory owns source-level regression evaluation for TranslateIT's selected English Meeting voice.

It does **not** claim audible quality from CI. Real evidence requires generated WAVs from the exact selected voice/runtime and an independent intelligibility/artifact review process.

Before native My Voice reference evaluation, the build path rejects clearly unusable guided recordings: non-canonical WAV format, extreme silence/clipping, very low active signal, large DC offset, and extreme level mismatch across accepted takes. Build readiness also requires at least one accepted 3–10 second take suitable for the bounded reference contract. Reference selection prefers the cleanest eligible 3–10 second take and uses distance from the 5-second target only as a later tie-breaker. Candidate selection rejects gross synthesis artifacts first (including clipping, near-total silence, and multi-second internal dropout), then compares held-out ASR intelligibility and speaker similarity; user approval requires every held-out preview to be listened through, a completed reviewable build state, and an exact evaluation-to-candidate identity match. Successful approval also clears disposable build/evaluation artifacts while retaining accepted source takes for an optional rebuild. These remain conservative quality gates, not substitutes for native listening quality.

## Native zero-shot migration acceptance

The migration milestones below are historical source checkpoints. Current Local source is native-only; there is no default legacy inference/training fallback. Create/Improve requires the reviewed native assets and independent verifier pins, and native acceptance remains held.

The maintainer requires My Voice to preserve or improve the resemblance to the authorized user's real voice, not merely reduce build time or remove Python. Native Quick Preview is only an early listening aid: it uses the same 3–10 s reference eligibility, gross signal thresholds and clean-reference ranking as the former Quick Preview, with quiet-edge energy compared only when all eligible takes have it. It does not train or approve a profile.

For Create/Improve, retain the authorized saved takes, freeze exact source identity, shortlist a small bounded set of clean references, and compare actual native synthesis over the same eight held-out review lines. Reuse an independent speaker verifier (not the TTS conditioning embedding), an ASR intelligibility measurement, and artifact checks. Require explicit listening/approval bound to the exact reference hash, model/projector identity and all preview hashes; never reuse a GPT-SoVITS approval or write over its checkpoints. Compare old approved voice and native candidate on the existing 30-case corpus with the same independent evaluation method: no new critical-word/artifact failure, no material intelligibility or speaker-similarity regression, and user-confirmed naturalness/resemblance. Track build wall time, Meeting latency and peak VRAM separately. If no candidate meets quality acceptance, retain the old approved profile unchanged and keep cutover blocked; never auto-select another voice. Native speaker-verifier parity and full review/approval are not implemented by Quick Preview. The explicitly enabled candidate build blocks full Create/Improve until that review owner is implemented; the default legacy build preserves the existing workflow.

The shared Rust `my_voice_reference` module now owns reference ranking and the existing gross synthesis gate. Preview requests one winner; the future native evaluation path can request up to three eligible references without retaining all PCM or changing the legacy ranking. Source-only fixtures cover bounds, tie order, missing edge evidence and cancellation; no quality or native execution is claimed.

The source-only native evaluation core now uses one exclusive runtime scope for all shortlisted references and eight held-out lines, with exact source-WAV hashes, cached independent original-voice embeddings, common-anchor scores, direct PCM English ASR and ASCII WER. Gross artifact candidates are rejected; incomplete model/verifier/ASR work aborts. It keeps only the best complete candidate PCM and does not create a review manifest or approve a voice. The current native Create/Improve gate remains closed. A reviewed verifier pin, matched baseline and target quality evidence are still required.

Native evaluation now scores the exact canonical PCM16 review WAV after encoding with the existing writer's encoder. The source-only native review package stages the exact original reference plus eight WAVs and a manifest binding the DLL closure, verifier pin, anchor identities, text and scores into one SHA-256 review identity; manifest publication is separate from file staging and remains unconnected to public approval. No personal model weights are required. Legacy and native Create/Improve gates remain unchanged.

The explicit native candidate now connects Create/Improve to the existing authorized build and frozen dataset/receipt lifecycle. Review status is completion-bound; playback verifies frozen source and shortlisted anchor hashes. It does not run the Python hardware probe or training worker. Native approval remains disabled until saved profile and Meeting selection are coherent; no runtime quality or performance claim is made.

Native approval now verifies listening confirmation, completed review, frozen source and all WAV hashes before saving the exact review manifest and reference into the existing immutable VoiceVersions store. Backup verification understands the native two-file payload; no weights or preview audio are retained in the permanent version. This is not Meeting activation or proof of baseline quality parity.

Native candidate version selection now uses the existing selected MyVoice directory with an immutable version pointer. Meeting preparation verifies exact approved native profile/reference hashes and current model identity before binding, with no legacy or built-in fallback. Selecting a version is separate from approval. Source wiring does not establish runtime or baseline quality parity.

Native built-in selection now requires only the pinned shipped reference WAV and the existing selection marker, not copied GPT/SoVITS weights. The legacy release payload remains explicitly held for native candidate packaging; no Python retirement is claimed until the exact native assets, local target quality and legacy compatibility are verified.

Maintainer direction supersedes the temporary dual-build switch: Windows Local is native-only for the active voice path. The Python Quick Preview child and its packaging references are removed. Remaining retired training/worker source is still scheduled for physical deletion after consumer cleanup; the historical baseline is available from Git history, not a product fallback.

The Rust My Voice owner no longer contains the legacy trainer launch, GPU Python probe, child lifecycle or checkpoint review parser. Native creation uses the existing generation and completion receipt; legacy storage recovery is protective, not an inference fallback. Remaining Python files and release declarations still require physical consumer cleanup.

The retired Python training modules, resume probes and legacy preview helpers are removed from the active tree and package declarations. Historical baseline code remains accessible in Git history. The independent offline ONNX export is retained for verifier parity; no runtime Python trainer is required.

## Independent native verifier — exact legacy metric

The old GPT-SoVITS build calls `SV.compute_embedding3`, using ERes2NetV2 **baseWidth=24, scale=4, expansion=4** with the pinned `pretrained_eres2netv2w24s4ep4.ckpt` SHA-256 `4f5a0bf73c61eb41b174e1bb54e7ee3c83233892be8e0af1f187024e8e581a35`. Its `forward3` output is 20,480 values, not the conventional 192-dimensional embedding. Source: [pinned SV](https://github.com/RVC-Boss/GPT-SoVITS/blob/d523079fc05d9a8028d6085bffe4a2757c32abb6/GPT_SoVITS/sv.py), [forward3](https://github.com/RVC-Boss/GPT-SoVITS/blob/d523079fc05d9a8028d6085bffe4a2757c32abb6/GPT_SoVITS/eres2net/ERes2NetV2.py), and [fbank](https://github.com/RVC-Boss/GPT-SoVITS/blob/d523079fc05d9a8028d6085bffe4a2757c32abb6/GPT_SoVITS/eres2net/kaldi.py). The [public sherpa export](https://github.com/k2-fsa/sherpa-onnx/blob/7a9243056d334c73b6d1f8890cf257ba94f0f328/scripts/3dspeaker/export-onnx.py) exports standard forward and applies global-mean normalization; it is not a metric-equivalent substitution.

`export_legacy_speaker_onnx.py` is offline artifact-preparation source only, verifies exact upstream source blobs and legacy checkpoint SHA, exports FP32 forward3 and authors synthetic variable-length graph parity oracles. No downloads or execution in source development. The Rust evaluator under `voice_lab_build/native_speaker.rs` implements bounded legacy fbank, validated fixed-dimensional cosine and CPU ONNX inference using the existing pinned Silero ORT environment. It does not use ZONOS3 conditioning embeddings or create a new provider. CPU verifier is deliberately evaluation-only; CUDA remains the production TTS path. Rust feature/resampler parity, export graph and actual output identity still need authorized artifact preparation and target validation, including FP16-vs-FP32 score differences; no compatibility claim. Never use current runtime paths as hash authority. Reviewed export SHA-256 and byte count must be supplied at build time via `TRANSLATEIT_SPEAKER_ONNX_SHA256` / `TRANSLATEIT_SPEAKER_ONNX_BYTES`; absent pins fail closed. Target file `Voice/ModelData/SpeakerVerifier/eres2netv2-forward3-f32.onnx`; existing ORT DLL location remains transitional (Python package directory) until native payload staging. The ignored target fixture `exact_legacy_graph_and_rust_frontend_parity` consumes the synthetic F32LE oracles from the explicit `TRANSLATEIT_TEST_SPEAKER_ORACLES` directory; the artifact-preparation owner must verify their receipt hashes first. Proposed FP32 fbank and embedding parity bounds are conservative gates, not measured tolerances. Reuse one embedding per accepted take/utterance and compute advisory median peer cosine without repeated inference. Compare old and native output against separate authorized reference takes using the same verifier; no new biometric data is persisted. No full Create/Improve or personal profile approval is enabled by this adapter alone.

## What it measures

The evaluator combines four independent signals:

- speaker similarity supplied by the exact voice-evaluation runtime;
- intelligibility WER from an independently captured transcript of the generated speech;
- declared critical word/meaning preservation;
- explicit artifact flags: clipping, dropout, repetition, truncation, unexpected silence, noise burst, or unstable pitch.

A higher speaker-similarity score does not override an intelligibility or artifact regression. If every bounded reference candidate produces a gross clipping/silence artifact, the build fails closed instead of offering a "least bad" candidate.

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

Optional native performance evidence: include `timing_context` at bundle level with lowercase SHA-256 digests for `model_sha256`, `projector_sha256`, `reference_sha256`, `environment_sha256`, `settings_sha256`, `build_sha256` and a nonnegative integer `warmup_count`. Retain the exact local identity manifests: environment covers GPU/PCI/driver, CPU/OS and relevant runtime conditions; settings covers synthesis configuration, measurement boundary, warm-up procedure, and other controlled inputs. Build digest identifies the exact runtime closure, including native DLL. Model, reference, and environment values must come from actual evidence; never invent hashes.

Each case additionally records `elapsed_ms` (native owner handoff-to-reply), `sample_count` (complete PCM before WAV encoding), `sample_rate_hz` (44,100), unique `request_id` from the successful synthesis receipt, and the observed one-based `execution_index` in corpus order after the declared warm-up. Do not infer execution order from JSON row order, use failed attempts, or substitute whole Meeting latency. The evaluator reports first measured case, nearest-rank P50/P95, maximum and aggregate RTF only for complete valid receipts with matching corpus fingerprint and nonempty source identity. Comparison exposes both summaries and `native_timing_comparable_on_declared_context` only when all context fields except build digest match. Missing/invalid timing leaves the summary null; historical quality-only reports remain evaluable. These are declared metadata consistency checks, not proof of actual execution or speedup. Capture and native acceptance remain separate.

Promotion comparison uses whole critical tokens/phrases (dotted numeric versions and signs remain intact), not substrings. Existing declared alternatives are preserved; no guessed spoken-number equivalences or corpus changes. Newly missing literals/concepts and newly introduced artifact types are regressions even in already-failing cases. A separate `promotion_safe_on_quality_metrics` gate requires complete matched evidence and no decrease in reported mean speaker similarity or increase in reported mean WER at existing report precision. Per-case metric regressions are reported separately. This is a conservative zero-regression policy until an evidence-based tolerance is approved, not a perceptual threshold. Both gates are required for release evidence; old comparison/readiness reports without the new field cannot pass. Listening acceptance remains TARGET_WINDOWS evidence.


Repeated-synthesis cases are specifications for future native evidence. They do not claim deterministic audible stability from CI; compare multiple generated samples only when actual WAV evidence is available.
