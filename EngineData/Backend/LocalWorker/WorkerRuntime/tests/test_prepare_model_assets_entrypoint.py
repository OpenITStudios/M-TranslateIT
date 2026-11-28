from __future__ import annotations

import importlib.util
from pathlib import Path


def load_module():
    path = Path(__file__).resolve().parents[1] / "prepare_model_assets.py"
    spec = importlib.util.spec_from_file_location("translateit_prepare_model_assets_entry", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_entry_uses_the_core_transaction_without_second_marker_write() -> None:
    module = load_module()
    assert module.replace_target_from_snapshot is module._core.replace_target_from_snapshot
    assert module.REVISION_MARKER == module._core.REVISION_MARKER


def test_entry_main_delegates_without_mutating_core(monkeypatch) -> None:
    module = load_module()
    original = module._core.replace_target_from_snapshot

    def fake_main() -> int:
        assert module._core.replace_target_from_snapshot is original
        return 17

    monkeypatch.setattr(module._core, "main", fake_main)

    assert module.main() == 17
    assert module._core.replace_target_from_snapshot is original
