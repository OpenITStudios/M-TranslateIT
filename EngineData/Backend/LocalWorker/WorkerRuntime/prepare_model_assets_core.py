from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import stat
import uuid
from pathlib import Path
from typing import Any

from huggingface_hub import snapshot_download

MANIFEST_PATH = Path(__file__).with_name("model_manifest.json")
PROJECT_ROOT = Path(__file__).resolve().parents[4]
RUNTIME_ASSETS_ROOT = PROJECT_ROOT / "EngineData/Backend/RuntimeAssets"
FULL_REVISION = re.compile(r"^[0-9a-f]{40}$")
REVISION_MARKER = ".translateit_model_revision"
FULL_SHA256 = re.compile(r"^[0-9a-f]{64}$")


def load_manifest() -> dict[str, Any]:
    data = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    if data.get("schema") != "translateit.local_model_inventory.v2":
        raise RuntimeError("model_manifest.json has an unsupported schema")
    if data.get("inventory_scope") != "full_product_release_assets":
        raise RuntimeError("model_manifest.json has an unexpected inventory scope")
    models = data.get("models")
    if not isinstance(models, list) or not models:
        raise RuntimeError("model_manifest.json contains no model entries")
    return data


def require_plain_directory_path(root: Path, target: Path) -> None:
    # Check the lexical path BEFORE resolving it. resolve() would hide a
    # junction or symlink pointing to a different model under the same root.
    try:
        relative = target.relative_to(root)
    except ValueError as exc:
        raise RuntimeError("Model target escapes its authorized root") from exc
    if ".." in relative.parts:
        raise RuntimeError("Model target contains parent traversal")
    current = root
    for part in ("", *relative.parts):
        current = current / part
        try:
            meta = current.lstat()
        except FileNotFoundError:
            # No existing descendants can redirect a missing directory.
            return
        if is_redirected(meta) or not stat.S_ISDIR(meta.st_mode):
            raise RuntimeError(f"Model target cannot be a filesystem indirection or non-directory: {current}")


def resolve_target(expected_path: str) -> Path:
    raw = Path(expected_path)
    if raw.is_absolute() or raw.drive or ".." in raw.parts:
        raise RuntimeError(f"Model expected_path must be repository-relative without traversal: {expected_path}")
    target = PROJECT_ROOT / raw
    if target == RUNTIME_ASSETS_ROOT or not target.is_relative_to(RUNTIME_ASSETS_ROOT):
        raise RuntimeError(f"Model expected_path escapes the canonical RuntimeAssets root: {expected_path}")
    require_plain_directory_path(PROJECT_ROOT, target)
    return target


def validate_download_allow_patterns(model_id: str, value: Any) -> list[str] | None:
    if value in (None, []):
        return None
    if not isinstance(value, list) or not value:
        raise RuntimeError(f"{model_id} download_allow_patterns must be a non-empty list")

    patterns: list[str] = []
    for raw in value:
        pattern = str(raw or "").strip().replace("\\", "/")
        if not pattern or pattern.startswith("/"):
            raise RuntimeError(f"{model_id} has invalid download allow pattern: {raw!r}")
        parts = [part for part in pattern.split("/") if part]
        if any(part == ".." for part in parts):
            raise RuntimeError(f"{model_id} download allow pattern may not escape its snapshot")
        patterns.append(pattern)
    return list(dict.fromkeys(patterns))


