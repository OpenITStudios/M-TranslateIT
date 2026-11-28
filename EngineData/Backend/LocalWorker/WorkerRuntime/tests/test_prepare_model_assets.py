from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path


def load_module():
    path = Path(__file__).resolve().parents[1] / "prepare_model_assets.py"
    spec = importlib.util.spec_from_file_location("translateit_prepare_model_assets", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_required_huggingface_plan_is_revision_pinned_and_runtime_asset_scoped() -> None:
    module = load_module()
    selected, manual = module.build_plan(module.load_manifest())
    assert {item["model_id"] for item in selected} == {
        "zonos3-mini-q8_0",
        "qwen3-asr-1.7b-q8_0",
        "hy-mt2-1.8b-q6_k",
    }
    assert all(module.FULL_REVISION.fullmatch(item["revision"]) for item in selected)
    assert all(item["target"].is_relative_to(module.RUNTIME_ASSETS_ROOT) for item in selected)
    assert {item["model_id"] for item in manual} == {"eres2netv2-forward3-f32"}


def test_native_assets_have_only_exact_pinned_gguf_downloads() -> None:
    module = load_module()
    manifest = module.load_manifest()
    native = {m["model_id"]: m for m in manifest["models"] if m["backend"].startswith("llama.cpp")}
    assert set(native) == {"qwen3-asr-1.7b-q8_0", "hy-mt2-1.8b-q6_k", "zonos3-mini-q8_0"}
    for model in native.values():
        assert model["required"] is True and model["cpu_fallback"] is False
        assert set(model["download_allow_patterns"]) == set(model["artifact_hashes"])
        assert all(name.endswith(".gguf") for name in model["download_allow_patterns"])
        assert all(len(h) == 64 for h in model["artifact_hashes"].values())


def test_download_allow_pattern_cannot_escape_snapshot() -> None:
    module = load_module()
    try:
        module.validate_download_allow_patterns("example", ["../outside.bin"])
    except RuntimeError as exc:
        assert "may not escape" in str(exc)
    else:
        raise AssertionError("snapshot allow pattern must not escape its model root")


def test_optional_plan_never_restores_retired_meeting_fallback_assets() -> None:
    module = load_module()
    selected, _manual = module.build_plan(module.load_manifest(), include_optional=True)
    assert {item["model_id"] for item in selected} == {
        "zonos3-mini-q8_0",
        "qwen3-asr-1.7b-q8_0",
        "hy-mt2-1.8b-q6_k",
    }


def test_specific_model_selection_rejects_manual_asset() -> None:
    module = load_module()
    try:
        module.build_plan(
            module.load_manifest(),
            requested_ids={"eres2netv2-forward3-f32"},
        )
    except RuntimeError as exc:
        assert "not Hugging Face-acquirable" in str(exc)
    else:
        raise AssertionError(
            "manual speaker verifier export must not be promoted to Hugging Face acquisition"
        )


def test_release_model_stage_uses_only_workspace_without_touching_canonical_targets(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_module()
    monkeypatch.setattr(module._core, "PROJECT_ROOT", tmp_path)
    monkeypatch.setattr(
        module._core, "RUNTIME_ASSETS_ROOT", tmp_path / "EngineData/Backend/RuntimeAssets"
    )
    canonical = module._core.RUNTIME_ASSETS_ROOT / "ASR/ModelData/example"
    items = [{"model_id": "example", "target": canonical, "revision": "a" * 40}]
    stage = tmp_path / ".workspace/staging/release-candidate/payload-tree"
    (stage / "EngineData/Backend").mkdir(parents=True)
    redirected = module._core.stage_download_plan(items, stage)
    assert (
        redirected[0]["target"] == stage / "EngineData/Backend/RuntimeAssets/ASR/ModelData/example"
    )
    assert items[0]["target"] == canonical
    assert not canonical.exists()


def test_release_model_stage_rejects_paths_outside_reserved_workspace(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_module()
    monkeypatch.setattr(module._core, "PROJECT_ROOT", tmp_path)
    monkeypatch.setattr(
        module._core, "RUNTIME_ASSETS_ROOT", tmp_path / "EngineData/Backend/RuntimeAssets"
    )
    model = module._core.RUNTIME_ASSETS_ROOT / "ASR/ModelData/example"
    for forbidden in [
        tmp_path / "UserData",
        tmp_path / "EngineData",
        tmp_path / ".workspace/staging",
    ]:
        (forbidden / "EngineData/Backend").mkdir(parents=True, exist_ok=True)
        try:
            module._core.stage_download_plan([{"target": model}], forbidden)
        except RuntimeError as exc:
            assert "below .workspace/staging" in str(exc)
        else:
            raise AssertionError("Model acquisition must not stage into runtime/user roots")


def test_failed_model_download_preserves_existing_and_interrupted_backups(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_module()
    target = tmp_path / "model"
    target.mkdir()
    (target / "old.bin").write_bytes(b"accepted")
    # This simulates a still-unverified backup from a previous interrupted run.
    previous = tmp_path / ".model.previous-previous-process"
    previous.mkdir()
    (previous / "recover.bin").write_bytes(b"important")

    def fail_download(**kwargs) -> None:
        kwargs["local_dir"].mkdir()
        (kwargs["local_dir"] / "partial.bin").write_bytes(b"incomplete")
        raise RuntimeError("controlled download failure")

    monkeypatch.setattr(module._core, "snapshot_download", fail_download)
    item = {
        "target": target,
        "model_id": "test",
        "repo_id": "offline/example",
        "revision": "a" * 40,
        "download_allow_patterns": ["partial.bin"],
        "artifact_hashes": {"partial.bin": hashlib.sha256(b"complete").hexdigest()},
    }
    try:
        module._core.replace_target_from_snapshot(item)
    except RuntimeError as exc:
        assert "controlled download failure" in str(exc)
    else:
        raise AssertionError("Failed model download must not report success")
    assert (target / "old.bin").read_bytes() == b"accepted"
    assert (previous / "recover.bin").read_bytes() == b"important"
    assert list(tmp_path.glob(".model.download-*")), "Failed temporary model remains for review"


def test_promoted_snapshot_has_revision_marker_before_directory_cutover(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_module()
    target = tmp_path / "model"
    target.mkdir()
    (target / "old.bin").write_bytes(b"old")

    def populate(**kwargs) -> None:
        staged = kwargs["local_dir"]
        staged.mkdir()
        (staged / "model.bin").write_bytes(b"new")

    monkeypatch.setattr(module._core, "snapshot_download", populate)
    item = {
        "target": target,
        "model_id": "test",
        "repo_id": "offline/example",
        "revision": "b" * 40,
        "download_allow_patterns": ["model.bin"],
        "artifact_hashes": {"model.bin": hashlib.sha256(b"new").hexdigest()},
    }
    result = module._core.replace_target_from_snapshot(item)
    assert result["replaced_existing"] is True
    assert (target / "model.bin").read_bytes() == b"new"
    assert (target / ".translateit_model_revision").read_text(encoding="utf-8") == "b" * 40 + "\n"
    assert not list(tmp_path.glob(".model.previous-*"))


def test_failed_promotion_restores_previous_model_without_deleting_backups(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_module()
    target = tmp_path / "model"
    target.mkdir()
    (target / "original.bin").write_bytes(b"original")

    def populate(**kwargs) -> None:
        staged = kwargs["local_dir"]
        staged.mkdir()
        (staged / "next.bin").write_bytes(b"next")

    monkeypatch.setattr(module._core, "snapshot_download", populate)
    original_replace = Path.replace

    def fail_new_model_cutover(self: Path, destination: Path) -> Path:
        if self.name.startswith(".model.download-"):
            raise OSError("simulated stage cutover failure")
        return original_replace(self, destination)

    monkeypatch.setattr(Path, "replace", fail_new_model_cutover)
    item = {
        "target": target,
        "model_id": "test",
        "repo_id": "offline/example",
        "revision": "c" * 40,
        "download_allow_patterns": ["next.bin"],
        "artifact_hashes": {"next.bin": hashlib.sha256(b"next").hexdigest()},
    }
    try:
        module._core.replace_target_from_snapshot(item)
    except OSError as exc:
        assert "stage cutover failure" in str(exc)
    else:
        raise AssertionError("A failed model promotion must not pass")
    assert (target / "original.bin").read_bytes() == b"original"
    assert not (target / "next.bin").exists()


def test_exact_existing_snapshot_is_reused_without_network_or_marker_write(tmp_path: Path, monkeypatch) -> None:
    module = load_module()
    target = tmp_path / "model"
    target.mkdir()
    (target / "model.gguf").write_bytes(b"good")
    marker = target / module.REVISION_MARKER
    marker.write_text("a" * 40 + "\n", encoding="utf-8")
    before = marker.stat().st_mtime_ns
    item = {"target": target, "model_id": "test", "repo_id": "offline/example",
            "revision": "a" * 40, "download_allow_patterns": ["model.gguf"],
            "artifact_hashes": {"model.gguf": hashlib.sha256(b"good").hexdigest()},
            "artifact_bytes": {"model.gguf": 4}}
    def no_download(**kwargs):
        raise AssertionError("Verified snapshot must not download again")
    monkeypatch.setattr(module._core, "snapshot_download", no_download)
    result = module.replace_target_from_snapshot(item)
    assert result["reused_verified"] is True and result["replaced_existing"] is False
    assert marker.stat().st_mtime_ns == before
    assert not list(tmp_path.glob(".model.download-*"))


def test_same_size_corruption_and_bad_download_do_not_publish(tmp_path: Path, monkeypatch) -> None:
    module = load_module()
    target = tmp_path / "model"
    target.mkdir()
    (target / "model.gguf").write_bytes(b"evil")
    (target / module.REVISION_MARKER).write_text("a" * 40 + "\n", encoding="utf-8")
    item = {"target": target, "model_id": "test", "repo_id": "offline/example",
            "revision": "a" * 40, "download_allow_patterns": ["model.gguf"],
            "artifact_hashes": {"model.gguf": hashlib.sha256(b"good").hexdigest()},
            "artifact_bytes": {"model.gguf": 4}}
    def bad_download(**kwargs):
        stage = kwargs["local_dir"]
        stage.mkdir()
        (stage / "model.gguf").write_bytes(b"oops")
    monkeypatch.setattr(module._core, "snapshot_download", bad_download)
    try:
        module.replace_target_from_snapshot(item)
    except RuntimeError as exc:
        assert "exact artifact verification" in str(exc)
    else:
        raise AssertionError("Unverified bytes must not be published")
    assert (target / "model.gguf").read_bytes() == b"evil"
    def correct_download(**kwargs):
        stage = kwargs["local_dir"]
        stage.mkdir()
        (stage / "model.gguf").write_bytes(b"good")
    monkeypatch.setattr(module._core, "snapshot_download", correct_download)
    assert module.replace_target_from_snapshot(item)["replaced_existing"] is True
    assert (target / "model.gguf").read_bytes() == b"good"


def test_model_plan_rejects_same_root_directory_redirect_before_resolving(tmp_path: Path, monkeypatch) -> None:
    import pytest

    module = load_module()
    monkeypatch.setattr(module._core, "PROJECT_ROOT", tmp_path)
    assets = tmp_path / "EngineData/Backend/RuntimeAssets"
    monkeypatch.setattr(module._core, "RUNTIME_ASSETS_ROOT", assets)
    other = assets / "ASR/ModelData/other"
    other.mkdir(parents=True)
    sentinel = other / "keep.bin"
    sentinel.write_bytes(b"preserve")
    alias = other.parent / "alias"
    try:
        alias.symlink_to(other, target_is_directory=True)
    except (OSError, NotImplementedError):
        pytest.skip("directory symlinks unavailable on this host")
    with pytest.raises(RuntimeError, match="filesystem indirection"):
        module._core.resolve_target(str(alias.relative_to(tmp_path)))
    with pytest.raises(RuntimeError, match="filesystem indirection"):
        module._core.resolve_target(str((alias / "nested").relative_to(tmp_path)))
    assert sentinel.read_bytes() == b"preserve"


def test_stage_plan_rejects_reserved_transaction_roots(tmp_path: Path, monkeypatch) -> None:
    import pytest

    module = load_module()
    monkeypatch.setattr(module._core, "PROJECT_ROOT", tmp_path)
    assets = tmp_path / "EngineData/Backend/RuntimeAssets"
    monkeypatch.setattr(module._core, "RUNTIME_ASSETS_ROOT", assets)
    for name in ("release-active", "release-rollback"):
        candidate = tmp_path / ".workspace/staging" / name
        (candidate / "EngineData/Backend").mkdir(parents=True)
        with pytest.raises(RuntimeError, match="reserved"):
            module._core.stage_download_plan([{"target": assets / "ASR/ModelData/example"}], candidate)
