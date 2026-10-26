"""Explicit local neural listening preview; never a production TTS provider.

Only prepare downloads public pinned assets. Generate has no download fallback.
Human review is required before these samples can become quality fixtures.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import tempfile
import time
from html import escape
from importlib.metadata import version
from pathlib import Path
from urllib.request import urlopen

from harness import fingerprint, metrics, specs
from natural_voice_comparison import CASES

HERE = Path(__file__).resolve().parent
PINS = HERE / "local_neural_assets.json"
VOICES = ("M1", "F1")
SCHEMA = "translateit.local_neural_comparison.v1"
PACKAGES = {"supertonic": "1.3.1", "numpy": "1.26.4", "onnxruntime": "1.23.1",
            "soundfile": "0.13.1", "scipy": "1.17.1", "huggingface-hub": "0.36.2"}


def pins() -> dict:
    return json.loads(PINS.read_text(encoding="utf-8"))


def plan() -> dict:
    cases, identity = specs()
    selected = [row for row in cases if row["id"] in CASES]
    if len(selected) != len(CASES) or any(
        row["kind"] != "speech" or row["language"] != "id" for row in selected
    ):
        raise ValueError("comparison requires the five canonical public Indonesian scripts")
    return {"schema": SCHEMA, "scenario_fingerprint": identity,
            "model": {k: pins()[k] for k in ("repo", "revision", "license")},
            "voices": list(VOICES),
            "samples": [{"id": r["id"], "reference": r["reference"]} for r in selected],
            "listening_review": "PENDING", "proof": "PLAN_ONLY"}


def verify_assets(root: Path) -> None:
    if root.is_symlink() or not root.is_dir():
        raise ValueError("model directory must be a real existing directory")
    for row in pins()["files"]:
        path = root / row["path"]
        # Validate parent links as well, before loading ONNX or voice JSON.
        candidate = root
        for part in Path(row["path"]).parts:
            candidate = candidate / part
            if candidate.is_symlink():
                raise ValueError("linked model assets are not accepted")
        if not path.is_file() or path.stat().st_size != row["size"]:
            raise ValueError(f"missing/wrong-sized model asset: {row['path']}")
        if fingerprint(path) != row["sha256"]:
            raise ValueError(f"model asset hash mismatch: {row['path']}")


def prepare(destination: Path) -> None:
    if destination.exists() or destination.is_symlink():
        raise ValueError("model destination already exists; verify or choose a new directory")
    destination.parent.mkdir(parents=True, exist_ok=True)
    model = pins()
    with tempfile.TemporaryDirectory(dir=destination.parent, prefix="neural-assets-") as work:
        root = Path(work) / "model"
        root.mkdir()
        for row in model["files"]:
            target = root / row["path"]
            target.parent.mkdir(parents=True, exist_ok=True)
            url = f"https://huggingface.co/{model['repo']}/resolve/{model['revision']}/{row['path']}"
            with urlopen(url, timeout=120) as response, target.open("xb") as output:
                remaining = row["size"]
                while block := response.read(min(1024 * 1024, remaining + 1)):
                    remaining -= len(block)
                    if remaining < 0:
                        raise ValueError("download exceeds pinned asset size")
                    output.write(block)
            print(f"Downloaded {row['path']}", flush=True)
        verify_assets(root)
        root.rename(destination)


def review_html(data: dict) -> str:
    rows = []
    for row in data["files"]:
        rows.append(f'<article><h2>{escape(row["id"])} · {escape(row["voice"])}</h2>'
                    f'<p>{escape(row["reference"])}</p>'
                    f'<audio controls preload="none" src="{escape(row["native_file"])}"></audio>'
                    f'<p><a href="{escape(row["native_file"])}">Original 44.1 kHz</a> · '
                    f'<a href="{escape(row["file"])}">ASR input 16 kHz</a></p></article>')
    return ('<!doctype html><html lang="id"><meta charset="utf-8">'
            '<meta name="viewport" content="width=device-width,initial-scale=1">'
            '<title>TranslateIT — sampel neural lokal</title><style>'
            'body{font:18px system-ui;max-width:850px;margin:40px auto;padding:20px;'
            'background:#121719;color:#e5e8eb}article{padding:16px;border-bottom:1px solid #536067}'
            'audio{width:100%}a{color:#9ed6ff}</style><h1>Sampel neural lokal</h1>'
            '<p>Audio ini sintetis. M1: pria; F1: wanita. Dengarkan kelancaran, pelafalan, '
            'negasi, angka, dan kata Inggris. Kualitas belum disetujui; ini bukan hasil tes aplikasi.</p>'
            + ''.join(rows) + '</html>')


def load_engine(model_dir: Path):
    verify_assets(model_dir)
    installed = {name: version(name) for name in PACKAGES}
    if installed != PACKAGES:
        raise ValueError("use the isolated pinned neural-fixture requirements")
    os.environ.update({"HF_HUB_OFFLINE": "1", "HF_HUB_DISABLE_TELEMETRY": "1",
                       "TRANSFORMERS_OFFLINE": "1"})
    from supertonic import TTS

    engine = TTS(model="supertonic-3", model_dir=model_dir, auto_download=False,
                 intra_op_num_threads=4, inter_op_num_threads=1)
    if engine.sample_rate != 44100:
        raise ValueError("unexpected model sample rate")
    return engine


def synthesize(engine, text: str, language: str, voice: str, seed: int, speed: float = 1.0):
    import numpy as np
    from scipy.signal import resample_poly

    if language not in ("id", "en") or voice not in VOICES:
        raise ValueError("explicit ID/EN language and approved preview voice required")
    np.random.seed(seed)
    waveform, duration = engine.synthesize(text, voice_style=engine.get_voice_style(voice),
                                           lang=language, total_steps=16, speed=speed)
    seconds = float(duration[0])
    if not 0.1 <= seconds <= 125:
        raise ValueError("unexpected generated duration")
    waveform = waveform[0, :round(seconds * engine.sample_rate)]
    if not waveform.size or not np.isfinite(waveform).all():
        raise ValueError("empty/nonfinite generated waveform")
    # Do not hide clipping or silence with automatic normalization.
    if np.max(np.abs(waveform)) >= 1 or np.sqrt(np.mean(waveform**2)) <= .0005:
        raise ValueError("clipped/silent generated waveform")
    converted = resample_poly(waveform, 160, 441)
    if not np.isfinite(converted).all() or np.max(np.abs(converted)) >= 1:
        raise ValueError("invalid/clipped resampled waveform")
    return waveform, converted


def speech_pcm(engine, text: str, row: dict, voice: str, seed: int) -> list[int]:
    import numpy as np

    _, converted = synthesize(engine, text, row["language"], voice, seed,
                              speed=row.get("rate_wpm", 155) / 155)
    return np.rint(converted * 32767).astype(np.int16).tolist()


def generate(model_dir: Path, destination: Path) -> dict:
    if destination.exists() or destination.is_symlink():
        raise ValueError("comparison destination exists; never overwrite prior samples")
    engine = load_engine(model_dir)
    import soundfile as sf

    data = plan()
    data.update({"packages": {name: version(name) for name in PACKAGES},
                 "generator_sha256": fingerprint(Path(__file__)),
                 "asset_pins_sha256": fingerprint(PINS), "seed": 42, "steps": 16,
                 "speed": 1.0, "resampler": "scipy.signal.resample_poly:160/441",
                 "provider": "CPUExecutionProvider", "files": [],
                 "proof": "GENERATED_LOCAL_NEURAL_PREVIEW_ONLY_NOT_APP_ACCEPTANCE"})
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=destination.parent, prefix="neural-preview-") as work:
        root = Path(work) / "preview"
        (root / "samples").mkdir(parents=True)
        for voice in VOICES:
            for row in data["samples"]:
                started = time.monotonic()
                waveform, converted = synthesize(engine, row["reference"], "id", voice, 42)
                seconds = len(waveform) / engine.sample_rate
                native = f"samples/{row['id']}-{voice}-44100.wav"
                canonical = f"samples/{row['id']}-{voice}-16000.wav"
                sf.write(root / native, waveform, 44100, subtype="PCM_16")
                sf.write(root / canonical, converted, 16000, subtype="PCM_16")
                checked = metrics(root / canonical)
                data["files"].append({**row, "voice": voice, "file": canonical,
                    "native_file": native, "sha256": fingerprint(root / canonical),
                    "native_sha256": fingerprint(root / native),
                    "reference_sha256": hashlib.sha256(row["reference"].encode()).hexdigest(),
                    "metrics": checked, "synthesis_ms": round((time.monotonic()-started)*1000)})
                print(f"Generated {row['id']} / {voice}: {seconds:.2f}s", flush=True)
        shutil.copyfile(model_dir / "LICENSE", root / "MODEL_LICENSE.txt")
        (root / "comparison.json").write_text(json.dumps(data, ensure_ascii=False, indent=2)+'\n',
                                              encoding="utf-8")
        (root / "listen.html").write_text(review_html(data), encoding="utf-8")
        root.rename(destination)
    return data


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "prepare", "generate"))
    parser.add_argument("--model-dir", type=Path)
    parser.add_argument("--destination", type=Path)
    args = parser.parse_args()
    if args.command == "plan":
        print(json.dumps(plan(), ensure_ascii=False, indent=2))
        return
    if args.model_dir is None:
        parser.error("--model-dir is required")
    if args.command == "prepare":
        prepare(args.model_dir)
    else:
        if args.destination is None:
            parser.error("generate requires --destination")
        result = generate(args.model_dir, args.destination)
        print(json.dumps({"samples": len(result["files"]), "review": "PENDING"}))


if __name__ == "__main__":
    main()
