# Voice Runtime Assets

This directory owns the controlled reference and native model assets for the selected English Meeting voice. Runtime orchestration is Rust-owned; there is no production Python inference, personal model training or alternate voice engine.

## Controlled layout

```text
Voice/
├─ BuiltInVoices/
│  ├─ SOURCES.json
│  ├─ MaleVoice/reference.wav
│  └─ FemaleVoice/reference.wav
└─ ModelData/
   ├─ zonos3-mini/
   │  ├─ ZONOS3-Mini-Q8_0.gguf
   │  └─ mmproj-ZONOS3-Mini-F32.gguf
   └─ SpeakerVerifier/eres2netv2-forward3-f32.onnx
```

The [model manifest](../../LocalWorker/WorkerRuntime/model_manifest.json) owns exact model revisions, artifact hashes and pending export status. The independent CPU speaker verifier is evaluation-only, not a TTS fallback; its reviewed export pin and parity remain pending. Native package staging and target quality acceptance are held. Model presence is not evidence of readiness.

## Runtime routes

Built-in Male/Female uses the pinned shipped reference without training. My Voice uses authorized recordings, bounded reference selection, independent speaker/intelligibility and artifact evaluation, listening and explicit approval. Both use the same selected reference binding and ZONOS3 synthesis on the native CUDA owner. Complete 44.1 kHz mono F32 PCM is handed to the existing Meeting playback owner; no partial playback or fallback voice.

The `VoiceLab` storage and `voice_lab_*` compatibility identifiers remain unchanged. Existing personal recordings and historical profiles are preserved. See [native inference](../../../../tools/translation_quality/native_cuda/README.md) and [TTS quality](../../../../tools/tts_quality/README.md).

## Built-in voice provenance

`BuiltInVoices/SOURCES.json` is the machine-readable authority. Current references:

```text
MaleVoice
speaker      3752
utterance    3752-4943-0003
source       LibriSpeech dev-clean / OpenSLR SLR12
license      CC-BY-4.0
wav SHA-256  1351eea8182b2f6ef00025b4e68b6a0e5184c059bd7c6d7dacc18988f2dbd200

FemaleVoice
speaker      6313
utterance    6313-66125-0007
source       LibriSpeech dev-clean / OpenSLR SLR12
license      CC-BY-4.0
wav SHA-256  81b131f23a3f4dd8dbef31082e07a957ea42bf7e7b86522c75b0e23209343423
```

These are **CC-BY-4.0**, not public-domain assets. Preserve exact source URL/reference text/utterance/hash metadata and include attribution in the canonical generated third-party notice bundle.

## Historical provenance

The previous [GPT-SoVITS/G2P/FFmpeg source and license inventory](https://github.com/OpenITStudios/M-TranslateIT/blob/2ccc01ca09560f8bf1eee793029fa4b188e6ac5e/EngineData/Backend/RuntimeAssets/Voice/README.md) remains in Git history for migration, attribution and legacy evaluation. It is not a current package instruction. The historical speaker checkpoint identity is retained by the independent verifier export contract. Preserve all applicable third-party notices; source inspection does not establish distribution clearance.

## Rules

- One local native selected-voice inference path; no silent CPU/cloud/other-voice fallback.
- No normal-user runtime downloads or retired Python dependency installation.
- Keep large models, generated audio and private voice data out of Git.
- Preserve exact reference authorization, package pins and device/quality proof boundaries.
