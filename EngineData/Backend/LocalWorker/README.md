# Backend LocalWorker

This directory retains the model inventory and developer acquisition tools, not a production Python worker. The Windows product source uses one Rust-owned native CUDA runtime for Qwen3-ASR, Hy-MT2 and ZONOS3-Mini selected voice synthesis. My Voice uses an approved reference and independent CPU ONNX speaker evaluation; no personal model training or alternate TTS engine.

## Ownership

- [WorkerRuntime](WorkerRuntime/README.md): current model inventory, pinned asset acquisition and retained historical developer lock.
- [Native inference](../../../tools/translation_quality/native_cuda/README.md): runtime ABI, lifecycle, synthesis and proof limits.
- [RuntimeAssets](../RuntimeAssets/README.md): controlled local assets.
- [Active development](../../../planning/development.md): native package holds and acceptance.

Legacy Python/GPT-SoVITS runtime descriptions and assets are historical; see [the previous revision](https://github.com/OpenITStudios/M-TranslateIT/blob/395d25d9fb0b678672cb8fc65b11ab7895f0f70d/EngineData/Backend/LocalWorker/README.md) for migration provenance. Preserve existing user data and storage identifiers. Do not install retired dependencies as a product runtime or infer native readiness from source or model presence.
