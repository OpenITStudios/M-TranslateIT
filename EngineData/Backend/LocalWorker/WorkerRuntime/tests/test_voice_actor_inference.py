from __future__ import annotations

import json
import wave
from pathlib import Path

import pytest
from test_worker_contract import load_worker_module


def load_provider_module():
    return load_worker_module().io_runtime.voice_actor_provider


@pytest.mark.parametrize(('text', 'spoken'), [
    ('The correct number is 125. Not 150.',
     'The correct number is one hundred and twenty-five. Not 150.'),
    ('Use 125.5. Not 150.', 'Use one hundred and twenty-five point five. Not 150.'),
    ('Use 1,125. Not 150.', 'Use one thousand, one hundred and twenty-five. Not 150.'),
    ('Use -125. Not 150.', 'Use minus one hundred and twenty-five. Not 150.'),
    ('Use 00125. Not 150.', 'Use zero zero one two five. Not 150.'),
    ('Use the 125th item. Not 150th.', 'Use the 125th item. Not 150th.'),
    ('Use 125, not 150.', 'Use 125, not 150.'),
    ('Version 1.2.3. Not ID125.', 'Version 1.2.3. Not ID125.'),
    ('The payment is $125. Please confirm.',
     'The payment is one hundred and twenty-five dollars. Please confirm.'),
    ('Charge $1.50. Then stop.', 'Charge one dollar and fifty cents. Then stop.'),
    ('The balance is £1.01. Please check.',
     'The balance is one pound and one penny. Please check.'),
    ('The invoice is £1,125. Please pay.',
     'The invoice is one thousand, one hundred and twenty-five pounds. Please pay.'),
    ('It costs $0.05. Confirm.', 'It costs five cents. Confirm.'),
    ('The budget is $0.00. Next step.', 'The budget is zero dollars. Next step.'),
    ('We owe $-125. Please check.', 'We owe minus one hundred and twenty-five dollars. Please check.'),
    ('Keep amount $125, not $150.', 'Keep amount $125, not $150.'),
    ('Start at 18:00. Please confirm.',
     "Start at six o'clock P M. Please confirm."),
    ('Meet at 08:30. The room is ready.',
     'Meet at eight thirty A M. The room is ready.'),
    ('We start at 08:05. Confirm.',
     'We start at eight oh five A M. Confirm.'),
    ('Meet at 00:00. Tomorrow.',
     "Meet at twelve o'clock A M. Tomorrow."),
    ('Meet at 12:00. Tomorrow.',
     "Meet at twelve o'clock P M. Tomorrow."),
    ('We will finish at 23:59. Thank you.',
     'We will finish at eleven fifty-nine P M. Thank you.'),
    ('First. Second. Third. Meet at 18:00. Then proceed.',
     "First. Second. Third. Meet at six o'clock P M. Then proceed."),
    ('Meet at 18:00. Pay $125. Confirm.',
     "Meet at six o'clock P M. Pay one hundred and twenty-five dollars. Confirm."),
    ('The recorded time is 18:00:30. Next.', 'The recorded time is 18:00:30. Next.'),
    ('Timestamp 2026-11-06T18:00. Next.', 'Timestamp 2026-11-06T18:00. Next.'),
    ('UTC+07:00 is the offset. Next.', 'UTC+07:00 is the offset. Next.'),
    ('This endpoint is https://example.com:18:00. Next.',
     'This endpoint is https://example.com:18:00. Next.'),
    ('The invalid time is 24:00. Next.', 'The invalid time is 24:00. Next.'),
    ('The invalid time is 18:60. Next.', 'The invalid time is 18:60. Next.'),
    ('The opening hours are 18:00-19:00. Next.',
     'The opening hours are 18:00-19:00. Next.'),
    ('The opening hours are 18:00–19:00. Next.',
     'The opening hours are 18:00–19:00. Next.'),
    ('Time span 18:00~19:00. Next.', 'Time span 18:00~19:00. Next.'),
    ('Time with offset 18:00+07:00. Next.',
     'Time with offset 18:00+07:00. Next.'),
    ('Keep the API, JSON, GPU, and URL names.',
     'Keep the API, JSON, GPU, and URL names.'),
    ('Use the JSON.parse API with HTTP/2.',
     'Use the JSON dot parse API with HTTP version two.'),
    ('Call JSON.parse() before continuing.',
     'Call JSON dot parse() before continuing.'),
    ('First. Second. Third. Fourth. Fifth. Call JSON.parse now.',
     'First. Second. Third. Fourth. Fifth. Call JSON dot parse now.'),
    ('HTTP/1.0 and HTTP/1.1 remain supported.',
     'HTTP version one point zero and HTTP version one point one remain supported.'),
    ('HTTP/3 is not enabled yet.',
     'HTTP version three is not enabled yet.'),
    ('Keep GPT-SoVITS, API, JSON, GPU, and URL unchanged.',
     'Keep GPT-SoVITS, API, JSON, GPU, and URL unchanged.'),
    ('We use C++ and C# in this project.',
     'We use C plus plus and C sharp in this project.'),
    ('C++ is not C#.',
     'C plus plus is not C sharp.'),
    ('The server is written in C++. Please confirm.',
     'The server is written in C plus plus. Please confirm.'),
    ('Keep C++17, C#12, and C++/CLI as identifiers.',
     'Keep C++17, C#12, and C++/CLI as identifiers.'),
    ('Do not rewrite C++::vector or pkg-C#.',
     'Do not rewrite C++::vector or pkg-C#.'),
    ('Keep lib.C++ and src/C# folder names.',
     'Keep lib.C++ and src/C# folder names.'),
    ('Keep C++.hpp and C#.cs filenames.',
     'Keep C++.hpp and C#.cs filenames.'),
    ('The URL is https://example.com/C++. Confirm.',
     'The URL is https://example.com/C++. Confirm.'),
    ('C# is different from C.',
     'C sharp is different from C.'),
    ('Use PyTorch for the next benchmark.',
     'Use pie torch for the next benchmark.'),
    ('PyTorch is ready. Please confirm.',
     'pie torch is ready. Please confirm.'),
    ('We need PyTorch 2.4 before release.',
     'We need pie torch 2.4 before release.'),
    ('The voice engine uses GPT-SoVITS and PyTorch.',
     'The voice engine uses GPT-SoVITS and pie torch.'),
    ('Keep SoVITS and GPT-SoVITS unchanged.',
     'Keep SoVITS and GPT-SoVITS unchanged.'),
    ('The URL is https://pytorch.org/ and the docs mention PyTorch.',
     'The URL is https://pytorch.org/ and the docs mention pie torch.'),
    ('Read https://example.com/PyTorch before merging.',
     'Read https://example.com/PyTorch before merging.'),
    ('Keep torch.PyTorch and PyTorch.module.',
     'Keep torch.PyTorch and PyTorch.module.'),
    ('Do not rename PyTorch2 or PyTorch-based.',
     'Do not rename PyTorch2 or PyTorch-based.'),
    ('The account is @PyTorch, not the framework.',
     'The account is @PyTorch, not the framework.'),
    ('Leave PYTORCH and pytorch untouched.',
     'Leave PYTORCH and pytorch untouched.'),
    ('Do not rewrite #PyTorch tags.',
     'Do not rewrite #PyTorch tags.'),
    ('The docs URL is https://example.com/JSON.parse.',
     'The docs URL is https://example.com/JSON.parse.'),
    ('The endpoint is https://example.com/HTTP/2.',
     'The endpoint is https://example.com/HTTP/2.'),
    ('Preserve module.JSON.parse and JSON.parseExtra.',
     'Preserve module.JSON.parse and JSON.parseExtra.'),
    ('Do not rewrite JSON.parse.more or HTTP/2.0.',
     'Do not rewrite JSON.parse.more or HTTP/2.0.'),
    ('Keep HTTP/20 and HTTP/2-alpha intact.',
     'Keep HTTP/20 and HTTP/2-alpha intact.'),
    ('Keep pkg-HTTP/2 and obj-JSON.parse as identifiers.',
     'Keep pkg-HTTP/2 and obj-JSON.parse as identifiers.'),
    ('API v1.2.3 is ready.',
     'API version one point two point three is ready.'),
    ('Upgrade GPU firmware v0.9.12. Then confirm.',
     'Upgrade GPU firmware version zero point nine point twelve. Then confirm.'),
    ('Version 2.10.0 is approved.',
     'Version two point ten point zero is approved.'),
    ('Use v1.02.003 for the deployment.',
     'Use version one point zero two point zero zero three for the deployment.'),
    ('Release V1.2.3.4 is stable.',
     'Release Version one point two point three point four is stable.'),
    ('The URL is https://example.com/v1.2.3. Confirm.',
     'The URL is https://example.com/v1.2.3. Confirm.'),
    ('IP address 192.168.0.1 is allowed.',
     'IP address 192.168.0.1 is allowed.'),
    ('Do not rewrite model_v1.2.3.',
     'Do not rewrite model_v1.2.3.'),
    ('The prerelease v1.2.3-rc1 is unchanged.',
     'The prerelease v1.2.3-rc1 is unchanged.'),
    ('Do not rewrite v1.2.3.4.5.',
     'Do not rewrite v1.2.3.4.5.'),
])
def test_english_tts_preserves_sentence_cardinals(text: str, spoken: str) -> None:
    inputs = load_provider_module().english_tts_inputs(text, Path('reference.wav'), text)
    assert inputs['text'] == spoken
    assert inputs['prompt_text'] == spoken


