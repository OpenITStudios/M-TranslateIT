"""Voice-only worker. Meeting ASR and MT belong to the Rust native owner."""
from __future__ import annotations

from typing import Any

import worker_io_runtime as io_runtime
import worker_runtime_common as common

MAX_WORKER_REQUEST_BYTES = common.MAX_WORKER_REQUEST_BYTES
compact_runtime_text = common.compact_runtime_text
safe_command_name = common.safe_command_name
request_deadline_expired = common.request_deadline_expired

# Existing voice handlers retain their actor identity and output contracts.
voice_actor_static_status = io_runtime.voice_actor_static_status
get_voice_actor_runtime = io_runtime.get_voice_actor_runtime
clear_voice_actor_runtime = io_runtime.clear_voice_actor_runtime
handle_voice_actor_preflight = io_runtime.handle_voice_actor_preflight
handle_voice_actor_synthesize = io_runtime.handle_voice_actor_synthesize


def build_status_payload(_payload: dict[str, Any] | None = None) -> dict[str, Any]:
    # Do not import Faster Whisper/CTranslate2 or inspect legacy ASR/MT
    # model directories here. Those libraries remain for My Voice evaluation.
    torch_ready = common.import_ready("torch")
    transformers_ready = common.import_ready("transformers")
    actor = voice_actor_static_status()
    voice_ready = bool(actor["ready"]) and torch_ready and transformers_ready
    blocker = (
        "dependency:torch_missing" if not torch_ready
        else "dependency:transformers_missing" if not transformers_ready
        else str(actor["blocker"])
    )
    state = io_runtime.state_snapshot()
    return {
        "ok": voice_ready,
        "stage": "local_realtime_worker_preflight",
        "blocker": blocker,
        "note": "Voice capability is available; native ASR/MT readiness belongs to Rust."
        if voice_ready else "Voice capability is unavailable.",
        "readiness": {"voice_actor_tts": voice_ready},
        "dependencies": {"torch": torch_ready, "transformers": transformers_ready},
        "torch_import_ready": torch_ready,
        "transformers_import_ready": transformers_ready,
        "voice_actor_ready": voice_ready,
        "voice_actor_token": str(actor["actor_token"]),
        "tts": {
            "ready": voice_ready,
            "provider": "gpt-sovits-v2proplus" if voice_ready else None,
            "voice_id": "MyVoice" if voice_ready else None,
            "language_code": "en",
            "blocker": blocker,
            "actor_token": str(actor["actor_token"]),
        },
        "gpu": {"torch_allocator": common.torch_allocator_snapshot()},
        "loaded": {
            "voice_actor": state["voice_actor_loaded"],
            "voice_actor_device": state["voice_actor_device"],
        },
    }


def handle_status(payload: dict[str, Any]) -> dict[str, Any]:
    return build_status_payload(payload)


def handle_ping(_: dict[str, Any]) -> dict[str, Any]:
    return {"ok": True, "stage": "ping", "unix_ms": common.now_ms()}


HANDLERS = {
    "ping": handle_ping,
    "status": handle_status,
    "voice_actor_preflight": handle_voice_actor_preflight,
    "voice_actor_synthesize": handle_voice_actor_synthesize,
}
