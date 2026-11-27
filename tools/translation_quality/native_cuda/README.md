# Native CUDA candidate — source only

This is the staged Rust ↔ llama.cpp/mtmd ABI, not a second production engine.
It is deliberately outside the Tauri Cargo graph. The only current production
worker remains unchanged until the coordinated cutover. My Voice is excluded.

- Exact upstream: [llama.cpp 6d8a888](https://github.com/ggml-org/llama.cpp/tree/6d8a8886fa44a4a404c58ace93052ea680193981), MIT; use the upstream LICENSE in any eventual distribution.
- Apply only the reviewed adjacent `llama-strict-cuda.patch` in a dedicated checkout at that SHA. The candidate CMake file checks SHA and reverse-patch applicability; it does not prove there are no unrelated changes. No fetch, install, patch application or build is performed by this source increment.
- The candidate is one Windows x64 MSVC DLL linked to the exact static llama/mtmd/GGML closure, CUDA SM 86, no alternate acceleration backend selection or runtime download. Only the narrow C ABI is consumed by Rust. The dependency-free candidate Cargo graph does not modify the Tauri lock.
- Both the decoder and audio projector receive the exact same explicitly selected CUDA PCI device. `n_gpu_layers=-1`, no host buffer preference, KQV offload and strict graph guard are requested; actual model/KV residency, direct non-scheduler ops, WDDM spill and graph rejection require real evidence. CPU preprocessing and token selection remain ordinary host work.
- ASR uses the official [forced-language prefill](https://github.com/QwenLM/Qwen3-ASR/blob/7c6daf77a2421100f5fb066495372c00129d39ff/qwen_asr/inference/qwen3_asr.py), only current finalized PCM, no history, no external VAD, no additional audio-file conversion. Bound 300 ms..60 s, 2048 positions, batch 256, 512 generated tokens. Cap/error/empty output returns no transcript; only the reported length after EOG authorizes consumption of the caller's scratch bytes. Context and batch are reused; utterance memory resets each time. A native inference error poisons the candidate handle; the caller must dispose it, not retry in another engine.
- Hy-MT2 1.8B translation uses the same decoder owner and DLL for both directions. Its fixed user-only prompt and stop marker follow Tencent's [published prompt](https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF/blob/a0c709d9fac510f2c807aa3af52872340dc37a4a/README.md) and [single-turn template](https://huggingface.co/tencent/Hy-MT2-1.8B/blob/ef3ba04e0b2a9c49221ca008eea876ad19f84aec/chat_template.jinja). Validate exact BOS/User/Assistant/EOS token IDs and actual EOG at load against Tencent's [generation config](https://huggingface.co/tencent/Hy-MT2-1.8B/blob/ef3ba04e0b2a9c49221ca008eea876ad19f84aec/generation_config.json); reject unsupported directions, role markers, over-limit input, incomplete/capped/empty or malformed output and exact instruction-envelope echo. Greedy decoding and adaptive 64–256-token budget deliberately preserve the prior Meeting policy rather than Tencent's sampling/4096-token recommendation; quality/latency parity is a held acceptance gate, not established. One resident model for ID↔EN, separate context from ASR, reset memory per request; caller scratch reused without output serialization. Rust ABI v2 requires matching DLL.
- Candidate asset identities are recorded in the active development checkpoint. Verify the exact publisher hashes before running; architecture metadata alone is not identity proof. No production asset manifest change.

## Held local integration

Only the separately authorized integrated PC phase may configure/build or execute.
Use an already prepared, reviewed patched checkout and existing local CUDA
toolchain. Configure CMake with an absolute `LLAMA_SOURCE_DIR`; build
`translateit_native_cuda` in Release; set `TRANSLATEIT_NATIVE_LIB_DIR` to its
absolute DLL/import library directory for the candidate Rust crate. The DLL
must be in the controlled local loader directory, not an arbitrary PATH entry.
Record actual compiler/CUDA toolkit, dependency closure, patch diff, build
identity, DLL identity, model hashes and target GPU. This is not a packaging
recipe or authorization to run a test now.

ASR and translation candidate paths are authored, but native compile, linkage,
GPU execution, tokenizer/prompt parity, EOG handling, Indonesian/English quality,
cancellation/timeout, process isolation, and combined VRAM remain unverified.
Do not activate them in Meeting until these boundaries, asset admission,
lifecycle, packaging and full consumer cutover are coherent.