def write_reference_wav(path: Path, duration_ms: int = 4_000) -> None:
    sample_rate = 32_000
    frames = sample_rate * duration_ms // 1_000
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(sample_rate)
        writer.writeframes(b"\0\0" * frames)


def write_actor(root: Path, revision: str | None = None) -> None:
    root.mkdir(parents=True, exist_ok=True)
    (root / "gpt.ckpt").write_bytes(b"gpt")
    (root / "sovits.pth").write_bytes(b"sovits")
    write_reference_wav(root / "reference.wav")
    (root / "actor.json").write_text(
        json.dumps(
            {
                "schema_version": 1,
                "engine": "gpt-sovits-v2proplus",
                "engine_revision": revision or "d523079fc05d9a8028d6085bffe4a2757c32abb6",
                "gpt_weight_file": "gpt.ckpt",
                "sovits_weight_file": "sovits.pth",
                "reference_wav_file": "reference.wav",
                "reference_text": "Tomorrow we will review the final project timeline.",
                "reference_duration_ms": 4_000,
                "held_out_evaluation_complete": True,
            }
        ),
        encoding="utf-8",
    )


def test_actor_package_validation_matches_approved_contract(tmp_path: Path) -> None:
    provider = load_provider_module()
    actor = tmp_path / "MyVoice"
    write_actor(actor)
    package = provider.validate_actor_package(actor)
    assert package["reference_duration_ms"] == 4_000
    assert package["gpt_path"] == actor / "gpt.ckpt"
    assert len(package["fingerprint"]) == 5
    write_actor(actor, revision="0" * 40)
    with pytest.raises(provider.VoiceLabProviderError, match="actor_engine_contract_mismatch"):
        provider.validate_actor_package(actor)


