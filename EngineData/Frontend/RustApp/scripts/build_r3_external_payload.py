#!/usr/bin/env python3
"""Build TranslateIT's version-bound R3 external offline payload.

Build-time only: no runtime download/bootstrap behavior. The script packages the
already staged native runtime into one 7z/LZMA2 archive, validates that Windows
bsdtar can read it, binds the payload to the verified runtime file identity,
and renders the trusted NSIS hook consumed by the Tauri build.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

PAYLOAD_SCHEMA = "translateit.r3.external_payload.v3"
INSTALLED_RUNTIME_SCHEMA = "translateit.installed_runtime.v3"
PAYLOAD_FILENAME = "TranslateIT-Payload.7z"
MODEL_MANIFEST = "EngineData/Backend/LocalWorker/WorkerRuntime/model_manifest.json"
ASSETS = "EngineData/Backend/RuntimeAssets/"
PAYLOAD_CONTRACT = ASSETS + "NativeOrt/TRANSLATEIT_PAYLOAD_CONTRACT.json"


def load_native_inventory(repo_root: Path) -> dict:
    manifest = json.loads((repo_root / MODEL_MANIFEST).read_text(encoding="utf-8"))
    package = manifest["native_payload"]
    if (manifest["schema"] != "translateit.local_model_inventory.v2"
            or package["schema"] != PAYLOAD_SCHEMA
            or package["installed_schema"] != INSTALLED_RUNTIME_SCHEMA
            or package["contract_path"] != PAYLOAD_CONTRACT):
        raise RuntimeError("native payload source identity mismatch")
    return manifest


def require_file(path: Path, label: str) -> None:
    if not path.is_file() or path.stat().st_size <= 0:
        raise RuntimeError(f"missing required file: {label}: {path}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_bytes(root: Path) -> int:
    return sum(path.stat().st_size for path in root.rglob("*") if path.is_file())


def app_version(repo_root: Path) -> str:
    app_root = repo_root / "EngineData/Frontend/RustApp"
    package = json.loads((app_root / "package.json").read_text(encoding="utf-8"))
    tauri = json.loads((app_root / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
    package_version = str(package.get("version") or "").strip()
    tauri_version = str(tauri.get("version") or "").strip()
    if not package_version or package_version != tauri_version:
        raise RuntimeError(
            f"application version drift: package.json={package_version!r}, tauri.conf.json={tauri_version!r}"
        )
    return package_version


def is_filesystem_indirection(path: Path) -> bool:
    if path.is_symlink():
        return True
    is_junction = getattr(path, "is_junction", None)
    return bool(is_junction and is_junction())


def safe_relative_path(relative: str) -> bool:
    return bool(relative) and all(
        re.fullmatch(r"[A-Za-z0-9_.-]+", part) and part not in (".", "..")
        and not part.endswith(".")
        for part in relative.split("/")
    )


def validate_no_filesystem_indirection(repo_root: Path, roots: list[str]) -> None:
    if len(roots) != len(set(roots)):
        raise RuntimeError("duplicate native payload root")
    for relative in roots:
        if not safe_relative_path(relative) or not relative.startswith(ASSETS):
            raise RuntimeError("invalid native payload root")
        root = repo_root / relative
        if not root.is_dir():
            raise RuntimeError(f"missing payload root: {relative}")
        for parent in (root, *root.parents):
            if parent == repo_root:
                break
            if is_filesystem_indirection(parent):
                raise RuntimeError(f"payload filesystem indirection is forbidden: {parent}")
        if is_filesystem_indirection(root):
            raise RuntimeError(f"payload filesystem indirection is forbidden: {relative}")
        for path in root.rglob("*"):
            if is_filesystem_indirection(path):
                display = path.relative_to(repo_root).as_posix()
                raise RuntimeError(f"payload filesystem indirection is forbidden: {display}")


def validate_native_inputs(repo_root: Path, pins: str) -> dict[str, str]:
    # Trusted pins are supplied by the reviewed build, not derived from staged files.
    if not pins or len(pins) > 8192:
        raise RuntimeError("reviewed native DLL pins are required")
    expected: dict[str, str] = {}
    for entry in pins.split(";"):
        match = re.fullmatch(r"([a-z0-9][a-z0-9_.-]{0,95}\.dll)=([a-f0-9]{64})", entry)
        if not match or ".." in match[1] or match[1] in expected:
            raise RuntimeError("invalid or duplicate native DLL pin")
        expected[match[1]] = match[2]
    if "translateit_native_cuda.dll" not in expected or len(expected) > 32:
        raise RuntimeError("native DLL closure is incomplete")
    root = repo_root / "EngineData/Backend/RuntimeAssets/NativeCuda"
    if {p.name for p in root.iterdir()} != {"bin", "THIRD_PARTY_NOTICES.txt"}:
        raise RuntimeError("unexpected NativeCuda package entries")
    directory = root / "bin"
    actual = {p.name: p for p in directory.iterdir()}
    if set(actual) != set(expected):
        raise RuntimeError("native DLL set differs from reviewed pins")
    for name in expected:
        path = actual[name]
        require_file(path, name)
        if path.stat().st_size > 1024 * 1024 * 1024:
            raise RuntimeError(f"native DLL exceeds reviewed bound: {name}")
    # The file-contract recorder performs the hash check once below.
    return expected


def native_runtime_contract(repo_root: Path, manifest: dict, native_pins: str) -> dict:
    # Read-only verification; nothing becomes a reviewed pin merely by being staged.
    package = manifest["native_payload"]
    roots = package["roots"]
    verifier = next((model for model in manifest["models"]
                     if model["model_id"] == "eres2netv2-forward3-f32"), {})
    digest = verifier.get("artifact_sha256")
    size = verifier.get("artifact_size_bytes")
    if (verifier.get("pin_status") != "reviewed_export" or not isinstance(digest, str)
            or not re.fullmatch(r"[a-f0-9]{64}", digest) or type(size) is not int
            or not 0 < size <= 512 * 1024 * 1024
            or digest != os.environ.get("TRANSLATEIT_SPEAKER_ONNX_SHA256")
            or str(size) != os.environ.get("TRANSLATEIT_SPEAKER_ONNX_BYTES")):
        raise RuntimeError("reviewed speaker verifier export and matching build pins required")
    validate_no_filesystem_indirection(repo_root, roots)
    files: dict[str, dict] = {}

    def record(relative: str, expected: str | None = None, size: int | None = None) -> None:
        if (not safe_relative_path(relative) or relative in files
                or not any(relative.startswith(root + "/") for root in roots)):
            raise RuntimeError(f"invalid native payload file: {relative}")
        path = repo_root / relative
        require_file(path, relative)
        actual_size = path.stat().st_size
        actual = sha256_file(path)
        if (expected is not None and actual != expected) or (size is not None and actual_size != size):
            raise RuntimeError(f"native artifact identity mismatch: {relative}")
        files[relative] = {"sha256": actual, "bytes": actual_size}

    ids = {"qwen3-asr-1.7b-q8_0", "hy-mt2-1.8b-q6_k",
           "zonos3-mini-q8_0", "eres2netv2-forward3-f32"}
    if len(manifest["models"]) != len(ids) or {m["model_id"] for m in manifest["models"] if m["required"]} != ids:
        raise RuntimeError("native model inventory mismatch")
    for model in manifest["models"]:
        target = model["expected_path"]
        if model["source_type"] == "huggingface":
            if not safe_relative_path(target) or not any(target.startswith(root + "/") for root in roots):
                raise RuntimeError("invalid native model path")
            hashes = model["artifact_hashes"]
            if set(hashes) != set(model["download_allow_patterns"]):
                raise RuntimeError("native model file closure mismatch")
            for name, digest in hashes.items():
                if not re.fullmatch(r"[A-Za-z0-9_.-]+", name) or ".." in name or not re.fullmatch(r"[a-f0-9]{64}", digest):
                    raise RuntimeError("invalid model artifact pin")
                record(f"{target}/{name}", digest, model.get("artifact_bytes", {}).get(name))
            marker = target + "/.translateit_model_revision"
            if (repo_root / marker).read_text(encoding="utf-8").strip() != model["revision"]:
                raise RuntimeError("native model revision mismatch")
            record(marker)
        elif model is verifier and model["source_type"] == "offline_export":
            record(target, verifier["artifact_sha256"], verifier["artifact_size_bytes"])
        else:
            raise RuntimeError("unexpected native model acquisition type")
    for name, digest in validate_native_inputs(repo_root, native_pins).items():
        record(ASSETS + "NativeCuda/bin/" + name, digest)
    for name, pin in package["ort_files"].items():
        record(ASSETS + "NativeOrt/" + name, pin["sha256"], pin["bytes"])
    for name, digest in package["provider_files"].items():
        record(ASSETS + "AudioProvider/VBCABLE/Package/" + name, digest)
    for directory in ("NativeCuda", "NativeOrt"):
        record(ASSETS + directory + "/THIRD_PARTY_NOTICES.txt")
    builtins = ASSETS + "Voice/BuiltInVoices/"
    sources = json.loads((repo_root / (builtins + "SOURCES.json")).read_text(encoding="utf-8-sig"))
    voices = sources["voices"]
    if (sources["schema"] != "translateit.builtin_voice_sources.v1" or len(voices) != 2
            or {voice["voice_id"] for voice in voices} != {"MaleVoice", "FemaleVoice"}):
        raise RuntimeError("built-in voice source inventory mismatch")
    record(builtins + "SOURCES.json")
    for voice in voices:
        if voice["license"] != "CC-BY-4.0":
            raise RuntimeError("built-in voice license mismatch")
        record(builtins + voice["voice_id"] + "/reference.wav", voice["wav_sha256"])
        record(builtins + voice["voice_id"] + "/REFERENCE_SOURCE.txt")
    actual = {path.relative_to(repo_root).as_posix()
              for root in roots for path in (repo_root / root).rglob("*") if path.is_file()}
    actual.discard(PAYLOAD_CONTRACT)
    if actual != set(files):
        raise RuntimeError("unexpected or missing native payload files")
    return {"schema": PAYLOAD_SCHEMA, "installed_schema": INSTALLED_RUNTIME_SCHEMA,
            "roots": roots, "files": files,
            "user_data_policy": "preserve_app_local_user_data",
            "driver_policy": "preserve_system_driver"}


def write_payload_contract(repo_root: Path, body: dict) -> Path:
    # Exclude app version and timestamps: identical runtime bytes remain
    # compatible with a later app-only update. Setup signs this exact digest.
    path = repo_root / PAYLOAD_CONTRACT
    path.write_text(json.dumps(body, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    return path


def find_7zip() -> str:
    candidates: list[str] = []
    configured = os.environ.get("TRANSLATEIT_7ZIP", "").strip()
    if configured:
        candidates.append(configured)
    for command in ("7z.exe", "7zz.exe", "7z", "7zz"):
        resolved = shutil.which(command)
        if resolved:
            candidates.append(resolved)
    if os.name == "nt":
        candidates.extend(
            [r"C:\Program Files\7-Zip\7z.exe", r"C:\Program Files (x86)\7-Zip\7z.exe"]
        )
    for candidate in candidates:
        path = Path(candidate)
        if path.is_file():
            return str(path)
        resolved = shutil.which(candidate)
        if resolved:
            return resolved
    raise RuntimeError(
        "7-Zip CLI is required on the release build machine only. Install 7-Zip or set "
        "TRANSLATEIT_7ZIP; normal users do not need 7-Zip."
    )


def find_tar() -> str:
    for candidate in ("tar.exe", "tar"):
        resolved = shutil.which(candidate)
        if resolved:
            return resolved
    raise RuntimeError("Windows tar/bsdtar is required to validate the generated R3 payload.")


def create_payload(repo_root: Path, output: Path, seven_zip: str, tar_program: str,
                   roots: list[str], files: dict) -> list[str]:
    output.parent.mkdir(parents=True, exist_ok=True)
    output.unlink(missing_ok=True)
    subprocess.run(
        [
            seven_zip,
            "a",
            "-t7z",
            "-m0=LZMA2",
            "-mx=9",
            "-ms=on",
            "-mmt=on",
            "-y",
            str(output),
            *roots,
        ],
        cwd=repo_root,
        check=True,
    )
    require_file(output, PAYLOAD_FILENAME)

    listing = subprocess.run(
        [tar_program, "-tf", str(output)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    ).stdout.splitlines()
    entries = [line.strip().replace("\\", "/") for line in listing if line.strip()]
    if not entries:
        raise RuntimeError("payload archive listing is empty")

    exact_roots = set(roots)
    prefixes = tuple(root.rstrip("/") + "/" for root in roots)
    normalized_entries = {entry.rstrip("/") for entry in entries}
    if len(normalized_entries) != len(entries) or len({entry.lower() for entry in normalized_entries}) != len(entries):
        raise RuntimeError("duplicate payload archive entries")
    for entry in entries:
        normalized = entry.rstrip("/")
        if not safe_relative_path(normalized):
            raise RuntimeError(f"unsafe payload archive entry: {entry}")
        if normalized not in exact_roots and not normalized.startswith(prefixes):
            raise RuntimeError(f"payload entry escapes approved roots: {entry}")
    for required in (*files, PAYLOAD_CONTRACT):
        if required.replace("\\", "/") not in normalized_entries:
            raise RuntimeError(f"required payload entry absent from archive: {required}")
    return entries


def render_hook(
    template: Path,
    output: Path,
    helper: Path,
    version: str,
    digest: str,
    expanded_bytes: int,
    contract_sha256: str,
) -> None:
    rendered = template.read_text(encoding="utf-8")
    replacements = {
        "@@PAYLOAD_SCHEMA@@": PAYLOAD_SCHEMA,
        "@@INSTALLED_RUNTIME_SCHEMA@@": INSTALLED_RUNTIME_SCHEMA,
        "@@APP_VERSION@@": version,
        "@@PAYLOAD_FILENAME@@": PAYLOAD_FILENAME,
        "@@PAYLOAD_SHA256@@": digest,
        "@@RUNTIME_CONTRACT_SHA256@@": contract_sha256,
        "@@PAYLOAD_EXPANDED_BYTES@@": str(expanded_bytes),
        "@@INSTALLER_HELPER_SOURCE@@": str(helper.resolve()).replace("/", "\\"),
    }
    for token, value in replacements.items():
        if token not in rendered:
            raise RuntimeError(f"installer hook template token missing: {token}")
        rendered = rendered.replace(token, value)
    if "@@" in rendered:
        raise RuntimeError("installer hook contains unresolved placeholders")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered, encoding="utf-8", newline="\n")


def write_evidence(
    path: Path,
    payload: Path,
    version: str,
    digest: str,
    expanded_bytes: int,
    root_bytes: dict[str, int],
    roots: list[str],
    contract_sha256: str,
    entries: list[str],
    seven_zip: str,
    tar_program: str,
    native_pins: str,
) -> None:
    body = {
        "schema": PAYLOAD_SCHEMA,
        "installed_runtime_schema": INSTALLED_RUNTIME_SCHEMA,
        "app_version": version,
        "payload_file": PAYLOAD_FILENAME,
        "payload_sha256": digest,
        "native_dll_pins": native_pins,
        "payload_bytes": payload.stat().st_size,
        "expanded_bytes": expanded_bytes,
        "compression_ratio": round(payload.stat().st_size / expanded_bytes, 6)
        if expanded_bytes
        else None,
        "payload_root_bytes": root_bytes,
        "runtime_contract_sha256": contract_sha256,
        "archive_format": "7z",
        "compression": "LZMA2",
        "compression_level": 9,
        "build_7zip_program": seven_zip,
        "target_extract_program": "Windows tar.exe / bsdtar",
        "validation_tar_program": tar_program,
        "entry_count": len(entries),
        "payload_roots": roots,
        "user_facing_files": ["TranslateIT-Setup.exe", PAYLOAD_FILENAME],
        "installer_mode": "perMachine",
        "network_download": False,
        "manual_extraction": False,
        "second_user_facing_installer": False,
        "target_pc_acceptance": "deferred",
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(body, indent=2) + "\n", encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--hook-template", required=True, type=Path)
    parser.add_argument("--installer-helper", required=True, type=Path)
    parser.add_argument("--generated-hook", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    # The implementation below remains source-only until reviewed artifacts,
    # provenance, and integrated target acceptance permit release.
    raise RuntimeError("Native packaging is held: native payload builder is not release-admitted.")
    repo_root = args.repo_root.resolve()
    if not (repo_root / "AGENTS.md").is_file():
        raise RuntimeError("invalid TranslateIT repository root")
    require_file(args.hook_template, "R3 NSIS hook template")
    require_file(args.installer_helper, "R3 installer helper")
    version = app_version(repo_root)
    manifest = load_native_inventory(repo_root)
    native_pins = os.environ.get("TRANSLATEIT_NATIVE_DLL_PINS", "")
    body = native_runtime_contract(repo_root, manifest, native_pins)
    roots = body["roots"]
    contract_sha256 = sha256_file(write_payload_contract(repo_root, body))
    root_bytes = {root: tree_bytes(repo_root / root) for root in roots}
    expanded_bytes = sum(root_bytes.values())
    seven_zip, tar_program = find_7zip(), find_tar()
    payload = args.output.resolve()
    entries = create_payload(repo_root, payload, seven_zip, tar_program, roots, body["files"])
    digest = sha256_file(payload)
    render_hook(args.hook_template.resolve(), args.generated_hook.resolve(),
                args.installer_helper.resolve(), version, digest, expanded_bytes, contract_sha256)
    write_evidence(args.evidence.resolve(), payload, version, digest, expanded_bytes,
                   root_bytes, roots, contract_sha256, entries, seven_zip, tar_program, native_pins)
    print(f"[r3-payload] app_version={version} sha256={digest} runtime_contract_sha256={contract_sha256}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # build boundary must fail closed with one clear reason
        print(f"[r3-payload] FAILED: {exc}", file=sys.stderr)
        raise SystemExit(1)
