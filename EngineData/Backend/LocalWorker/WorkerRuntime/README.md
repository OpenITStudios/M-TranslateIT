# Backend model inventory and developer acquisition

The Windows product runtime is native Rust/CUDA: Qwen3-ASR, Hy-MT2 (ID ↔ EN), and ZONOS3-Mini reference-conditioned synthesis. Independent My Voice speaker scoring uses the existing CPU ONNX Runtime; its exact reviewed offline export pin remains pending. No production Python inference or personal model training remains.

## Source ownership

- `model_manifest.json`: required release inventory and pinned downloadable model identities.
- `prepare_model_assets.py` / `prepare_model_assets_core.py`: developer-only pinned Hugging Face acquisition. A plan performs no downloads. Existing exact snapshots are reused after fresh file/hash verification; downloaded snapshots are verified before atomic directory promotion. No post-promotion marker rewrite.
- `pyproject.toml`, `uv.lock`, `.python-version`: retained historical dependency intent/lock while the held package contract is replaced. They are not the native product runtime and are not approval to install the legacy dependency set.
- `run_realtime_worker_smoke.ps1`: historical harness, not native runtime proof.

The pending independent speaker verifier is not Hugging Face-acquirable by this tool. Do not fabricate its pin, export or quality evidence.

## Release boundary

Legacy staging, payload build, optimization and Python license staging are held. The native package must use an exact reviewed artifact closure, including runtime DLLs, models, built-in voice attribution, ONNX Runtime/verifier and audio-provider assets. Historical user data and offline developer/export tooling remain separate. No release or native proof is implied by this source layout.

See [native inference source](../../../../tools/translation_quality/native_cuda/README.md) and [active development status](../../../../planning/development.md).