def test_reference_wav_metrics_preserve_original_sample_math(tmp_path: Path) -> None:
    import math
    import struct

    provider = load_provider_module()
    samples = (
        -32_768,
        -32_760,
        -32_759,
        -129,
        -128,
        -1,
        0,
        1,
        128,
        129,
        32_759,
        32_760,
        32_767,
    ) * 100
    path = tmp_path / "mixed-reference.wav"
    with wave.open(str(path), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(32_000)
        writer.writeframes(struct.pack(f"<{len(samples)}h", *samples))

    # Reference equations are the pre-optimization behavior. In particular,
    # the silence/active boundary and clipping boundary must remain exact.
    active = [sample for sample in samples if abs(sample) > 128]
    expected = {
        "duration_ms": len(samples) * 1_000 / 32_000,
        "active_rms": math.sqrt(sum(sample * sample for sample in active) / len(active)),
        "dc_offset": abs(sum(samples) / len(samples)),
        "silence_fraction": sum(abs(sample) <= 128 for sample in samples) / len(samples),
        "clipping_fraction": sum(abs(sample) >= 32_760 for sample in samples) / len(samples),
    }
    assert provider.wav_signal_metrics_pcm16(path) == expected


def test_reference_wav_metrics_preserve_silent_reference_behavior(tmp_path: Path) -> None:
    import struct

    provider = load_provider_module()
    path = tmp_path / "silent-reference.wav"
    samples = (0, -128, 128) * 100
    with wave.open(str(path), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(32_000)
        writer.writeframes(struct.pack(f"<{len(samples)}h", *samples))

    metrics = provider.wav_signal_metrics_pcm16(path)
    assert metrics["active_rms"] == 0.0
    assert metrics["silence_fraction"] == 1.0
    assert metrics["clipping_fraction"] == 0.0
    assert metrics["dc_offset"] == 0.0


def test_voice_wav_writer_preserves_pcm16_bytes_without_extra_serialize(tmp_path: Path) -> None:
    import numpy as np

    provider = load_provider_module()
    samples = np.array([-32768, -32000, -128, 0, 128, 16000, 32767], dtype=np.int16)
    output = tmp_path / "voice-pcm16.wav"
    provider.write_wav(output, 32_000, samples)

    with wave.open(str(output), "rb") as reader:
        assert reader.getnchannels() == 1
        assert reader.getsampwidth() == 2
        assert reader.getframerate() == 32_000
        assert reader.readframes(reader.getnframes()) == samples.tobytes()


def test_voice_wav_writer_supports_strided_pcm16_audio(tmp_path: Path) -> None:
    import numpy as np

    provider = load_provider_module()
    samples = np.arange(-1000, 1000, dtype=np.int16)[::3]
    assert not samples.flags.c_contiguous
    output = tmp_path / "voice-strided.wav"
    provider.write_wav(output, 24_000, samples)

    with wave.open(str(output), "rb") as reader:
        assert reader.getframerate() == 24_000
        assert reader.readframes(reader.getnframes()) == samples.tobytes()


def test_voice_wav_writer_preserves_float_quantization(tmp_path: Path) -> None:
    import numpy as np

    provider = load_provider_module()
    samples = np.array([[-1.5, -1.0, -0.5, 0.0], [0.5, 1.0, 1.5, 0.25]], dtype=np.float32)
    expected = (np.clip(samples.reshape(-1), -1.0, 1.0) * 32767.0).astype(np.int16)
    output = tmp_path / "voice-float.wav"
    provider.write_wav(output, 32_000, samples)

    with wave.open(str(output), "rb") as reader:
        assert reader.readframes(reader.getnframes()) == expected.tobytes()


@pytest.mark.parametrize("kind", ["pcm16", "sub_lsb_float"])
def test_voice_wav_writer_refuses_silent_generated_pcm_before_creating_file(
    tmp_path: Path, kind: str
) -> None:
    import numpy as np

    provider = load_provider_module()
    samples = (
        np.zeros(64, dtype=np.int16)
        if kind == "pcm16"
        else np.full(64, 1e-7, dtype=np.float32)
    )
    output = tmp_path / f"silent-{kind}.wav"
    with pytest.raises(provider.VoiceLabProviderError, match="inference_audio_silent"):
        provider.write_wav(output, 32_000, samples)
    assert not output.exists()


def test_voice_actor_synthesis_does_not_admit_silent_tts_audio(tmp_path: Path) -> None:
    import numpy as np

    provider = load_provider_module()

    class SilentTTS:
        def run(self, _inputs):
            yield 32_000, np.zeros(64, dtype=np.int16)

    runtime = {
        "tts": SilentTTS(),
        "reference_wav": tmp_path / "reference.wav",
        "reference_text": "The scheduled meeting will start tomorrow.",
    }
    output = tmp_path / "silent-synthesis.wav"
    with pytest.raises(provider.VoiceLabProviderError, match="inference_audio_silent"):
        provider.synthesize_voice_actor(runtime, "The meeting starts now.", output)
    assert not output.exists()


def test_voice_wav_writer_still_refuses_empty_audio(tmp_path: Path) -> None:
    import numpy as np

    provider = load_provider_module()
    with pytest.raises(provider.VoiceLabProviderError, match="evaluation_audio_empty"):
        provider.write_wav(tmp_path / "empty.wav", 32_000, np.empty(0, dtype=np.int16))


@pytest.mark.parametrize("invalid_sample", [float("nan"), float("inf"), -float("inf")])
def test_voice_wav_writer_rejects_nonfinite_float_audio_before_creating_file(
    tmp_path: Path, invalid_sample: float
) -> None:
    import numpy as np

    provider = load_provider_module()
    output = tmp_path / "nonfinite.wav"
    samples = np.array([0.25, invalid_sample, -0.25], dtype=np.float32)
    with pytest.raises(provider.VoiceLabProviderError, match="evaluation_audio_nonfinite"):
        provider.write_wav(output, 32_000, samples)
    assert not output.exists()


@pytest.mark.parametrize("sample_rate", [0, 7_999, 192_001])
def test_voice_wav_writer_rejects_unplayable_sample_rates(tmp_path: Path, sample_rate: int) -> None:
    import numpy as np

    provider = load_provider_module()
    output = tmp_path / "unplayable-rate.wav"
    with pytest.raises(provider.VoiceLabProviderError, match="evaluation_sample_rate_unsupported"):
        provider.write_wav(output, sample_rate, np.array([0, 1], dtype=np.int16))
    assert not output.exists()


def test_actor_wav_duration_avoids_recomputing_reference_quality_metrics(
    tmp_path: Path, monkeypatch
) -> None:
    provider = load_provider_module()
    path = tmp_path / "actor-reference.wav"
    write_reference_wav(path, duration_ms=4_000)

    def forbid_full_quality_metrics(_path: Path):
        raise AssertionError("actor identity only requires verified duration")

    monkeypatch.setattr(provider, "wav_signal_metrics_pcm16", forbid_full_quality_metrics)
    assert provider.wav_duration_ms(path) == 4_000
    # Repeat as a static readiness probe would; no quality analysis is needed.
    assert provider.wav_duration_ms(path) == 4_000


def test_actor_duration_rejects_truncated_and_wrong_format_references(tmp_path: Path) -> None:
    provider = load_provider_module()
    truncated = tmp_path / "truncated.wav"
    write_reference_wav(truncated)
    truncated.write_bytes(truncated.read_bytes()[:-64])
    with pytest.raises(provider.VoiceLabProviderError, match="empty_take:truncated.wav"):
        provider.wav_duration_ms(truncated)

    wrong_rate = tmp_path / "wrong-rate.wav"
    with wave.open(str(wrong_rate), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(16_000)
        writer.writeframes(b"\0\0" * 16_000)
    with pytest.raises(provider.VoiceLabProviderError, match="noncanonical_take:wrong-rate.wav"):
        provider.wav_duration_ms(wrong_rate)


def test_actor_duration_and_preview_quality_use_identical_validated_frames(tmp_path: Path) -> None:
    provider = load_provider_module()
    path = tmp_path / "shared-reference.wav"
    write_reference_wav(path, duration_ms=4_000)
    assert provider.wav_duration_ms(path) == int(
        provider.wav_signal_metrics_pcm16(path)["duration_ms"]
    )


def test_source_working_directory_is_restored(tmp_path: Path) -> None:
    provider = load_provider_module()
    original = Path.cwd()
    target = tmp_path / "source"
    target.mkdir()
    with provider.source_working_directory(target):
        assert Path.cwd() == target
    assert Path.cwd() == original


def test_worker_reuses_actor_runtime_until_package_identity_changes(monkeypatch) -> None:
    worker = load_worker_module()
    worker.clear_voice_actor_runtime()
    current = {"fingerprint": (("actor.json", 1, 1),)}
    loads: list[object] = []
    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "validate_actor_package",
        lambda _path: {"fingerprint": current["fingerprint"]},
    )

    def load_runtime(_source, _actor):
        loads.append(current["fingerprint"])
        return {
            "fingerprint": current["fingerprint"],
            "device": "cpu",
            "reference_cached": True,
        }

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "load_voice_actor_runtime",
        load_runtime,
    )
    first = worker.get_voice_actor_runtime()
    second = worker.get_voice_actor_runtime()
    assert first is second
    assert len(loads) == 1
    current["fingerprint"] = (("actor.json", 2, 2),)
    third = worker.get_voice_actor_runtime()
    assert third is not first
    assert len(loads) == 2


def test_voice_actor_synthesis_uses_only_myvoice_path(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])
    fingerprint = (("actor.json", 1, 1),)
    validations: list[Path] = []

    def validate_actor(root: Path):
        validations.append(root)
        return {"fingerprint": fingerprint}

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "validate_actor_package",
        validate_actor,
    )
    monkeypatch.setattr(
        worker.io_runtime,
        "get_voice_actor_runtime",
        lambda _package=None: {
            "device": "cpu",
            "reference_cached": True,
            "fingerprint": fingerprint,
        },
    )

    def synthesize(_runtime, _text, output_path):
        output_path.write_bytes(b"R" * 80)
        return {"sample_rate": 32_000, "device": "cpu", "reference_cached": True}

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "synthesize_voice_actor",
        synthesize,
    )
    output = cache / "myvoice.wav"
    result = worker.handle_voice_actor_synthesize(
        {"text": "Hello from My Voice.", "output_path": str(output)}
    )
    assert result["ok"] is True
    assert result["voice_id"] == "MyVoice"
    assert result["sample_rate"] == 32_000
    assert Path(result["output_path"]) == output
    assert len(validations) == 1


