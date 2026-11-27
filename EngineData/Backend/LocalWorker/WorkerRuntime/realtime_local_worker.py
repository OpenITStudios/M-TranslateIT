from __future__ import annotations

import json
import math
import sys
from contextlib import redirect_stdout

import realtime_local_worker_base as runtime

MAX_PROTOCOL_STAGE_CHARS = 96
MAX_PROTOCOL_BLOCKER_CHARS = 512
MAX_PROTOCOL_NOTE_CHARS = 1_000

class _NonFiniteJsonNumber(ValueError):
    pass


def _reject_non_finite_json_constant(_value: str):
    raise _NonFiniteJsonNumber


def _contains_non_finite_json_number(value) -> bool:
    if isinstance(value, float):
        return not math.isfinite(value)
    if isinstance(value, dict):
        return any(_contains_non_finite_json_number(item) for item in value.values())
    if isinstance(value, list):
        return any(_contains_non_finite_json_number(item) for item in value)
    return False


def _parse_protocol_request(raw: str):
    try:
        request = json.loads(raw, parse_constant=_reject_non_finite_json_constant)
    except _NonFiniteJsonNumber:
        return None, "worker:non_finite_json_number"
    except json.JSONDecodeError:
        return None, "worker:invalid_json"
    if _contains_non_finite_json_number(request):
        return None, "worker:non_finite_json_number"
    if not isinstance(request, dict):
        return None, "worker:request_must_be_object"
    return request, ""


def _bounded_protocol_response(payload):
    if not isinstance(payload, dict):
        return {
            "ok": False,
            "stage": "worker_error",
            "blocker": "worker:response_must_be_object",
        }
    result = dict(payload)
    if "stage" in result:
        result["stage"] = runtime.compact_runtime_text(result["stage"], MAX_PROTOCOL_STAGE_CHARS)
    if "blocker" in result:
        result["blocker"] = runtime.compact_runtime_text(
            result["blocker"], MAX_PROTOCOL_BLOCKER_CHARS
        )
    if "note" in result:
        result["note"] = runtime.compact_runtime_text(result["note"], MAX_PROTOCOL_NOTE_CHARS)
    return result


def _respond_protocol(payload, request_id: str | None = None) -> None:
    response = _bounded_protocol_response(payload)
    # Optional for standalone probes; required for Rust-managed requests.
    # Echo only the host-supplied identity, overriding a handler's field.
    if request_id is not None:
        response["request_id"] = request_id
    sys.stdout.write(json.dumps(response, ensure_ascii=False, allow_nan=False) + "\n")
    sys.stdout.flush()


def _handler_failure(exc: Exception) -> dict:
    exception_type = "".join(
        char for char in type(exc).__name__ if char.isascii() and (char.isalnum() or char == "_")
    )[:96]
    return {
        "ok": False,
        "stage": "worker_error",
        "blocker": f"worker:handler_failed:{exception_type or 'Exception'}",
        "note": str(exc),
    }


def __getattr__(name: str):
    return getattr(runtime, name)


def __dir__() -> list[str]:
    return sorted(set(globals()) | set(dir(runtime)))


def main() -> int:
    for raw in sys.stdin:
        if len(raw.encode("utf-8", errors="ignore")) > runtime.MAX_WORKER_REQUEST_BYTES:
            _respond_protocol(
                {
                    "ok": False,
                    "stage": "worker_request",
                    "blocker": "worker:request_too_large",
                    "max_bytes": runtime.MAX_WORKER_REQUEST_BYTES,
                }
            )
            continue

        request, framing_blocker = _parse_protocol_request(raw)
        if framing_blocker:
            _respond_protocol(
                {
                    "ok": False,
                    "stage": "worker_request",
                    "blocker": framing_blocker,
                }
            )
            continue
        assert request is not None
        request_id = request.get("request_id")
        if request_id is not None and (
            not isinstance(request_id, str)
            or not request_id
            or len(request_id) > 128
            or not request_id.isascii()
            or not all(char.isalnum() or char in "-_" for char in request_id)
        ):
            _respond_protocol(
                {
                    "ok": False,
                    "stage": "worker_request",
                    "blocker": "worker:invalid_request_id",
                }
            )
            continue

        command = runtime.safe_command_name(request.get("command", "status"))
        if runtime.request_deadline_expired(request):
            _respond_protocol(
                {
                    "ok": False,
                    "stage": command or "worker_request",
                    "blocker": "worker:request_deadline_expired",
                    "note": (
                        "The request reached the worker after its host deadline "
                        "and was not executed."
                    ),
                },
                request_id,
            )
            continue
        handler = runtime.HANDLERS.get(command)
        if handler is None:
            _respond_protocol(
                {
                    "ok": False,
                    "stage": command or "unknown",
                    "blocker": "worker:unknown_command",
                },
                request_id,
            )
            continue
        try:
            with redirect_stdout(sys.stderr):
                response = handler(request)
            _respond_protocol(response, request_id)
        except Exception as exc:
            _respond_protocol(_handler_failure(exc), request_id)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
