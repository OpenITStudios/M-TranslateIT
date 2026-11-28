#!/usr/bin/env python3
"""Deterministically remove release-only Python and VoiceLab baggage from a staged backend."""

from __future__ import annotations

import argparse
import csv
import shutil
from pathlib import Path

EXCLUDED_DISTRIBUTIONS = {
    "annotated-types",
    "budoux",
    "cn2an",
    "colorlog",
    "fast-langdetect",
    "fasttext-predict",
    "pandas",
    "peft",
    "proces",
    "pydantic",
    "pydantic-core",
    "pypinyin",
    "robust-downloader",
    "split-lang",
    "typing-inspection",
    "tzdata",
}

REQUIRED_DISTRIBUTIONS = {
    "accelerate",
    "ctranslate2",
    "faster-whisper",
    "g2p-en",
    "librosa",
    "matplotlib",
    "numpy",
    "onnxruntime",
    "pytorch-lightning",
    "scipy",
    "sentencepiece",
    "soundfile",
    "tensorboard",
    "torch",
    "torchaudio",
    "transformers",
}

EXPECTED_BASELINE_DISTRIBUTIONS = 116
# License-pinned distributions (validate_release_payload.mjs
# exceptionalPythonLicenseMaterials) must never be excluded, so the optimized
# closure keeps them: 116 - 16 exclusions = 100.
EXPECTED_OPTIMIZED_DISTRIBUTIONS = 100

ENGLISH_ONLY_MARKER = "TRANSLATEIT_ENGLISH_ONLY.txt"
ENGLISH_ONLY_MARKER_TEXT = (
    "TranslateIT release payload: English-only GPT-SoVITS text path.\n"
    "Chinese RoBERTa model bytes are intentionally excluded.\n"
    "voice_lab_upstream_stage.py supplies zero BERT features for approved English text.\n"
)


def normalize_name(value: str) -> str:
    return value.strip().lower().replace("_", "-")


def tree_bytes(root: Path) -> int:
    if not root.exists():
        return 0
    return sum(path.stat().st_size for path in root.rglob("*") if path.is_file())


def distribution_name(dist_info: Path) -> str:
    metadata = dist_info / "METADATA"
    if not metadata.is_file():
        raise RuntimeError(f"release_optimize:distribution_metadata_missing:{dist_info.name}")
    for raw in metadata.read_text(encoding="utf-8", errors="strict").splitlines():
        if raw.lower().startswith("name:"):
            return normalize_name(raw.split(":", 1)[1])
    raise RuntimeError(f"release_optimize:distribution_name_missing:{dist_info.name}")


def record_paths(python_root: Path, dist_info: Path) -> set[Path]:
    record = dist_info / "RECORD"
    if not record.is_file():
        raise RuntimeError(f"release_optimize:distribution_record_missing:{dist_info.name}")
    result: set[Path] = set()
    with record.open("r", encoding="utf-8", newline="") as handle:
        for row in csv.reader(handle):
            if not row:
                continue
            candidate = (python_root / row[0]).resolve()
            try:
                candidate.relative_to(python_root)
            except ValueError:
                continue
            result.add(candidate)
    return result


def collect_distributions(python_root: Path) -> dict[str, tuple[Path, set[Path]]]:
    result: dict[str, tuple[Path, set[Path]]] = {}
    for dist_info in sorted(python_root.glob("*.dist-info")):
        if not dist_info.is_dir():
            continue
        name = distribution_name(dist_info)
        if name in result:
            raise RuntimeError(f"release_optimize:duplicate_distribution:{name}")
        result[name] = (dist_info, record_paths(python_root, dist_info))
    return result


