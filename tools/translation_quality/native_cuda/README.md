# Native CUDA inference — source integration

This Rust ↔ llama.cpp/mtmd ABI is the sole ASR/MT route in the Windows
application source. The existing scheduler and lifecycle own the pair.
Explicit preparation opens the DLL/model pair, not app startup or polling.
Python IPC carries only control/status and existing voice operations.
My Voice synthesis, training and evaluation algorithms are unchanged.

- Exact upstream: [llama.cpp 6d8a888](https://github.com/ggml-org/llama.cpp/tree/6d8a8886fa44a4a404c58ace93052ea680193981), MIT; use the upstream LICENSE in any eventual distribution.
- Apply only the reviewed adjacent `llama-strict-cuda.patch` in a dedicated checkout at that SHA. The candidate CMake file checks SHA and reverse-patch applicability; it does not prove there are no unrelated changes. No fetch, install, patch application or build is performed by this source increment.
- The candidate is one Windows x64 MSVC DLL linked to the exact static llama/mtmd/GGML closure, CUDA SM 86, no alternate acceleration backend selection or runtime download. Only the narrow C ABI is consumed by Rust. The Rust crate reuses the application's exact sha2 0.11.1; no new package/version enters the Tauri graph.
- Both the decoder and audio projector receive the same exact CUDA PCI device. ABI v7 allows explicit PCI ID in the held fixture or admits only one visible CUDA GPU with a valid physical PCI-form identity; unknown, ambiguous or virtual-suffixed identity fails closed. The actual selected PCI ID is returned with the successful open and included in the Rust-owned inference receipt. `n_gpu_layers=-1`, no host buffer preference, KQV offload and strict graph guard are requested; actual model/KV residency, direct non-scheduler ops, WDDM spill and graph rejection require real evidence. CPU preprocessing and token selection remain ordinary host work.
- ASR uses the official [forced-language prefill](https://github.com/QwenLM/Qwen3-ASR/blob/7c6daf77a2421100f5fb066495372c00129d39ff/qwen_asr/inference/qwen3_asr.py), only current finalized PCM, no history, no external VAD, no additional audio-file conversion. Bound 300 ms..60 s, 2048 positions, batch 256, 512 generated tokens. Cap/error/empty output returns no transcript; only the reported length after EOG authorizes consumption of the caller's scratch bytes. Context and batch are reused; utterance memory resets each time. A native inference error poisons the candidate handle; the caller must dispose it, not retry in another engine.
- Hy-MT2 1.8B translation uses the same decoder owner and DLL for both directions. Its fixed user-only prompt and stop marker follow Tencent's [published prompt](https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF/blob/a0c709d9fac510f2c807aa3af52872340dc37a4a/README.md) and [single-turn template](https://huggingface.co/tencent/Hy-MT2-1.8B/blob/ef3ba04e0b2a9c49221ca008eea876ad19f84aec/chat_template.jinja). Validate exact BOS/User/Assistant/EOS token IDs and actual EOG at load against Tencent's [generation config](https://huggingface.co/tencent/Hy-MT2-1.8B/blob/ef3ba04e0b2a9c49221ca008eea876ad19f84aec/generation_config.json); reject unsupported directions, role markers, over-limit input, incomplete/capped/empty or malformed output and exact instruction-envelope echo. Greedy decoding and adaptive 64–256-token budget deliberately preserve the prior Meeting policy rather than Tencent's sampling/4096-token recommendation; quality/latency parity is a held acceptance gate, not established. One resident model for ID↔EN, separate context from ASR, reset memory per request; caller scratch reused without output serialization. Rust ABI v7 requires matching DLL.
- One thread-bound Rust `Runtime` owns the ASR/MT pair and, when explicitly supplied, the native voice candidate; no independent voice open. The native owner admits one runtime per DLL process, uses the same exact CUDA device for all stages, locks and checks every selected GGUF SHA-256 hash using Windows BCrypt before model loading, then uses resolved paths and retains read handles (deny ordinary write/delete) until close. Rejects non-fixed-drive paths and reparse-point files, caps each file at 3 GiB. Requires a trusted controlled parent directory; path replacement via privileged/host filesystem operations is not proven impossible. No model bytes are downloaded. The 1 MiB verification scratch is released before loading. Hash pins are the publisher identities recorded in the active development checkpoint, not a new production inventory.
- A retained unavailable native owner blocks Ready reuse in both lifecycle modes, even if the legacy voice process still exists. Cold absence is distinct from failed retained ownership; only explicit existing startup/cleanup may recover it. Status never opens a model.
- Free VRAM admission uses device-global observations before loading, between model loads, and after both load: file sizes plus 1.5 GiB reserve initially, remaining MT size plus reserve after ASR, reserve after MT. These are conservative checks, not measured per-process peak or WDDM spill/residency guarantees. No polling or per-utterance hash. A partial open rolls back every selected model; an inference/internal error invalidates all resident stages, releases their model resources, and requires explicit close before reopen. No automatic fallback. Normal utterance failures do not unload the models.

- ABI v7 retains the synchronous caller-owned cancellation check and absolute
  monotonic deadline for each inference request. It rejects cancellation or
  expiration before work, between ASR chunks, MT prefill batches and
  generation tokens, and before publishing a complete output. It returns
  distinct cancellation/deadline status with zero output length. The
  pinned upstream abort callback is CPU-only; no claim of preempting
  CUDA kernels, a projector chunk, model loading or a blocked driver.
  Cancellation drains the decoder before clearing utterance memory; native
  inference faults still invalidate both stages. The same owning thread
  must return from the call before close; callers must fence generation
  and retain incomplete cleanup on timeout. No extra timer or scheduler.

- `RuntimeOwner` opens, executes and closes the thread-bound pair on one
  persistent Rust thread. `begin` receives owned PCM/text only after the
  existing host scheduler admits work. A single guarded handoff slot
  refuses a second active request; it is not a priority or waiting queue.
  One reusable output scratch lives on that thread. The caller holds the
  existing permit through the owned reply's `wait`; dropping an unfinished
  reply cancels and waits rather than detaching work. Wait/drop must run
  outside the host lifecycle lock. The host retains exact generation/session
  authority, signals `cancel_active` only for its matching request, and
  retains the owner if nonblocking `try_close` reports incomplete cleanup
  or a failed join. Owner Drop joins as a safety backstop, not a UI Stop
  operation. Model open is synchronous and not interruptible. The existing
  HelperBridgeRuntime now has the resident native owner slot; Stop/restart/
  shutdown share bounded cleanup and preserve Meeting ownership between
  requests. The existing scheduler now has an admitted native dispatch
  branch for a bound owner: transfer the actual owned PCM allocation, apply
  existing gain in place, borrow current Meeting translation text and language
  outside JSON until scheduler admission, then move the owned text into the native
  request, wait outside the lifecycle mutex with the same permit, then
  fence the result against exact session/generation and request ownership.
  Native ASR/MT results move the owned text directly into the existing JSON
  Value receipt; the Meeting consumer borrows that text. Native results have
  a Rust-owned outer receipt identity. Scheduler metadata is parsed once;
  native scope checks still read current session authority, moving the owned
  snapshot session ID rather than copying it. A selected
  native request never retries in Python, and a closed Meeting-native
  binding remains until verified Stop, blocking Python-only recovery.
  The host now opens the pair on explicit ASR preload or internal
  standalone translation, binds the exact Meeting scope, and rejects
  inference when the native pair is unavailable. The old Meeting WAV
  serialization and payload-cloning replay are removed from transport.
  Model downloads and native execution have not occurred.

- The import-library build script is retired. Explicit open uses the
  absolute controlled `translateit_native_cuda.dll` path, restricts the
  dependency search to its directory and System32, checks the actual
  module path and complete ABI symbol table, and pins one library identity
  for the process. Models still close per owner lifecycle.
  [Windows loader flags](https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-loadlibraryexw).
  The loader requires reviewed build-supplied SHA-256 pins for a flat private
  DLL directory, rejects extra files and reparse-point files, hashes locked
  file handles before any load, rejects detected preloaded private names,
  and retains the verified handles with the process-pinned module. The
  host binds the pins at compile time via `TRANSLATEIT_NATIVE_DLL_PINS`
  (semicolon-separated lowercase `filename.dll=64hexsha256`, including
  `translateit_native_cuda.dll`). The approved build must provide the actual
  complete private PE dependency closure, not compute trusted pins from
  untrusted installed files. System32 dependencies, controlled directory ACLs,
  delay-load/transitive imports and concurrent module loading still require
  integrated package review. The unsafe boundary is not eliminated.
  Canonical future asset paths under the verified runtime root:
  `EngineData/Backend/RuntimeAssets/NativeCuda/bin/translateit_native_cuda.dll`
  (private DLLs only); `ASR/ModelData/qwen3-asr-1.7b/` and
  `Translation/ModelData/hy-mt2-1.8b/` under RuntimeAssets, with the exact
  pinned GGUF filenames. No installed manifest is trusted as a hash anchor.
  The production owner open method now uses these paths/pins. A bound
  owner supplies ASR/MT preload receipts without loading twice; Rust
  projects its actual availability and PCI identity into the existing
  status shape while retaining Python voice capability and allocator
  observations. The functional check requires one helper generation and
  exact native PCI identity throughout, and decodes only its owned voice
  probe WAV with the existing Rust decoder/resampler. The output is
  reserved uniquely using the host preload request ID; collision fails
  closed. Cleanup removes only that request's cache file, never an
  arbitrary worker report.
  The same offline release staging transaction now acquires pinned GGUFs
  through the existing model manifest and copies only reviewed native DLLs
  from `TRANSLATEIT_NATIVE_ARTIFACT_ROOT/bin`, with the accompanying reviewed
  `THIRD_PARTY_NOTICES.txt` (license text, source/build provenance, CUDA
  redistribution review). The supplied `TRANSLATEIT_NATIVE_DLL_PINS` must
  match actual DLL bytes before staging and packaging. Setup receives the
  digest of those pins; install and app-only update reject mismatch, and
  Rust compilation consumes the same pin string. No third installer or
  first-use download. Neither real artifact input nor package has been
  produced here. The production source now selects native-only ASR/MT.
  Idle diagnostic model weights close after verification; VoiceLab
  admission releases idle native weights before GPU use. Legacy Python
  ASR/MT implementation and MiLMMT model staging are retired. The voice
  worker retains only voice/control handlers. Faster Whisper Large V3
  Turbo and CTranslate2 remain for My Voice evaluation and Silero assets.
  Offline payload and installed-runtime schema v2 reject the old layout.
  Parent names are locked during DLL admission and redirected directories
  rejected. Complete reviewed PE closure and actual installed ACLs
  remain release evidence requirements.

## Held local integration

Only the separately authorized integrated PC phase may configure/build or execute.
Use an already prepared, reviewed patched checkout and existing local CUDA
toolchain. Configure CMake with an absolute `LLAMA_SOURCE_DIR`; build
`translateit_native_cuda` in Release. The held fixture uses
`TRANSLATEIT_TEST_NATIVE_DLL` for the absolute trusted DLL path and
`TRANSLATEIT_TEST_NATIVE_DLL_PINS` for reviewed build pins, not self-reported
installed hashes. No import library or PATH entry is required.
The held ignored Rust lifecycle fixture requires explicit ASR/projector/MT GGUF
paths and CUDA PCI ID from the target; missing input is not PASS. It covers wrong
asset admission, explicit and unique visible device identity, exclusive ownership, pre-cancellation, expired deadline,
cooperative MT cancellation with no partial output, subsequent MT reuse,
owner-thread translation, close and reopen. A separate mechanical fixture
checks the one-slot admission and incomplete close without model execution.
Neither fixture is comprehensive inference or latency proof.
Record actual compiler/CUDA toolkit, dependency closure, patch diff, build
identity, DLL identity, model hashes and target GPU. This is not a packaging
recipe or authorization to run a test now.

ASR and translation candidate paths are authored, but native compile, linkage,
GPU execution, tokenizer/prompt parity, EOG handling, Indonesian/English quality,
cancellation/timeout, process isolation, and combined VRAM remain unverified.
Do not release or claim runtime acceptance until integrated source review and held Windows build/runtime proof are complete.

## Native TTS candidate — ZONOS3-Mini (explicit source candidate)

The approved Python retirement direction is one Rust-owned native Meeting pipeline, with ZONOS3-Mini as the first TTS candidate. The existing exact llama.cpp revision already provides ZONOS3 generation, built-in voice and reference embedding, and raw F32 PCM through `mtmd_helper_gen_audio_*`. Do not introduce llama-server, a second GGML runtime, Candle, or Python IPC for this candidate. The adjacent controlled source patch additionally rejects ZONOS3 context/frame exhaustion rather than reporting it as end-of-speech, and rejects non-streaming output before the end-of-audio delay completes. This is a source-side prerequisite, not an active TTS implementation or successful patch application.

The pinned helper omits speaker CFG and performs the iSTFT synthesis head on the host after codec graph execution. The controlled patch now exposes a ZONOS3-only reference bind operation; one embedding remains resident across utterance resets and is cleared on replacement or owner close. The existing strict CUDA scheduler guard covers scheduled model graphs, not all host DSP or direct operations. Verify decoder, codebook heads, speaker encoder and codec graph placement and error propagation on the exact device. Host DSP is not a model fallback. The bound embedding is not a persisted profile or a guarantee of reference quality.

Candidate artifact identities from the exact [publisher upload commit 2e7bc41](https://huggingface.co/ggml-org/ZONOS3-Mini-GGUF/commit/2e7bc4126a2343787298e1a297a3d1c4a13165aa), whose three LFS pointers are unchanged in [932e429](https://huggingface.co/ggml-org/ZONOS3-Mini-GGUF/commit/932e429e3c75b5eda8325bf9c21f6c3d2b3059e2). Apache-2.0 is declared; release notice/embedded voice provenance still needs review.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| ZONOS3-Mini-Q8_0.gguf | 854726208 | `60568efc88e2f9231f4f5c061e246421f575986642d72ab9ae68f77d6cf76be4` |
| mmproj-ZONOS3-Mini-F32.gguf | 1142502880 | `aeeb92325b4b6927bef656390b78e1246148065f947d8b59c2663557a809414d` |
| ZONOS3-Mini-BF16.gguf (comparison only) | 1605924288 | `25f9399875a874b8ed515fda04fae78064cf39467a4bd5990f29bebb7b2eca78` |

ABI v7 extends the same exclusive runtime open with an optional, exact Q8_0/F32 pair, verified before any model loads, and checks available device memory before and after the candidate load. Both paths are required together; the legacy default calls the pair-only Rust entry, while the explicit native candidate opens the voice closure. Voice opens its own context on that exact CUDA device with embeddings, reusing libmtmd's model-specific generation; no additional decoder sampler, worker, library or process. The existing RuntimeOwner binds authorized 16 kHz mono F32 reference (3–30 s) once, returns an ephemeral Rust VoiceBinding, then accepts only the current English text and that exact current handle per utterance. Rust normalizes the speech text before crossing the ABI. Invalid replacement revokes the old handle; no default speaker fallback. The reference PCM is not sent again; output is owned 44.1 kHz mono F32 PCM (complete, max 60 s). A bounded 900-step generation loop checks the existing stop/deadline at safe boundaries; no kernel preemption. Only completed, valid PCM is returned; cleanup resets utterance state and retains the model. The Rust-only normalization module covers bounded English cardinal/decimal/ordinal/currency/percent/clock/year and explicit software-version forms, plus the existing C++/C#/HTTP/API method cases. It preserves punctuation, whitespace and unrecognized technical tokens instead of guessing; dates, mixed tokens and full NeMo/upstream template parity remain unproven. The translation text and transcript are not modified. No normalization dependency or extra model call. Application-level selected profile authorization and direct PCM playback are wired in the explicit candidate. Native packaging, matched quality evidence and legacy retirement remain open. The held `voice_candidate_uses_the_existing_owner_and_returns_pcm` fixture requires the existing native acceptance inputs plus `TRANSLATEIT_TEST_VOICE_GGUF`, `TRANSLATEIT_TEST_VOICE_PROJECTOR_GGUF`, and `TRANSLATEIT_TEST_VOICE_REFERENCE_PCM` (a public, authorized 16 kHz mono F32LE reference). It checks wrong artifact rejection, same-owner binding, repeated voice PCM, invalid replacement revocation and subsequent translation; it is not a quality benchmark. No model bytes downloaded, patch applied, build or inference performed. Preserve old actor files; a GPT-SoVITS checkpoint is not a ZONOS3 speaker embedding.

### Candidate packaging boundary (Unreleased)

Native My Voice and built-in selection no longer require a Python child or personal training weights in the explicit candidate. Native built-ins use the existing `MyVoice/native-version.json` selection marker with the exact approved built-in ID and shipped reference pinned by `BuiltInVoices/SOURCES.json`; personal markers retain the immutable version ID. Legacy selections retain their original compatibility format. The R3 release staging, Tauri resource map, external payload and quality gate still require the private Python/GPT-SoVITS closure. They do not stage ZONOS3 or the independent verifier. Release/staging scripts now reject `TRANSLATEIT_NATIVE_VOICE_CANDIDATE=1` until a coherent candidate payload and target quality acceptance exist. For the later authorized local packaging lane, the required additional exact artifacts are the Q8_0/F32 pair above under `Voice/ModelData/zonos3-mini/` and the reviewed `Voice/ModelData/SpeakerVerifier/eres2netv2-forward3-f32.onnx` with independently verified `TRANSLATEIT_SPEAKER_ONNX_SHA256` and `TRANSLATEIT_SPEAKER_ONNX_BYTES`, plus verified ORT closure, native DLL pins and license/provenance. No digest, size, parity or release PASS is inferred for the pending verifier export.
