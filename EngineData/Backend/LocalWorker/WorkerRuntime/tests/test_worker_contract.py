from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import types
from pathlib import Path

import pytest

WORKER_PATH = Path(__file__).resolve().parents[1] / "realtime_local_worker.py"


def load_worker_module():
    spec = importlib.util.spec_from_file_location("translateit_realtime_local_worker", WORKER_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_voice_provider_keeps_heavy_upstream_stage_off_worker_import_path() -> None:
    provider_path = WORKER_PATH.parent / "voice_lab_gpt_sovits.py"
    source = provider_path.read_text(encoding="utf-8")

    assert (
        "from voice_lab_upstream_stage import install_headless_my_utils\n"
        not in source.split("def create_tts_runtime", 1)[0]
    )
    create_runtime = source.split("def create_tts_runtime", 1)[1]
    assert "from voice_lab_upstream_stage import install_headless_my_utils" in create_runtime


def test_legacy_userdata_label_cannot_escape_allowed_root(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    user_root = tmp_path / "app-local-data"
    cache_root = user_root / "CacheData"
    common = worker.io_runtime.common
    monkeypatch.setattr(common, "USER_DATA_ROOT", user_root)
    with pytest.raises(ValueError, match="worker:path_outside_allowed_roots"):
        common.resolve_worker_path(
            "UserData/CacheData/../LogData/not-allowed.wav",
            cache_root / "default.wav",
            [cache_root],
        )


def test_configured_runtime_root_must_be_absolute(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setenv("TRANSLATEIT_TEST_ROOT", "relative/path")
    with pytest.raises(RuntimeError, match="translateit_test_root_must_be_absolute"):
        worker.io_runtime.common.configured_absolute_root("TRANSLATEIT_TEST_ROOT", tmp_path)


def test_newline_json_protocol_rejects_unknown_command() -> None:
    completed = subprocess.run(
        [sys.executable, str(WORKER_PATH)],
        input='{"command":"not_a_real_command"}\n',
        text=True,
        capture_output=True,
        timeout=5,
        check=False,
    )
    assert completed.returncode == 0
    payload = json.loads(completed.stdout.strip())
    assert payload["ok"] is False
    assert payload["blocker"] == "worker:unknown_command"


def test_newline_protocol_rejects_already_expired_request() -> None:
    completed = subprocess.run(
        [sys.executable, str(WORKER_PATH)],
        input='{"command":"ping","deadline_unix_ms":1,"deadline_ms":5000}\n',
        text=True,
        capture_output=True,
        timeout=5,
        check=False,
    )
    assert completed.returncode == 0
    payload = json.loads(completed.stdout.strip())
    assert payload["ok"] is False
    assert payload["blocker"] == "worker:request_deadline_expired"


def test_torch_allocator_snapshot_never_initializes_cuda(monkeypatch) -> None:
    worker = load_worker_module()
    common = worker.io_runtime.common
    monkeypatch.delitem(sys.modules, "torch", raising=False)
    assert common.torch_allocator_snapshot() == {"state": "not_initialized"}

    def forbidden():
        raise AssertionError("allocator observation initialized CUDA")

    cuda = types.SimpleNamespace(
        is_initialized=lambda: False,
        current_device=forbidden,
        memory_allocated=forbidden,
        memory_reserved=forbidden,
        max_memory_allocated=forbidden,
        max_memory_reserved=forbidden,
    )
    monkeypatch.setitem(sys.modules, "torch", types.SimpleNamespace(cuda=cuda))
    assert common.torch_allocator_snapshot() == {"state": "not_initialized"}


def test_torch_allocator_snapshot_reports_only_measured_pytorch_bytes(monkeypatch) -> None:
    worker = load_worker_module()
    common = worker.io_runtime.common
    mib = 1024 * 1024
    cuda = types.SimpleNamespace(
        is_initialized=lambda: True,
        current_device=lambda: 0,
        memory_allocated=lambda device: 2 * mib if device == 0 else -1,
        memory_reserved=lambda device: 4 * mib if device == 0 else -1,
        max_memory_allocated=lambda device: 3 * mib if device == 0 else -1,
        max_memory_reserved=lambda device: 5 * mib if device == 0 else -1,
        mem_get_info=lambda device: (6 * mib, 8 * mib) if device == 0 else (-1, 0),
    )
    monkeypatch.setitem(sys.modules, "torch", types.SimpleNamespace(cuda=cuda))
    assert common.torch_allocator_snapshot() == {
        "state": "measured",
        "device_index": 0,
        "allocated_mib": 2,
        "reserved_mib": 4,
        "peak_allocated_mib": 3,
        "peak_reserved_mib": 5,
        "device_memory": {
            "state": "measured",
            "free_mib": 6,
            "total_mib": 8,
            "used_mib": 2,
        },
    }


def test_torch_allocator_snapshot_cuda_attribute_failure_is_nonfatal(monkeypatch) -> None:
    worker = load_worker_module()
    common = worker.io_runtime.common

    class BrokenTorch:
        @property
        def cuda(self):
            raise RuntimeError("private CUDA initialization detail")

    monkeypatch.setitem(sys.modules, "torch", BrokenTorch())
    assert common.torch_allocator_snapshot() == {
        "state": "unavailable",
        "reason": "torch_cuda_allocator_probe_failed:RuntimeError",
    }


def test_torch_allocator_snapshot_failure_is_explicit_not_zero(monkeypatch) -> None:
    worker = load_worker_module()
    common = worker.io_runtime.common

    def broken_probe():
        raise RuntimeError("private driver message")

    cuda = types.SimpleNamespace(is_initialized=broken_probe)
    monkeypatch.setitem(sys.modules, "torch", types.SimpleNamespace(cuda=cuda))
    assert common.torch_allocator_snapshot() == {
        "state": "unavailable",
        "reason": "torch_cuda_allocator_probe_failed:RuntimeError",
    }


def test_cuda_device_memory_failure_preserves_measured_allocator(monkeypatch) -> None:
    worker = load_worker_module()
    common = worker.io_runtime.common
    mib = 1024 * 1024

    def failed_device_probe(_device):
        raise RuntimeError("sensitive driver message")

    cuda = types.SimpleNamespace(
        is_initialized=lambda: True,
        current_device=lambda: 0,
        memory_allocated=lambda _device: mib,
        memory_reserved=lambda _device: 2 * mib,
        max_memory_allocated=lambda _device: mib,
        max_memory_reserved=lambda _device: 2 * mib,
        mem_get_info=failed_device_probe,
    )
    monkeypatch.setitem(sys.modules, "torch", types.SimpleNamespace(cuda=cuda))
    snapshot = common.torch_allocator_snapshot()
    assert snapshot["state"] == "measured"
    assert snapshot["allocated_mib"] == 1
    assert snapshot["reserved_mib"] == 2
    assert snapshot["device_memory"] == {
        "state": "unavailable",
        "reason": "cuda_device_memory_probe_failed:RuntimeError",
    }


def test_cuda_device_memory_rejects_inconsistent_free_total(monkeypatch) -> None:
    worker = load_worker_module()
    common = worker.io_runtime.common
    mib = 1024 * 1024
    cuda = types.SimpleNamespace(
        is_initialized=lambda: True,
        current_device=lambda: 0,
        memory_allocated=lambda _device: mib,
        memory_reserved=lambda _device: 2 * mib,
        max_memory_allocated=lambda _device: mib,
        max_memory_reserved=lambda _device: 2 * mib,
        mem_get_info=lambda _device: (10 * mib, 8 * mib),
    )
    monkeypatch.setitem(sys.modules, "torch", types.SimpleNamespace(cuda=cuda))
    snapshot = common.torch_allocator_snapshot()
    assert snapshot["state"] == "measured"
    assert snapshot["device_memory"] == {
        "state": "unavailable",
        "reason": "cuda_device_memory_values_invalid",
    }


@pytest.mark.parametrize("command", ["asr_preload", "transcribe", "translation_preload", "translate"])
def test_python_worker_rejects_retired_inference_commands(command: str) -> None:
    completed = subprocess.run(
        [sys.executable, str(WORKER_PATH)],
        input=json.dumps({"command": command, "request_id": "retired-task"}) + "\n",
        text=True, capture_output=True, timeout=5, check=False,
    )
    assert completed.returncode == 0
    receipt = json.loads(completed.stdout)
    assert receipt["request_id"] == "retired-task"
    assert receipt["ok"] is False
    assert receipt["blocker"] == "worker:unknown_command"


def test_voice_status_never_probes_retired_meeting_models(monkeypatch) -> None:
    worker = load_worker_module()
    common = worker.io_runtime.common
    imported = []
    def observe(name):
        imported.append(name)
        return True
    monkeypatch.setattr(common, "import_ready", observe)
    monkeypatch.setattr(worker.runtime, "voice_actor_static_status",
                        lambda: {"ready": True, "actor_token": "fixture", "blocker": ""})
    monkeypatch.setattr(common, "torch_allocator_snapshot", lambda: {"state": "not_initialized"})
    result = worker.runtime.build_status_payload()
    assert imported == ["torch", "transformers"]
    assert result["ok"] is True
    assert result["readiness"] == {"voice_actor_tts": True}
    assert "selected_device" not in result and "asr_primary_model_ready" not in result
    assert result["gpu"]["torch_allocator"] == {"state": "not_initialized"}