def remove_excluded_distributions(python_root: Path) -> tuple[int, list[str]]:
    python_root = python_root.resolve()
    before = tree_bytes(python_root)
    distributions = collect_distributions(python_root)
    if len(distributions) not in {EXPECTED_BASELINE_DISTRIBUTIONS, EXPECTED_OPTIMIZED_DISTRIBUTIONS}:
        raise RuntimeError(
            f"release_optimize:unexpected_distribution_count:{len(distributions)}"
        )
    missing_required = sorted(REQUIRED_DISTRIBUTIONS - distributions.keys())
    if missing_required:
        raise RuntimeError(
            "release_optimize:required_distributions_missing:" + ",".join(missing_required)
        )

    present_excluded = sorted(EXCLUDED_DISTRIBUTIONS & distributions.keys())
    if not present_excluded:
        if len(distributions) != EXPECTED_OPTIMIZED_DISTRIBUTIONS:
            raise RuntimeError(
                f"release_optimize:optimized_distribution_count_mismatch:{len(distributions)}"
            )
        return 0, []

    retained_owners: dict[Path, set[str]] = {}
    for name, (_dist_info, paths) in distributions.items():
        if name in EXCLUDED_DISTRIBUTIONS:
            continue
        for path in paths:
            retained_owners.setdefault(path, set()).add(name)

    removable: set[Path] = set()
    for name in present_excluded:
        _dist_info, paths = distributions[name]
        for path in paths:
            if path not in retained_owners:
                removable.add(path)

    for path in sorted(removable, key=lambda item: len(item.parts), reverse=True):
        if path.is_file() or path.is_symlink():
            path.unlink(missing_ok=True)

    for name in present_excluded:
        dist_info = distributions[name][0]
        if dist_info.exists():
            shutil.rmtree(dist_info)

    for cache in sorted(
        python_root.rglob("__pycache__"), key=lambda item: len(item.parts), reverse=True
    ):
        if cache.is_dir():
            shutil.rmtree(cache)
    for suffix in ("*.pyc", "*.pyo"):
        for path in python_root.rglob(suffix):
            if path.is_file():
                path.unlink()

    for directory in sorted(
        (path for path in python_root.rglob("*") if path.is_dir()),
        key=lambda item: len(item.parts),
        reverse=True,
    ):
        try:
            directory.rmdir()
        except OSError:
            pass

    remaining = collect_distributions(python_root)
    if len(remaining) != EXPECTED_OPTIMIZED_DISTRIBUTIONS:
        raise RuntimeError(
            f"release_optimize:optimized_distribution_count_mismatch:{len(remaining)}"
        )
    leaked = sorted(EXCLUDED_DISTRIBUTIONS & remaining.keys())
    if leaked:
        raise RuntimeError("release_optimize:excluded_distributions_remain:" + ",".join(leaked))
    missing_required = sorted(REQUIRED_DISTRIBUTIONS - remaining.keys())
    if missing_required:
        raise RuntimeError(
            "release_optimize:required_distributions_removed:" + ",".join(missing_required)
        )

    after = tree_bytes(python_root)
    return before - after, present_excluded


def remove_torch_build_artifacts(python_root: Path) -> int:
    """Remove PyTorch C/C++ build inputs while preserving every runtime DLL."""

    python_root = python_root.resolve()
    torch_root = python_root / "torch"
    if not torch_root.is_dir():
        raise RuntimeError("release_optimize:torch_package_missing")

    lib_root = torch_root / "lib"
    dll_inventory_before = {
        path.name: path.stat().st_size for path in sorted(lib_root.glob("*.dll"))
    }
    if not dll_inventory_before:
        raise RuntimeError("release_optimize:torch_runtime_dlls_missing")

    targets: list[Path] = []
    for directory in (torch_root / "include", torch_root / "share"):
        if directory.exists():
            targets.append(directory)
    for pattern in ("*.lib", "*.exp", "*.pdb"):
        targets.extend(sorted(lib_root.glob(pattern)))

    before = sum(tree_bytes(path) if path.is_dir() else path.stat().st_size for path in targets)
    for path in targets:
        if path.is_dir():
            shutil.rmtree(path)
        elif path.is_file() or path.is_symlink():
            path.unlink(missing_ok=True)

    dll_inventory_after = {
        path.name: path.stat().st_size for path in sorted(lib_root.glob("*.dll"))
    }
    if dll_inventory_after != dll_inventory_before:
        raise RuntimeError("release_optimize:torch_runtime_dll_inventory_changed")

    return before


def optimize_english_voice_asset(backend_root: Path) -> int:
    bert_root = (
        backend_root
        / "RuntimeAssets"
        / "Voice"
        / "GPTSoVITS"
        / "Source"
        / "GPT_SoVITS"
        / "pretrained_models"
        / "chinese-roberta-wwm-ext-large"
    )
    before = tree_bytes(bert_root)
    marker = bert_root / ENGLISH_ONLY_MARKER
    if marker.is_file():
        extra = [path for path in bert_root.rglob("*") if path.is_file() and path != marker]
        if extra:
            raise RuntimeError("release_optimize:english_only_bert_marker_has_extra_payload")
        return 0
    if not bert_root.is_dir() or before <= 0:
        raise RuntimeError("release_optimize:chinese_roberta_source_missing")
    shutil.rmtree(bert_root)
    bert_root.mkdir(parents=True, exist_ok=True)
    marker.write_text(ENGLISH_ONLY_MARKER_TEXT, encoding="utf-8", newline="\n")
    after = tree_bytes(bert_root)
    return max(0, before - after)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--backend-root", required=True)
    args = parser.parse_args()
    raise SystemExit("Native packaging is held: legacy Python payload optimizer is disabled.")

    backend_root = Path(args.backend_root).resolve()
    python_root = backend_root / "LocalWorker" / "PythonRuntime"
    if not (python_root / "python.exe").is_file():
        raise SystemExit("release_optimize:python_runtime_missing")

    distribution_saving, removed = remove_excluded_distributions(python_root)
    torch_build_saving = remove_torch_build_artifacts(python_root)
    bert_saving = optimize_english_voice_asset(backend_root)
    python_saving = distribution_saving + torch_build_saving
    print(
        "[release-optimize] "
        f"python_saving_bytes={python_saving} "
        f"excluded_distribution_saving_bytes={distribution_saving} "
        f"torch_build_saving_bytes={torch_build_saving} "
        f"excluded_distributions={len(removed)} "
        f"voice_bert_saving_bytes={bert_saving}"
    )
    if removed:
        print("[release-optimize] removed=" + ",".join(removed))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