def validate_huggingface_model(model: dict[str, Any]) -> dict[str, Any]:
    model_id = str(model.get("model_id", "")).strip()
    repo_id = str(model.get("repo_id", "")).strip()
    revision = str(model.get("revision", "")).strip()
    expected_path = str(model.get("expected_path", "")).strip()
    if not model_id or not repo_id or not expected_path:
        raise RuntimeError(f"Incomplete Hugging Face model entry: {model_id or '<missing-id>'}")
    if model.get("install_method") != "huggingface_snapshot_download":
        raise RuntimeError(f"Unexpected install method for {model_id}")
    if not FULL_REVISION.fullmatch(revision):
        raise RuntimeError(f"{model_id} must pin a full Hugging Face commit revision")
    target = resolve_target(expected_path)
    patterns = validate_download_allow_patterns(model_id, model.get("download_allow_patterns"))
    hashes, sizes = artifact_pins(model, patterns)
    return {
        "model_id": model_id,
        "repo_id": repo_id,
        "revision": revision,
        "required": bool(model.get("required")),
        "stage": str(model.get("stage", "")),
        "target": target,
        "download_allow_patterns": patterns,
        "artifact_hashes": hashes,
        "artifact_bytes": sizes,
    }


def artifact_pins(item: dict[str, Any], patterns: list[str] | None) -> tuple[dict[str, str], dict[str, int]]:
    hashes = item.get("artifact_hashes")
    sizes = item.get("artifact_bytes", {})
    if not isinstance(hashes, dict) or not hashes or not isinstance(sizes, dict):
        raise RuntimeError("Exact native artifact hashes are required")
    if patterns is not None and set(patterns) != set(hashes):
        raise RuntimeError("Native artifact pins must match exact download allow patterns")
    for name, digest in hashes.items():
        if (not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9_.-]+", name)
                or name in (".", "..", REVISION_MARKER) or ".." in name
                or not isinstance(digest, str) or not FULL_SHA256.fullmatch(digest)):
            raise RuntimeError("Invalid native artifact pin")
    if any(name not in hashes or type(size) is not int or size <= 0 for name, size in sizes.items()):
        raise RuntimeError("Invalid native artifact size pin")
    return hashes, sizes


