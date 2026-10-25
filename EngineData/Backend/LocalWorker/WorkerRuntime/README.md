# Backend Local Worker Runtime

This folder owns TranslateIT's single Python inference worker used by the Tauri/Rust application.

## Process

```text
realtime_local_worker.py
├─ realtime_local_worker_base.py
│  ├─ worker_runtime_common.py
│  └─ worker_io_runtime.py
└─ milmmt_translation_provider.py
```

`realtime_local_worker.py` is the only application-facing worker entrypoint. Supporting modules split responsibility without creating another process/runtime:

- `realtime_local_worker_base.py`: newline-JSON protocol, status composition, command dispatch and standalone Text orchestration;
- `worker_runtime_common.py`: paths, limits, language normalization, GPU/device probes and generic completion helpers;
- `worker_io_runtime.py`: Faster Whisper ASR and GPT-SoVITS selected-voice inference state/commands;
- `milmmt_translation_provider.py`: Indonesian ↔ English MiLMMT translation behavior.

The worker exposes:

```text
status
asr_preload
transcribe
translation_preload
translate
voice_actor_preflight
voice_actor_synthesize
```

Rust remains Meeting/session/audio and selected-voice profile authority. The Python worker does not own Windows virtual-audio routing or a second product architecture.

## Translation

```text
xiaomi-research/MiLMMT-46-1B-v1.0
revision 4fc480b6c58dec29c159dcdf9fde0f6d5c354995
one resident causal model
ID → EN and EN → ID
CUDA BF16 primary
CPU fallback = explicit degraded operation
```

Production rules:

- official Xiaomi translation prompt;
- deterministic `do_sample=false`;
- continuation-only causal decoding;
- source is never silently truncated;
- known incomplete/non-EOS generation fails closed;
- RuntimeAssets readiness requires `.translateit_model_revision` to match the pinned revision;
- no M2M100/Marian/second-translator fallback/router.

Meeting translation is context-free across turns: outbound ID→EN uses only the current finalized Indonesian utterance; incoming EN→ID uses only the current finalized English utterance. Standalone Text uses only its current input. Prior translated pairs, transcript history, and historical `context_pairs` are not supplied to MiLMMT. Transcript retention is for review/export only.

## ASR and Meeting voice

- ASR primary: Faster Whisper Large V3 Turbo.
- Optional ASR fallback: Faster Whisper Medium.
- Meeting voice inference: one GPT-SoVITS `voice_actor_*` contract.
- Built-in Male/Female: Rust prepares a selected actor profile from pinned LibriSpeech reference assets plus reviewed shared V2ProPlus pretrained weights; user training is not required.
- My Voice: optional trained GPT-SoVITS V2ProPlus actor promoted only after held-out evaluation and explicit user approval.
- Existing `voice_lab_*` Python filenames/error namespaces are legacy compatibility identifiers, not product vocabulary.

## Dependencies

```text
pyproject.toml  = dependency intent
uv.lock         = resolved dependency graph
.python-version = developer Python pin
```

Normal setup consumes the lock frozen. Do not edit dependency intent without regenerating/reviewing the same lock change.

Current dependency versions:

```text
Python       3.12.x
PyTorch      2.14.1 / 2.14.1+cu126 on Windows/Linux CUDA path
Transformers 4.57.6 exact pin
Tokenizers   0.22.2 resolved by uv.lock
Accelerate   1.15.0 resolved by uv.lock
```

Transformers 4.57.6 remains the reviewed MiLMMT runtime boundary. Updating the lock or passing source tests does not revalidate model quality, GPU coexistence or Meeting delivery.

Compatibility ceilings retained during the 2026-10-26 dependency refresh:

- The pinned GPT-SoVITS source requirements retain NumPy `<2`, librosa `0.10.2`, PEFT `<0.18` and TorchMetrics `<=1.5`. They remain explicit constraints, not claims that those packages are globally latest. SciPy and pandas are resolved to the newest versions allowed by that NumPy boundary.
- Transformers stays at the reviewed 4.57.6 MiLMMT boundary; its dependency metadata keeps Hugging Face Hub below 1 and Tokenizers below 0.23. A Transformers 5 migration needs its own model/voice acceptance and is not implied by a lockfile refresh.
- TorchAudio 2.11 is current and uses PyTorch's stable ABI, supporting PyTorch 2.11 and later ([upstream compatibility](https://docs.pytorch.org/audio/stable/installation.html)). Torch 2.14.1 keeps the existing CUDA 12.6 index; source/import tests are separate from GPU/model proof.
- Python stays at 3.12.10 because the NumPy 1.26 Windows wheels support Python through 3.12, and 3.12.10 is the final official 3.12 Windows embeddable binary. Later 3.12 security releases are source-only ([Python release policy](https://www.python.org/downloads/release/python-31215/)). This leaves a security-maintenance limitation; it does not claim that 3.12.10 includes newer source-only fixes. A newer Python requires migration of the pinned voice dependency boundary.

The other worker packages are resolved by `uv lock --upgrade`; no model/checkpoint or GPT-SoVITS source identity changes with a dependency refresh.

## PyTorch allocator diagnostic

The existing `status` response includes `gpu.torch_allocator` as a read-only,
best-effort observation. It inspects an **already initialized** CUDA context
and never imports PyTorch, initializes CUDA or preloads a model just for metrics.

- `state=not_initialized`: no initialized PyTorch CUDA allocator to inspect;
  **not** evidence that the GPU is unused.
- `state=unavailable`: inspection is unsupported or failed; an explicit
  `reason` accompanies it. This cannot change readiness or activate CPU fallback.
- `state=measured`: current PyTorch device ordinal and allocator values in
  MiB (`allocated`, `reserved`, `peak_allocated`, `peak_reserved`).
  Peaks are process-lifetime values; they are not per-turn peaks.
- `device_memory.state=measured`: CUDA-reported instantaneous device
  `free_mib`, `total_mib` and derived `used_mib`. This is **not**
  PyTorch-specific consumption and may include CTranslate2/other processes.
  Device-memory failure retains valid allocator metrics, marking the nested
  observation unavailable with a typed, redacted `reason`.

Do not subtract allocator values from device usage and label the difference as
Faster Whisper memory: CUDA contexts, libraries, WDDM, other applications and
memory accounting can overlap or differ. On multi-GPU systems, the PyTorch
current-device ordinal alone does not establish CTranslate2 physical GPU
identity. No reading is a safe training reservation, a device-wide peak, or
proof of Meeting stability.

For the eventual coordinated TARGET_WINDOWS acceptance, pair the existing
per-stage Meeting timings (`asr_ms`, `translation_ms`, `tts_ms` and
scheduler waits) with read-only worker status snapshots after cold preload,
warm readiness, sustained Live and Stop, plus the existing independent
whole-device `nvidia-smi` / Windows GPU observations. Compare the same
pinned Local SHA and GPU identity. A single status snapshot cannot attribute
GPU usage to individual stages or capture transient peaks. Do not query
the full status path on every utterance or add a background sampler.

## Developer checks

Repository-only MiLMMT validation does not require model/GPU inference:

```powershell
python tools/translation_quality/validate_canonical_milmmt_repo.py
```

With the frozen developer environment available:

```powershell
uv sync --frozen
uv run --frozen ruff check .
uv run --frozen ruff format --check .
uv run --frozen pytest
```

Target runtime smoke, when explicitly required:

```powershell
.\run_realtime_worker_smoke.ps1 -ExpectedDevice Cuda
```

Static/CI success does not prove real CUDA latency/VRAM, linguistic quality, voice quality, microphone behavior, virtual-audio delivery or clean-machine installation.

## Model acquisition / packaging

`model_manifest.json` is the model inventory. `prepare_model_assets.py` validates full revisions, stages Hugging Face snapshots atomically and writes revision markers after successful replacement.

Release packaging explicitly includes the worker entrypoint/support modules; large Python/model/GPT-SoVITS/built-in reference assets remain controlled payload inputs outside Git.

## Runtime rules

- one persistent Python AI worker;
- one dependency project/lock;
- one Indonesian ↔ English translation model;
- one selected-Meeting-voice inference contract;
- no hidden CPU retry after unknown CUDA/model failure;
- model presence is not model correctness without exact revision evidence;
- output without verifiable completion is not successful output;
- repository/hosted checks and target-Windows checks remain separate.
