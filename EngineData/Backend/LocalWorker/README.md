# Backend LocalWorker

`LocalWorker` owns TranslateIT's remaining local Python boundary for selected Meeting-voice inference and the bounded My Voice build child. ASR and Indonesian ↔ English translation are Rust-owned native CUDA operations.

## Layout

```text
LocalWorker/
├─ WorkerRuntime/                    # Git-tracked runtime source
│  ├─ realtime_local_worker.py       # one application-facing worker entrypoint
│  ├─ realtime_local_worker_base.py
│  ├─ worker_runtime_common.py
│  ├─ worker_io_runtime.py
│  ├─ voice_lab_*.py                 # retained compatibility filenames
│  ├─ model_manifest.json
│  ├─ pyproject.toml
│  └─ uv.lock
└─ PythonRuntime/                    # staged R3 payload; ignored by Git
   └─ python.exe                     # installed-product interpreter
```

Built-in Male/Female selection is prepared by the Rust/Tauri voice owner from pinned packaged reference assets and the shared GPT-SoVITS pretrained weights, then consumed through the same worker `voice_actor_*` inference contract. My Voice uses the same selected-Meeting-voice contract after its trained actor is explicitly approved. There is no second daily Python worker or fallback TTS service.

Existing `voice_lab_*` filenames/state/error namespaces remain compatibility identifiers where current protocol/storage/package contracts require them; product terminology is **My Voice**.

Development may use `WorkerRuntime/.venv`, `TRANSLATEIT_WORKER_PYTHON`, or system Python only inside repository-development mode. Packaged mode resolves the private `PythonRuntime/python.exe` and fails closed if it is missing. Normal users do not install Python, pip, uv, or WorkerRuntime dependencies manually.

## Dependency versions

`WorkerRuntime/pyproject.toml` + `uv.lock` is the dependency authority. Current required versions include:

```text
Python       3.12.x
Torch        2.11.0 / cu126 release path
Transformers 4.57.6
Tokenizers   0.22.2
Accelerate   1.14.0
```

The R3 staged private runtime starts from CPython 3.12.10 Windows embeddable package:

```text
archive SHA-256 4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3
```

`stage_release_inputs.ps1` owns deterministic staging. `stage_release_license_material.py`, `validate_release_payload.mjs`, and the third-party notice generator own release provenance/material checks. `optimize_release_payload.py` owns the profiled release-only **116 → 100** distribution closure. The validator derives this relationship from the optimizer's actual excluded set instead of duplicating a separate magic count.

## R3 release boundary

Private Python is part of `TranslateIT-Payload.7z`, not a large Tauri resource. The small Tauri Setup closure contains WorkerRuntime source required to launch the packaged worker; the external payload installs `PythonRuntime` and large assets under the application resource root.

The payload contract binds app/schema identity, dependency/model pins and SHA-256. Reinstall replaces external application runtime transactionally. Uninstall removes TranslateIT-owned runtime while preserving app-local user data and the system VB-CABLE driver.

## Rules

- Keep one Python runtime and one daily worker process.
- Keep the native ASR/MT owner as the single production speech recognition and translation route.
- Keep one selected-Meeting-voice inference contract for built-in voices and approved My Voice.
- Do not create a second GPT-SoVITS environment/server/provider registry.
- Do not make system Python, pip, uv, PATH hacks or repository checkout installed-product dependencies.
- Do not commit private Python/model/GPT-SoVITS runtime bytes, `.venv`, caches, personal voice data or user data.
- Do not turn source/hosted proof into installed-runtime, CUDA, audio, speaker-quality or clean-machine proof.
