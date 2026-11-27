from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import voice_lab_gpt_sovits as voice_actor_provider
import worker_runtime_common as common

VOICE_ACTOR_RUNTIME: dict[str, Any] | None = None
VOICE_ACTOR_RUNTIME_FINGERPRINT: Any | None = None


def clear_voice_actor_runtime() -> None:
    global VOICE_ACTOR_RUNTIME, VOICE_ACTOR_RUNTIME_FINGERPRINT
    VOICE_ACTOR_RUNTIME = None
    VOICE_ACTOR_RUNTIME_FINGERPRINT = None


def voice_actor_blocker(exc: Exception) -> str:
    detail = str(exc).strip()
    if isinstance(exc, voice_actor_provider.VoiceLabProviderError) and detail:
        safe = "".join(ch for ch in detail if ch.isascii() and (ch.isalnum() or ch in "_:-"))
        if safe:
            return f"voice_actor:{safe[:160]}"
    return f"voice_actor:runtime_failed:{type(exc).__name__}"


def voice_actor_package_token(package: dict[str, Any]) -> str:
    fingerprint = package.get("fingerprint")
    if not isinstance(fingerprint, tuple) or not fingerprint:
        raise voice_actor_provider.VoiceLabProviderError("actor_identity_missing")
    token = json.dumps(fingerprint, ensure_ascii=True, separators=(",", ":"))
    if not token or len(token) > 512:
        raise voice_actor_provider.VoiceLabProviderError("actor_identity_invalid")
    return token


def voice_actor_static_status() -> dict[str, Any]:
    try:
        package = voice_actor_provider.validate_actor_package(common.VOICE_ACTOR_ROOT)
        voice_actor_provider.inference_source_assets(common.GPT_SOVITS_SOURCE_ROOT)
        return {"ready": True, "actor_token": voice_actor_package_token(package), "blocker": ""}
    except Exception as exc:
        clear_voice_actor_runtime()
        return {"ready": False, "actor_token": "", "blocker": voice_actor_blocker(exc)}


def get_voice_actor_runtime(package: dict[str, Any] | None = None) -> dict[str, Any]:
    global VOICE_ACTOR_RUNTIME, VOICE_ACTOR_RUNTIME_FINGERPRINT
    package = package or voice_actor_provider.validate_actor_package(common.VOICE_ACTOR_ROOT)
    fingerprint = package["fingerprint"]
    if VOICE_ACTOR_RUNTIME is not None and VOICE_ACTOR_RUNTIME_FINGERPRINT == fingerprint:
        return VOICE_ACTOR_RUNTIME
    clear_voice_actor_runtime()
    runtime = voice_actor_provider.load_voice_actor_runtime(
        common.GPT_SOVITS_SOURCE_ROOT,
        common.VOICE_ACTOR_ROOT,
        validated_package=package,
    )
    if runtime.get("fingerprint") != fingerprint:
        raise voice_actor_provider.VoiceLabProviderError("actor_changed_during_load")
    latest = voice_actor_provider.validate_actor_package(common.VOICE_ACTOR_ROOT)
    if latest["fingerprint"] != fingerprint:
        raise voice_actor_provider.VoiceLabProviderError("actor_changed_during_load")
    VOICE_ACTOR_RUNTIME = runtime
    VOICE_ACTOR_RUNTIME_FINGERPRINT = fingerprint
    return runtime


def get_bound_voice_actor_runtime(expected_token: str) -> tuple[dict[str, Any], str]:
    expected_token = expected_token.strip()
    if expected_token and VOICE_ACTOR_RUNTIME is not None:
        runtime_token = voice_actor_package_token(
            {"fingerprint": VOICE_ACTOR_RUNTIME.get("fingerprint")}
        )
        if runtime_token == expected_token:
            return VOICE_ACTOR_RUNTIME, runtime_token

    package = voice_actor_provider.validate_actor_package(common.VOICE_ACTOR_ROOT)
    package_token = voice_actor_package_token(package)
    if expected_token and package_token != expected_token:
        clear_voice_actor_runtime()
        raise voice_actor_provider.VoiceLabProviderError("actor_changed_since_meeting_start")

    runtime = get_voice_actor_runtime(package)
    runtime_token = voice_actor_package_token({"fingerprint": runtime.get("fingerprint")})
    if expected_token and runtime_token != expected_token:
        clear_voice_actor_runtime()
        raise voice_actor_provider.VoiceLabProviderError("actor_changed_since_meeting_start")
    return runtime, runtime_token


