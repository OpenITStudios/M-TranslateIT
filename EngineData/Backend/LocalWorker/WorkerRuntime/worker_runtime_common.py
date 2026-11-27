from __future__ import annotations

import os
import sys
import time
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parents[4]


def configured_absolute_root(environment_name: str, fallback: Path) -> Path:
    raw = str(os.environ.get(environment_name, "")).strip()
    if not raw:
        return fallback.resolve()
    candidate = Path(raw)
    if not candidate.is_absolute():
        raise RuntimeError(f"worker:{environment_name.lower()}_must_be_absolute")
    return candidate.resolve()


RUNTIME_ROOT = configured_absolute_root("TRANSLATEIT_RUNTIME_ROOT", SCRIPT_ROOT)
USER_DATA_ROOT = configured_absolute_root("TRANSLATEIT_USER_DATA_ROOT", SCRIPT_ROOT / "UserData")
RUNTIME_ASSETS_ROOT = RUNTIME_ROOT / "EngineData" / "Backend" / "RuntimeAssets"
GPT_SOVITS_SOURCE_ROOT = RUNTIME_ASSETS_ROOT / "Voice" / "GPTSoVITS" / "Source"
VOICE_ACTOR_ROOT = USER_DATA_ROOT / "SavedProject" / "VoiceLab" / "MyVoice"
CACHE_ROOT = USER_DATA_ROOT / "CacheData"
LOG_ROOT = USER_DATA_ROOT / "LogData"
ALLOWED_INPUT_ROOTS = [CACHE_ROOT, LOG_ROOT]
ALLOWED_OUTPUT_ROOTS = [CACHE_ROOT]

MAX_WORKER_REQUEST_BYTES = 1_000_000
MAX_TTS_TEXT_CHARS = 1_000


def now_ms() -> int:
    return int(time.time() * 1000)


def request_deadline_remaining_ms(payload: dict[str, Any] | None) -> int | None:
    if not isinstance(payload, dict):
        return None
    try:
        deadline = int(payload.get("deadline_unix_ms", 0))
    except (TypeError, ValueError):
        return None
    if deadline <= 0:
        return None
    return max(0, deadline - now_ms())


def request_deadline_expired(payload: dict[str, Any] | None) -> bool:
    remaining = request_deadline_remaining_ms(payload)
    return remaining is not None and remaining <= 0


def import_ready(module_name: str) -> bool:
    try:
        __import__(module_name)
        return True
    except Exception:
        return False


def compact_runtime_text(value: Any, max_chars: int) -> str:
    text = str(value or "").strip().replace("\x00", "")
    compact = " ".join(text.split())
    return compact[:max_chars] if len(compact) > max_chars else compact


def runtime_text_too_large(value: Any, max_chars: int) -> bool:
    return len(str(value or "")) > max_chars


def safe_command_name(value: Any) -> str:
    text = str(value or "status").strip().lower()
    return "".join(ch for ch in text if ch.isascii() and (ch.isalnum() or ch == "_"))[:64]


def torch_allocator_snapshot() -> dict[str, Any]:
    """Read only the current PyTorch device allocator, without initializing CUDA.

    Allocator metrics exclude CTranslate2 and other CUDA clients. The nested
    device_memory reading reports CUDA-visible device free/total at one instant
    and must not be misread as PyTorch ownership or a peak. PyTorch peaks are
    lifetime values, not per-utterance. Unavailable is never treated as zero.
    """
    torch_module = sys.modules.get("torch")
    if torch_module is None:
        return {"state": "not_initialized"}
    try:
        cuda = getattr(torch_module, "cuda", None)
        if cuda is None:
            return {"state": "unavailable", "reason": "torch_cuda_api_missing"}
        # current_device() and allocator reads are not called before the CUDA
        # context is initialized by real model work.
        if not cuda.is_initialized():
            return {"state": "not_initialized"}
        device = cuda.current_device()
        values = {
            "allocated": cuda.memory_allocated(device),
            "reserved": cuda.memory_reserved(device),
            "peak_allocated": cuda.max_memory_allocated(device),
            "peak_reserved": cuda.max_memory_reserved(device),
        }
        if (
            type(device) is not int
            or device < 0
            or any(type(value) is not int or value < 0 for value in values.values())
            or values["allocated"] > values["reserved"]
            or values["peak_allocated"] < values["allocated"]
            or values["peak_reserved"] < values["reserved"]
        ):
            return {"state": "unavailable", "reason": "torch_allocator_values_invalid"}
        mib = 1024 * 1024
        # CUDA-reported free/total memory is device-scoped, not allocator-owned.
        # It may include CTranslate2 and other processes; WDDM observations
        # require independent confirmation during Windows acceptance.
        try:
            free_bytes, total_bytes = cuda.mem_get_info(device)
            if (
                type(free_bytes) is not int
                or type(total_bytes) is not int
                or total_bytes <= 0
                or free_bytes < 0
                or free_bytes > total_bytes
            ):
                device_memory = {
                    "state": "unavailable",
                    "reason": "cuda_device_memory_values_invalid",
                }
            else:
                device_memory = {
                    "state": "measured",
                    "free_mib": free_bytes // mib,
                    "total_mib": total_bytes // mib,
                    "used_mib": (total_bytes - free_bytes) // mib,
                }
        except Exception as exc:
            device_memory = {
                "state": "unavailable",
                "reason": f"cuda_device_memory_probe_failed:{type(exc).__name__}",
            }
        return {
            "state": "measured",
            "device_index": device,
            "allocated_mib": values["allocated"] // mib,
            "reserved_mib": values["reserved"] // mib,
            "peak_allocated_mib": values["peak_allocated"] // mib,
            "peak_reserved_mib": values["peak_reserved"] // mib,
            "device_memory": device_memory,
        }
    except Exception as exc:
        # Diagnostics must not turn an otherwise healthy Meeting into failure.
        return {
            "state": "unavailable",
            "reason": f"torch_cuda_allocator_probe_failed:{type(exc).__name__}",
        }


def resolve_worker_path(value: Any, default_path: Path, allowed_roots: list[Path]) -> Path:
    raw = str(value).strip() if value not in (None, "") else str(default_path)
    path = Path(raw)
    if not path.is_absolute():
        normalized = raw.replace("\\", "/")
        if normalized == "UserData" or normalized.startswith("UserData/"):
            path = USER_DATA_ROOT / normalized.removeprefix("UserData").lstrip("/")
        else:
            path = RUNTIME_ROOT / path
    resolved = path.resolve()
    allowed = [root.resolve() for root in allowed_roots]
    if not any(resolved == root or root in resolved.parents for root in allowed):
        raise ValueError("worker:path_outside_allowed_roots")
    return resolved
