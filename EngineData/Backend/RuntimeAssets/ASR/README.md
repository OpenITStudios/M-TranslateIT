# ASR Runtime Assets

Native Qwen3-ASR uses the pinned Q8_0 model and audio projector in `ASR/ModelData/qwen3-asr-1.7b/`. The existing model manifest owns exact filenames, revisions and hashes. Rust owns inference; no Faster Whisper fallback is admitted.

Speech presence is separate: the pinned Silero VAD and CPU ONNX Runtime resolve from `RuntimeAssets/NativeOrt/`. Native packaging and physical staging remain held. Keep model binaries out of Git.