def test_meeting_warm_actor_skips_redundant_disk_validation(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])
    fingerprint = (("actor.json", 7, 9),)
    runtime = {
        "device": "cpu",
        "reference_cached": True,
        "fingerprint": fingerprint,
    }
    monkeypatch.setattr(worker.io_runtime, "VOICE_ACTOR_RUNTIME", runtime)
    monkeypatch.setattr(worker.io_runtime, "VOICE_ACTOR_RUNTIME_FINGERPRINT", fingerprint)

    def unexpected_validation(_root):
        raise AssertionError("warm bound Meeting actor must not revalidate package from disk")

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "validate_actor_package",
        unexpected_validation,
    )

    def synthesize(received_runtime, _text, output_path):
        assert received_runtime is runtime
        output_path.write_bytes(b"R" * 80)
        return {"sample_rate": 32_000, "device": "cpu", "reference_cached": True}

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "synthesize_voice_actor",
        synthesize,
    )
    output = cache / "warm-bound.wav"
    expected = worker.io_runtime.voice_actor_package_token({"fingerprint": fingerprint})
    result = worker.handle_voice_actor_synthesize(
        {
            "text": "Warm Meeting voice.",
            "output_path": str(output),
            "expected_actor_token": expected,
        }
    )
    assert result["ok"] is True
    assert result["actor_token"] == expected
    assert Path(result["output_path"]) == output