def is_redirected(meta: Any) -> bool:
    return stat.S_ISLNK(meta.st_mode) or bool(
        getattr(meta, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT
    )


def exact_snapshot_matches(target: Path, item: dict[str, Any], *, marked: bool) -> bool:
    hashes, sizes = artifact_pins(item, item.get("download_allow_patterns"))
    try:
        directory = target.lstat()
        if not stat.S_ISDIR(directory.st_mode) or is_redirected(directory):
            return False
        expected = set(hashes) | ({REVISION_MARKER} if marked else set())
        if {path.name for path in target.iterdir()} != expected:
            return False
        if marked:
            marker = target / REVISION_MARKER
            meta = marker.lstat()
            if is_redirected(meta) or not stat.S_ISREG(meta.st_mode) or meta.st_size > 128:
                return False
            with marker.open("rb") as stream:
                if stream.read(129).strip() != item["revision"].encode("ascii"):
                    return False
        for name, digest in hashes.items():
            path = target / name
            meta = path.lstat()
            if (is_redirected(meta) or not stat.S_ISREG(meta.st_mode) or meta.st_size <= 0
                    or (name in sizes and meta.st_size != sizes[name])):
                return False
            with path.open("rb") as stream:
                if hashlib.file_digest(stream, "sha256").hexdigest() != digest:
                    return False
        return True
    except OSError:
        return False


def build_plan(
    manifest: dict[str, Any],
    include_optional: bool = False,
    requested_ids: set[str] | None = None,
) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    selected: list[dict[str, Any]] = []
    manual_required: list[dict[str, str]] = []
    seen_ids: set[str] = set()
    seen_targets: set[Path] = set()

    for raw in manifest["models"]:
        model_id = str(raw.get("model_id", "")).strip()
        if not model_id:
            raise RuntimeError("Model manifest contains an entry without model_id")
        if model_id in seen_ids:
            raise RuntimeError(f"Duplicate model_id in manifest: {model_id}")
        seen_ids.add(model_id)

        source_type = str(raw.get("source_type", "")).strip()
        if source_type == "huggingface":
            item = validate_huggingface_model(raw)
            target = item["target"]
            if target in seen_targets:
                raise RuntimeError(f"Duplicate model target in manifest: {target}")
            seen_targets.add(target)
            if requested_ids is not None:
                if model_id in requested_ids:
                    selected.append(item)
            elif item["required"] or include_optional:
                selected.append(item)
        elif bool(raw.get("required")):
            manual_required.append(
                {
                    "model_id": model_id,
                    "stage": str(raw.get("stage", "")),
                    "install_method": str(raw.get("install_method", "manual")),
                }
            )

    if requested_ids is not None:
        missing = requested_ids - seen_ids
        if missing:
            raise RuntimeError(f"Unknown model_id requested: {', '.join(sorted(missing))}")
        unsupported = requested_ids - {item["model_id"] for item in selected}
        if unsupported:
            raise RuntimeError(
                "Requested model is not Hugging Face-acquirable: " + ", ".join(sorted(unsupported))
            )

    if not selected:
        raise RuntimeError("No Hugging Face model assets selected")
    return selected, manual_required


def clean_huggingface_local_cache(directory: Path) -> None:
    cache = directory / ".cache" / "huggingface"
    require_plain_directory_path(directory, cache)
    if cache.exists():
        shutil.rmtree(cache)
    cache_parent = directory / ".cache"
    if cache_parent.exists() and not any(cache_parent.iterdir()):
        cache_parent.rmdir()


def replace_target_from_snapshot(item: dict[str, Any]) -> dict[str, Any]:
    """Admit a checked snapshot without destroying an interrupted rollback.

    The final marker is part of the same staged directory as the model bytes.
    Never reuse a PID-derived path: interrupted backups must not be deleted
    merely because a new Python process happens to get the same PID.
    """
    target: Path = item["target"]
    # CLI plans are rooted at PROJECT_ROOT. Direct test fixtures may own a
    # disposable root; still reject an indirect target or immediate parent.
    boundary = PROJECT_ROOT if target.is_relative_to(PROJECT_ROOT) else target.parent
    require_plain_directory_path(boundary, target)
    hashes, _ = artifact_pins(item, item.get("download_allow_patterns"))
    if target.exists() or target.is_symlink():
        if is_redirected(target.lstat()) or not target.is_dir():
            raise RuntimeError("Model target cannot be a filesystem indirection or non-directory")
        if exact_snapshot_matches(target, item, marked=True):
            return {
                "model_id": item["model_id"], "repo_id": item["repo_id"],
                "revision": item["revision"], "file_count": len(hashes),
                "replaced_existing": False, "reused_verified": True,
                "revision_marker": REVISION_MARKER,
            }
    target.parent.mkdir(parents=True, exist_ok=True)
    transaction = uuid.uuid4().hex
    staging = target.parent / f".{target.name}.download-{transaction}"
    backup = target.parent / f".{target.name}.previous-{transaction}"

    had_previous = target.exists()
    moved_previous = False
    promoted = False
    try:
        download_args: dict[str, Any] = {
            "repo_id": item["repo_id"],
            "revision": item["revision"],
            "local_dir": staging,
        }
        if item.get("download_allow_patterns"):
            download_args["allow_patterns"] = item["download_allow_patterns"]
        snapshot_download(**download_args)
        require_plain_directory_path(boundary, staging)
        clean_huggingface_local_cache(staging)
        if not exact_snapshot_matches(staging, item, marked=False):
            raise RuntimeError(f"Downloaded snapshot for {item['model_id']} failed exact artifact verification")
        # The marker must be committed along with the snapshot, not written
        # after replacing the live model. A crash cannot publish an unmarked
        # model while claiming that the previous version was verified.
        (staging / REVISION_MARKER).write_text(
            str(item["revision"]).strip() + "\n", encoding="utf-8"
        )
        require_plain_directory_path(boundary, target)
        if had_previous:
            target.replace(backup)
            moved_previous = True
        try:
            staging.replace(target)
            promoted = True
        except Exception:
            if moved_previous and backup.exists() and not target.exists():
                backup.replace(target)
                moved_previous = False
            raise
        # Only retire the old version after the new directory and revision
        # marker have both been moved into the final canonical location.
        if moved_previous:
            shutil.rmtree(backup)
        return {
            "model_id": item["model_id"],
            "repo_id": item["repo_id"],
            "revision": item["revision"],
            "file_count": len(hashes),
            "replaced_existing": had_previous,
            "reused_verified": False,
            "revision_marker": REVISION_MARKER,
        }
    finally:
        if promoted:
            # Cleanup only this successfully committed transaction.
            if staging.exists():
                shutil.rmtree(staging)
        # On failed download/promotion, preserve unverified staged files and
        # especially any backup that could be required for manual recovery.
        # The next transaction obtains a fresh UUID and cannot overwrite them.


def stage_download_plan(
    items: list[dict[str, Any]], destination_root: Path
) -> list[dict[str, Any]]:
    """Redirect only vetted manifest targets into a private release staging tree."""
    root = destination_root.absolute()
    safe_parent = PROJECT_ROOT / ".workspace" / "staging"
    if root == safe_parent or not root.is_relative_to(safe_parent):
        raise RuntimeError("Release model staging must stay below .workspace/staging")
    relative_stage = root.relative_to(safe_parent)
    if relative_stage.parts[0] in {"release-active", "release-rollback"} or ".." in relative_stage.parts:
        raise RuntimeError("Release model staging cannot use reserved or redirected workspace paths")
    require_plain_directory_path(PROJECT_ROOT, root / "EngineData" / "Backend")
    if not (root / "EngineData" / "Backend").is_dir():
        raise RuntimeError("Release staging Backend root must already exist")
    redirected: list[dict[str, Any]] = []
    for item in items:
        relative = item["target"].relative_to(RUNTIME_ASSETS_ROOT)
        target = root / "EngineData" / "Backend" / "RuntimeAssets" / relative
        require_plain_directory_path(PROJECT_ROOT, target)
        redirected.append({**item, "target": target})
    return redirected


def serializable_plan(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    output: list[dict[str, Any]] = []
    for item in items:
        entry = {
            "model_id": item["model_id"],
            "repo_id": item["repo_id"],
            "revision": item["revision"],
            "required": item["required"],
            "stage": item["stage"],
            "expected_path": str(item["target"].relative_to(PROJECT_ROOT)).replace("\\", "/"),
        }
        if item.get("download_allow_patterns"):
            entry["download_allow_patterns"] = item["download_allow_patterns"]
        output.append(entry)
    return output


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Acquire TranslateIT developer Hugging Face model assets from pinned manifest revisions."
    )
    parser.add_argument(
        "--include-optional",
        action="store_true",
        help="Also acquire optional Hugging Face entries if present in the manifest.",
    )
    parser.add_argument(
        "--model-id",
        action="append",
        default=[],
        help="Acquire only a specific manifest model_id. May be repeated.",
    )
    parser.add_argument(
        "--destination-root",
        type=Path,
        help="Release-only isolated model target beneath .workspace/staging.",
    )
    parser.add_argument(
        "--plan",
        action="store_true",
        help="Validate and print the pinned acquisition plan without downloading files.",
    )
    args = parser.parse_args()

    requested = set(args.model_id) if args.model_id else None
    manifest = load_manifest()
    selected, manual_required = build_plan(manifest, args.include_optional, requested)
    if args.destination_root is not None:
        selected = stage_download_plan(selected, args.destination_root)
    output: dict[str, Any] = {
        "schema": "translateit.developer_model_acquisition.v1",
        "manifest_schema": manifest["schema"],
        "plan_only": bool(args.plan),
        "selected": serializable_plan(selected),
        "manual_required_assets": manual_required,
        "downloads": [],
        "note": (
            "Pinned Hugging Face assets are acquired from model_manifest.json. Required release assets "
            "such as the pending independent speaker-verifier export remain separate and are not fabricated by this developer downloader."
        ),
    }

    if not args.plan:
        output["downloads"] = [replace_target_from_snapshot(item) for item in selected]

    print(json.dumps(output, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
