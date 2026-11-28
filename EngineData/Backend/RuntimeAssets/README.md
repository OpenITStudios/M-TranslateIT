# RuntimeAssets

Controlled local native runtime and release inputs. Application orchestration stays in Rust. This layout describes the source target; physical staging, reviewed native package closure and release remain HELD.

## Native layout

```text
RuntimeAssets/
├─ ASR/ModelData/qwen3-asr-1.7b/
├─ Translation/ModelData/hy-mt2-1.8b/
├─ NativeCuda/
│  ├─ bin/                         # reviewed private DLL closure
│  └─ THIRD_PARTY_NOTICES.txt
├─ NativeOrt/
│  ├─ onnxruntime.dll              # existing pinned CPU ORT
│  ├─ silero_vad_v6.onnx           # existing pinned speech-presence model
│  └─ THIRD_PARTY_NOTICES.txt      # reviewed ORT/VAD provenance required
├─ Voice/
│  ├─ BuiltInVoices/               # tracked references and attribution
│  └─ ModelData/
│     ├─ zonos3-mini/
│     └─ SpeakerVerifier/eres2netv2-forward3-f32.onnx
├─ AudioProvider/VBCABLE/Package/
└─ ThirdPartyNotices/THIRD_PARTY_NOTICES.txt
```

The existing `LocalWorker/WorkerRuntime/model_manifest.json` owns model inventory and downloadable GGUF identity. The native loader owns build-supplied DLL pins. The independent speaker-verifier ONNX export and its exact build pins are pending; the checkpoint hash is not an ONNX hash.

## Native ORT/VAD cutover

The same controlled `NativeOrt` root is used for verified repository development and installed runtime. The VAD and independent speaker scorer share one ORT environment. No application runtime lookup in PythonRuntime, developer site-packages, PATH or Faster Whisper remains in this owner.

For the later authorized local integration/staging phase, reuse only the exact bytes of the already pinned `onnxruntime/capi/onnxruntime.dll` and `faster_whisper/assets/silero_vad_v6.onnx` from the previously reviewed input, placed under `NativeOrt` with the filenames above. Their required sizes and SHA-256 are compiled in `src-tauri/src/engine/audio/silero.rs` (19,288,928 and 1,245,151 bytes respectively). Native admission still verifies these pins. Do not copy an arbitrary installed version, add a fallback or remove the old development environment automatically. Missing native assets block VAD. This source change does not stage any bytes, establish the ORT DLL dependency closure, or prove native execution.

## Release and updater boundary

The historical R3 v2 builder, staging, Python optimizer, license staging and installer/update helper are held. The intended native package excludes production Python/Torch/Transformers/Faster Whisper/GPT-SoVITS runtime; offline export tools and retained historical data are separate. A future native package schema must bind the exact DLL, model, verifier, reference and notice closure, and reject a historical installed payload for an incompatible app-only update. Preserve transactional replacement, rollback, user data and shared VB-CABLE driver. No release permission or local proof is implied here.

## Provenance

Built-in Male/Female references remain CC-BY-4.0 LibriSpeech/OpenSLR SLR12 material; exact reference provenance is in `Voice/BuiltInVoices/SOURCES.json`. Native CUDA, ORT, Silero, GGUF models, independent speaker export and audio-provider redistribution require their reviewed license/provenance closure. VB-CABLE notice presence alone is not redistribution permission.

Large runtime bytes and generated notices are local-only. Do not commit them or embed them in Setup resources. No first-use core-model downloads or production Python fallback.