def test_voice_actor_failure_removes_stale_output_and_never_falls_back(
    tmp_path: Path, monkeypatch
) -> None:
    worker = load_worker_module()
    worker.clear_voice_actor_runtime()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    output = cache / "stale.wav"
    output.write_bytes(b"old" * 40)
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])

    def unavailable(_package=None):
        raise worker.io_runtime.voice_actor_provider.VoiceLabProviderError("approved_actor_missing")

    monkeypatch.setattr(worker.io_runtime, "get_voice_actor_runtime", unavailable)
    result = worker.handle_voice_actor_synthesize({"text": "Hello.", "output_path": str(output)})
    assert result["ok"] is False
    assert result["blocker"] == "voice_actor:approved_actor_missing"
    assert not output.exists()


def test_hard_voice_actor_synthesis_failure_clears_cached_runtime(
    tmp_path: Path, monkeypatch
) -> None:
    worker = load_worker_module()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])

    fingerprint = (("actor.json", 11, 12),)
    runtime = {
        "device": "cpu",
        "reference_cached": True,
        "fingerprint": fingerprint,
    }
    monkeypatch.setattr(worker.io_runtime, "VOICE_ACTOR_RUNTIME", runtime)
    monkeypatch.setattr(worker.io_runtime, "VOICE_ACTOR_RUNTIME_FINGERPRINT", fingerprint)

    def fail_synthesis(_runtime, _text, output_path):
        output_path.write_bytes(b"partial")
        raise RuntimeError("inference failed")

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "synthesize_voice_actor",
        fail_synthesis,
    )

    output = cache / "failed.wav"
    expected = worker.io_runtime.voice_actor_package_token({"fingerprint": fingerprint})
    result = worker.handle_voice_actor_synthesize(
        {
            "text": "Hard failure should invalidate the actor runtime.",
            "output_path": str(output),
            "expected_actor_token": expected,
        }
    )

    assert result["ok"] is False
    assert result["blocker"] == "voice_actor:runtime_failed:RuntimeError"
    assert worker.io_runtime.VOICE_ACTOR_RUNTIME is None
    assert worker.io_runtime.VOICE_ACTOR_RUNTIME_FINGERPRINT is None
    assert not output.exists()


