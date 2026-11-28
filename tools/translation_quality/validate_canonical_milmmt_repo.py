# Historical command filename retained for existing held source-validation entrypoints.
# This validates native-only ASR/MT, not the retired MiLMMT implementation.
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

MODEL_ID = "hy-mt2-1.8b-q6_k"
HF_MODEL_ID = "tencent/Hy-MT2-1.8B-GGUF"
REVISION = "a0c709d9fac510f2c807aa3af52872340dc37a4a"
EXPECTED_MODEL_PATH = (
    "EngineData/Backend/RuntimeAssets/Translation/ModelData/"
    "hy-mt2-1.8b"
)
ACCELERATE_VERSION = "1.15.0"
TRANSFORMERS_VERSION = "4.57.6"
TOKENIZERS_VERSION = "0.22.2"
TOKENIZERS_SDIST_SHA256 = "473b83b915e547aa366d1eee11806deaf419e17be16310ac0a14077f1e28f917"
LEGACY_ACTIVE_MARKERS = (
    "facebook/m2m100_418M",
    "Helsinki-NLP/opus-mt-id-en",
    "Helsinki-NLP/opus-mt-en-id",
    "AutoModelForSeq2SeqLM",
    "forced_bos_token_id",
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def read(root: Path, relative: str) -> str:
    path = root / relative
    require(path.is_file(), f"missing:{relative}")
    return path.read_text(encoding="utf-8")


def assignment_int(source: str, name: str) -> int:
    match = re.search(rf"(?m)^{re.escape(name)}\s*=\s*(\d+)\s*$", source)
    require(match is not None, f"release:missing_assignment:{name}")
    return int(match.group(1))


def validate_manifest(root: Path) -> None:
    data = json.loads(read(root, "EngineData/Backend/LocalWorker/WorkerRuntime/model_manifest.json"))
    entries = [item for item in data.get("models", []) if item.get("stage") == "translation_bidirectional_id_en"]
    require(len(entries) == 1, "manifest:translation_entry_count")
    item = entries[0]
    require(item.get("model_id") == MODEL_ID, "manifest:model_id")
    require(item.get("repo_id") == HF_MODEL_ID, "manifest:repo_id")
    require(item.get("revision") == REVISION, "manifest:revision")
    require(item.get("expected_path") == EXPECTED_MODEL_PATH, "manifest:expected_path")
    require(item.get("backend") == "llama.cpp" and item.get("cpu_fallback") is False, "manifest:native_backend")
    require(set(item.get("download_allow_patterns") or []) == {"Hy-MT2-1.8B-Q6_K.gguf"}, "manifest:exact_gguf")
    asr = [model for model in data["models"] if model.get("stage") == "asr_primary"]
    require(len(asr) == 1 and asr[0].get("model_id") == "qwen3-asr-1.7b-q8_0"
            and asr[0].get("revision") == "3b2c29585c2180452ccb14d3b6b58127f352400d"
            and asr[0].get("cpu_fallback") is False, "manifest:native_asr")
    require(data.get("meeting_readiness_owner") == "rust.helper_bridge", "manifest:readiness_owner")
    require(not any(model["model_id"] in {"milmmt-46-1b-v1.0", "faster-whisper-medium"} for model in data["models"]), "manifest:retired_models")


def validate_worker(root: Path) -> None:
    prefix = "EngineData/Backend/LocalWorker/WorkerRuntime/"
    entrypoint = read(root, prefix + "realtime_local_worker.py")
    base = read(root, prefix + "realtime_local_worker_base.py")
    common = read(root, prefix + "worker_runtime_common.py")
    io_runtime = read(root, prefix + "worker_io_runtime.py")
    acquisition = read(root, prefix + "prepare_model_assets.py")
    require("import realtime_local_worker_base as runtime" in entrypoint, "worker:base_missing")
    require("exec(" not in entrypoint and "compile(" not in entrypoint, "worker:dynamic_exec_bootstrap")
    require("import prepare_model_assets_core as _core" in acquisition, "acquisition:core_import_missing")
    require("exec(" not in acquisition and "compile(" not in acquisition, "acquisition:dynamic_exec_bootstrap")
    require(".translateit_model_revision" in acquisition, "acquisition:revision_marker")
    for name in ("milmmt_translation_provider.py", "translation_envelope.py", "realtime_local_worker_core.py"):
        require(not (root / (prefix + name)).exists(), f"worker:legacy_source:{name}")
    commands = re.search(r'HANDLERS\s*=\s*\{([\s\S]*?)\n\}', base)
    require(commands is not None and set(re.findall(r'"([a-z_]+)"\s*:', commands[1])) ==
            {"ping", "status", "voice_actor_preflight", "voice_actor_synthesize"}, "worker:voice_only")
    for label, body in (("entrypoint", entrypoint), ("base", base), ("common", common), ("io", io_runtime)):
        for marker in (*LEGACY_ACTIVE_MARKERS, "milmmt_translation_provider", "handle_transcribe", "handle_translate", "from faster_whisper"):
            require(marker not in body, f"worker:{label}:legacy:{marker}")
    transport = read(root, "EngineData/Frontend/RustApp/src-tauri/src/commands/helper_bridge/transport.rs")
    native = read(root, "EngineData/Frontend/RustApp/src-tauri/src/commands/helper_bridge/transport/native.rs")
    implementation = read(root, "tools/translation_quality/native_cuda/inference.cpp")
    require("return native::dispatch(" in transport and "TemporaryAsrWav" not in transport, "native:sole_dispatch")
    require("condition_asr_pcm(&mut samples" in native and "owner.begin(input, deadline)" in native, "native:owned_dispatch")
    require("TI_INCOMPLETE" in implementation and "TI_OUTPUT_LIMIT" in implementation
            and "llama_memory_clear" in implementation, "native:bounded_complete_current_source")


def lock_package_version(lock: str, name: str) -> str:
    match = re.search(
        rf'\[\[package\]\]\s*name = "{re.escape(name)}"\s*version = "([^"]+)"',
        lock,
        flags=re.MULTILINE,
    )
    require(match is not None, f"dependency:lock_package_missing:{name}")
    return match.group(1)


def validate_dependencies(root: Path) -> None:
    prefix = "EngineData/Backend/LocalWorker/WorkerRuntime/"
    pyproject = read(root, prefix + "pyproject.toml")
    lock = read(root, prefix + "uv.lock")
    require('"transformers==4.57.6"' in pyproject, "dependency:transformers_project_pin")
    require(lock_package_version(lock, "accelerate") == ACCELERATE_VERSION, "dependency:accelerate_lock")
    require(lock_package_version(lock, "transformers") == TRANSFORMERS_VERSION, "dependency:transformers_lock")
    require(lock_package_version(lock, "tokenizers") == TOKENIZERS_VERSION, "dependency:tokenizers_lock")
    require('{ name = "transformers", specifier = "==4.57.6" }' in lock, "dependency:transformers_lock_spec")
    require(TOKENIZERS_SDIST_SHA256 in lock, "dependency:tokenizers_sdist_hash")


def require_manual_workflow(source: str, label: str) -> None:
    """Verify the sole authorized workflow trigger using the current YAML shape."""
    lines = source.splitlines()
    blocks = [index for index, line in enumerate(lines) if line.startswith("on:")]
    require(len(blocks) == 1 and lines[blocks[0]] == "on:", f"{label}:trigger_block")
    events: list[str] = []
    for line in lines[blocks[0] + 1 :]:
        if line.strip() and not line[0].isspace():
            break
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line.startswith("    "):
            continue
        match = re.fullmatch(r"  ([a-z_]+):(?:[ ]*#.*)?", line)
        require(match is not None, f"{label}:trigger_shape")
        events.append(match.group(1))
    require(events == ["workflow_dispatch"], f"{label}:manual_only")


def validate_release(root: Path) -> None:
    app = "EngineData/Frontend/RustApp/"
    stage = read(root, app + "scripts/stage_release_inputs_impl.ps1")
    optimizer = read(root, app + "scripts/optimize_release_payload.py")
    license_stager = read(root, app + "scripts/stage_release_license_material.py")
    payload_validator = read(root, app + "scripts/validate_release_payload.mjs")
    package_contract = read(root, app + "scripts/validate_release_package_contract.mjs")
    builder = read(root, app + "scripts/build_r3_external_payload.py")
    hook = read(root, app + "src-tauri/windows/r3_payload_hooks.template.nsh")
    installer = read(root, app + "src-tauri/windows/r3_payload_installer.ps1")
    config = read(root, app + "src-tauri/tauri.release.conf.json")
    release_workflow = read(root, ".github/workflows/release-payload-verify.yml")
    lock_workflow = read(root, ".github/workflows/workerruntime-lock.yml")

    require("prepare_model_assets.py" in stage and "--destination-root $PayloadRoot" in stage and "Assert-TextEquals" in stage, "release:model_acquisition")
    require('"installMode": "perMachine"' in config, "release:per_machine")
    require("translateit-r3-payload-hooks.generated.nsh" in config, "release:hook_config")
    for forbidden in (
        "../../../Backend/LocalWorker/PythonRuntime/",
        "../../../Backend/RuntimeAssets/ASR/ModelData/",
        "../../../Backend/RuntimeAssets/Translation/ModelData/",
        "../../../Backend/RuntimeAssets/NativeCuda/",
        "../../../Backend/RuntimeAssets/Voice/GPTSoVITS/",
        "../../../Backend/RuntimeAssets/AudioProvider/VBCABLE/Package/",
    ):
        require(forbidden not in config, f"release:large_payload_embedded:{forbidden}")
    for marker in (
        "translateit.r3.external_payload.v3",
        "translateit.installed_runtime.v3",
        "TranslateIT-Payload.7z",
        "-m0=LZMA2",
        "TRANSLATEIT_PAYLOAD_CONTRACT.json",
    ):
        require(marker in builder, f"release:builder_marker:{marker}")
    for marker in ("NSIS_HOOK_PREINSTALL", "NSIS_HOOK_POSTINSTALL", "NSIS_HOOK_PREUNINSTALL", "$0 == 3010"):
        require(marker in hook, f"release:hook_marker:{marker}")
    for marker in ("@('-i','-h')", "Rollback-Payload", "Assert-NoRetiredRuntime", "preserve_system_driver"):
        require(marker in installer, f"release:installer_marker:{marker}")
    require("Native R3 source contract checks only" in package_contract, "release:package_contract")

    excluded_block = optimizer.split("EXCLUDED_DISTRIBUTIONS = {", 1)[1].split("}", 1)[0]
    required_block = optimizer.split("REQUIRED_DISTRIBUTIONS = {", 1)[1].split("}", 1)[0]
    excluded_names = set(re.findall(r'"([^"]+)"', excluded_block))
    baseline_count = assignment_int(optimizer, "EXPECTED_BASELINE_DISTRIBUTIONS")
    optimized_count = assignment_int(optimizer, "EXPECTED_OPTIMIZED_DISTRIBUTIONS")
    require('"accelerate"' not in excluded_block, "release:accelerate_excluded")
    require('"accelerate"' in required_block, "release:accelerate_required")
    require(
        optimized_count == baseline_count - len(excluded_names),
        "release:optimized_distribution_count_inconsistent",
    )

    active_release = "\n".join((stage, optimizer, builder, installer, config))
    for marker in LEGACY_ACTIVE_MARKERS:
        require(marker not in active_release, f"release:legacy_active:{marker}")
    require("tokenizers-0.22.2.dist-info" in license_stager, "release:tokenizers_license_version")
    require(TOKENIZERS_SDIST_SHA256 in license_stager, "release:tokenizers_license_hash")
    require("tokenizers-0.22.2.dist-info" in payload_validator, "release:tokenizers_payload_version")

    require_manual_workflow(release_workflow, "workflow:release")
    require_manual_workflow(lock_workflow, "workflow:lock")
    require("R3 Release Contract" in release_workflow, "workflow:release_owner")
    require("source-contract" in release_workflow, "workflow:release_source_job")
    require("controlled-payload" in release_workflow, "workflow:release_controlled_payload_job")
    require("build_r3_external_payload.py" in release_workflow, "workflow:payload_builder")
    require(not (root / ".github/workflows/release-efficiency-profile.yml").exists(), "workflow:duplicate_efficiency_profile")
    require(not (root / ".github/workflows/release-python-profile.yml").exists(), "workflow:duplicate_python_profile")
    require("uv lock --check" in lock_workflow, "workflow:lock_check")
    require("--upgrade-package" not in lock_workflow, "workflow:lock_mutation")


def validate_quality_evaluation(root: Path) -> None:
    quality_root = root / "tools" / "translation_quality"
    evaluator = quality_root / "evaluate_translation_quality.py"
    corpus = quality_root / "corpus" / "translation_quality_v1.json"
    self_test = quality_root / "self_test.py"
    quality_readme = quality_root / "README.md"
    for path in (evaluator, corpus, self_test, quality_readme):
        require(path.is_file(), f"quality:missing:{path.relative_to(root)}")
    corpus_data = json.loads(corpus.read_text(encoding="utf-8"))
    require(corpus_data.get("schema") == "translateit.translation_quality.v1", "quality:corpus_schema")
    cases = corpus_data.get("cases") or []
    require(len(cases) >= 26, "quality:corpus_too_small")
    require({item.get("direction") for item in cases} == {"id-en", "en-id"}, "quality:directions")
    categories = {item.get("category") for item in cases}
    for required in ("negation", "numbers_dates_units", "technical", "entities_literals", "code_switching", "prompt_boundary", "unicode", "repetition", "input_normalization"):
        require(required in categories, f"quality:missing_category:{required}")

    workflow = read(root, ".github/workflows/milmmt-repo-contract.yml")
    require_manual_workflow(workflow, "quality:workflow")
    require(
        "python tools/translation_quality/validate_canonical_milmmt_repo.py" in workflow,
        "quality:workflow_validator_task",
    )
    require("Validate translation quality corpus" in workflow, "quality:workflow_corpus_gate")
    require("Self-test translation quality evaluator" in workflow, "quality:workflow_self_test")


def validate_tests_and_runtime_docs(root: Path) -> None:
    tests = "\n".join(
        read(root, f"EngineData/Backend/LocalWorker/WorkerRuntime/tests/{name}")
        for name in ("test_worker_contract.py", "test_prepare_model_assets.py")
    )
    require(MODEL_ID in tests and "test_python_worker_rejects_retired_inference_commands" in tests, "tests:native_identity_and_voice_only")
    require("from transformers import AutoModelForSeq2SeqLM" not in tests, "tests:legacy_seq2seq")
    runtime_readme = read(root, "EngineData/Backend/LocalWorker/WorkerRuntime/README.md")
    require(TRANSFORMERS_VERSION in runtime_readme, "docs:runtime_readme:transformers")


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    validate_manifest(root)
    validate_worker(root)
    validate_dependencies(root)
    validate_release(root)
    validate_quality_evaluation(root)
    validate_tests_and_runtime_docs(root)
    print(
        json.dumps(
            {
                "ok": True,
                "model_id": MODEL_ID,
                "revision": REVISION,
                "accelerate": ACCELERATE_VERSION,
                "transformers": TRANSFORMERS_VERSION,
                "tokenizers": TOKENIZERS_VERSION,
                "worker_architecture": "rust_native_asr_mt_plus_voice_only_worker",
                "release_packaging": "r3_external_offline_payload",
                "target_pc_validation": "deferred",
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"Native inference repository contract validation failed: {exc}", file=sys.stderr)
        raise