def handle_voice_actor_preflight(_payload: dict[str, Any]) -> dict[str, Any]:
    started = common.now_ms()
    try:
        runtime = get_voice_actor_runtime()
        token = voice_actor_package_token({"fingerprint": runtime.get("fingerprint")})
        return {
            "ok": True,
            "stage": "voice_actor_preflight",
            "voice_id": "MyVoice",
            "language_code": "en",
            "device": str(runtime.get("device", "unknown")),
            "reference_cached": bool(runtime.get("reference_cached")),
            "actor_token": token,
            "elapsed_ms": common.now_ms() - started,
            "blocker": "",
        }
    except Exception as exc:
        return {
            "ok": False,
            "stage": "voice_actor_preflight",
            "voice_id": "MyVoice",
            "language_code": "en",
            "actor_token": "",
            "blocker": voice_actor_blocker(exc),
            "elapsed_ms": common.now_ms() - started,
        }


def handle_voice_actor_synthesize(payload: dict[str, Any]) -> dict[str, Any]:
    started = common.now_ms()
    if common.runtime_text_too_large(payload.get("text", ""), common.MAX_TTS_TEXT_CHARS):
        return {
            "ok": False,
            "stage": "voice_actor_synthesize",
            "blocker": "voice_actor:text_too_large",
            "max_chars": common.MAX_TTS_TEXT_CHARS,
        }
    text = common.compact_runtime_text(payload.get("text", ""), common.MAX_TTS_TEXT_CHARS)
    if not text:
        return {"ok": False, "stage": "voice_actor_synthesize", "blocker": "voice_actor:empty_text"}
    output_path: Path | None = None
    try:
        output_path = common.resolve_worker_path(
            payload.get("output_path", ""),
            common.CACHE_ROOT / "voice_actor_output.wav",
            common.ALLOWED_OUTPUT_ROOTS,
        )
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.unlink(missing_ok=True)
        expected = common.compact_runtime_text(payload.get("expected_actor_token", ""), 512)
        runtime, runtime_token = get_bound_voice_actor_runtime(expected)
        source_text = common.compact_runtime_text(
            payload.get("source_text", ""), common.MAX_TTS_TEXT_CHARS
        )
        source_speech_duration_ms = max(0, int(payload.get("source_speech_duration_ms", 0) or 0))
        has_pace_evidence = bool(source_text) and source_speech_duration_ms > 0
        pace_factor = (
            voice_actor_provider.meeting_pace_factor(
                runtime,
                source_text,
                source_speech_duration_ms,
            )
            if has_pace_evidence
            else 1.0
        )
        if has_pace_evidence:
            synthesis = voice_actor_provider.synthesize_voice_actor(
                runtime,
                text,
                output_path,
                speed_factor=pace_factor,
            )
        else:
            synthesis = voice_actor_provider.synthesize_voice_actor(runtime, text, output_path)
        if not output_path.is_file() or output_path.stat().st_size <= 44:
            raise voice_actor_provider.VoiceLabProviderError("inference_audio_invalid")
        return {
            "ok": True,
            "stage": "voice_actor_synthesize",
            "voice_id": "MyVoice",
            "language_code": "en",
            "device": synthesis["device"],
            "reference_cached": synthesis["reference_cached"],
            "sample_rate": synthesis["sample_rate"],
            "pace_factor": float(synthesis.get("speed_factor", pace_factor)),
            "pace_applied": abs(float(synthesis.get("speed_factor", pace_factor)) - 1.0) >= 0.01,
            "source_speech_duration_ms": source_speech_duration_ms,
            "actor_token": runtime_token,
            "output_path": str(output_path),
            "elapsed_ms": common.now_ms() - started,
            "blocker": "",
        }
    except Exception as exc:
        clear_voice_actor_runtime()
        if output_path is not None:
            try:
                output_path.unlink(missing_ok=True)
            except Exception:
                pass
        return {
            "ok": False,
            "stage": "voice_actor_synthesize",
            "voice_id": "MyVoice",
            "language_code": "en",
            "blocker": voice_actor_blocker(exc),
            "elapsed_ms": common.now_ms() - started,
        }


def state_snapshot() -> dict[str, Any]:
    return {
        "voice_actor_loaded": VOICE_ACTOR_RUNTIME is not None,
        "voice_actor_device": (
            str(VOICE_ACTOR_RUNTIME.get("device", "not_loaded"))
            if VOICE_ACTOR_RUNTIME
            else "not_loaded"
        ),
    }