def test_static_worker_readiness_requires_approved_actor_and_inference_assets(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "validate_actor_package",
        lambda _root: {"fingerprint": (("actor.json", 1, 1),)},
    )
    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "inference_source_assets",
        lambda _root: {"gsv": Path("gsv")},
    )
    status = worker.voice_actor_static_status()
    assert status["ready"] is True
    assert status["actor_token"] == '[["actor.json",1,1]]'


def test_meeting_actor_token_rejects_mid_session_actor_change(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    worker.clear_voice_actor_runtime()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])
    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "validate_actor_package",
        lambda _root: {"fingerprint": (("actor.json", 2, 2),)},
    )
    output = cache / "voice.wav"
    result = worker.handle_voice_actor_synthesize(
        {
            "text": "Hello.",
            "output_path": str(output),
            "expected_actor_token": '[["actor.json",1,1]]',
        }
    )
    assert result["ok"] is False
    assert result["blocker"] == "voice_actor:actor_changed_since_meeting_start"
    assert not output.exists()


def test_worker_protocol_exposes_only_trained_actor_tts_commands() -> None:
    worker = load_worker_module()
    assert worker.HANDLERS["voice_actor_preflight"] is worker.handle_voice_actor_preflight
    assert worker.HANDLERS["voice_actor_synthesize"] is worker.handle_voice_actor_synthesize
    assert "tts_preflight" not in worker.HANDLERS
    assert "synthesize" not in worker.HANDLERS


def test_meeting_pace_factor_uses_actor_reference_rate_and_clamps() -> None:
    from voice_lab_gpt_sovits import meeting_pace_factor

    runtime = {
        "reference_text": "one two three four",
        "reference_duration_ms": 2000,
    }
    assert meeting_pace_factor(runtime, "one two three four", 2000) == 1.0
    assert meeting_pace_factor(runtime, "one two three four", 1000) == 1.10
    assert meeting_pace_factor(runtime, "one two three four", 4000) == 0.90
    assert meeting_pace_factor(runtime, "", 1000) == 1.0


def test_voice_actor_synthesize_forwards_native_speed_factor(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])

    runtime = {
        "device": "cpu",
        "reference_cached": True,
        "fingerprint": (("actor.json", 1, 1),),
        "reference_text": "one two three four",
        "reference_duration_ms": 2000,
    }
    monkeypatch.setattr(
        worker.io_runtime,
        "get_bound_voice_actor_runtime",
        lambda _expected="": (runtime, '[["actor.json",1,1]]'),
    )

    seen: list[float] = []

    def synthesize(_runtime, _text, output_path, *, speed_factor=1.0):
        seen.append(speed_factor)
        output_path.write_bytes(b"R" * 80)
        return {
            "sample_rate": 32_000,
            "device": "cpu",
            "reference_cached": True,
            "speed_factor": speed_factor,
        }

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "synthesize_voice_actor",
        synthesize,
    )
    output = cache / "paced.wav"
    result = worker.handle_voice_actor_synthesize(
        {
            "text": "Translated output.",
            "source_text": "one two three four",
            "source_speech_duration_ms": 1000,
            "output_path": str(output),
        }
    )

    assert result["ok"] is True
    assert seen == [1.10]
    assert result["pace_applied"] is True
    assert result["pace_factor"] == 1.10
