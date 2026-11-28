"""Historical GPT-SoVITS inference import compatibility only.

TranslateIT consumes only the approved English VoiceLab / My Voice path. This
module keeps the pinned upstream source intact while removing WebUI and unrelated
multilingual/LoRA import requirements from the product runtime boundary.
"""

from __future__ import annotations

import importlib
import importlib.machinery
import os
import sys
import types
from pathlib import Path
from typing import Any

import ffmpeg
import numpy as np


class _EnglishOnlyLangSegmenter:
    @staticmethod
    def getTexts(*_args: Any, **_kwargs: Any) -> list[dict[str, str]]:
        raise RuntimeError("voice_lab:non_english_text_segmentation_not_supported")


def _unsupported_chinese_attribute(name: str) -> Any:
    if name.startswith("__"):
        raise AttributeError(name)
    raise RuntimeError("voice_lab:non_english_text_processing_not_supported")


def _unsupported_lora(*_args: Any, **_kwargs: Any) -> Any:
    raise RuntimeError("voice_lab:unsupported_lora_tts_path")


def clean_path(path_value: Any) -> str:
    path = str(path_value)
    if path.endswith(("\\", "/")):
        return clean_path(path[:-1])
    return path.replace("/", os.sep).replace("\\", os.sep).strip(" '\n\"\u202a")


def _ffmpeg_program(source_root: Path) -> Path:
    return source_root / ("ffmpeg.exe" if os.name == "nt" else "ffmpeg")


def _load_audio(source_root: Path, file: Any, sample_rate: Any) -> np.ndarray:
    audio_path = clean_path(file)
    if not os.path.isfile(audio_path):
        raise RuntimeError("voice_lab:upstream_audio_missing")
    ffmpeg_program = _ffmpeg_program(source_root)
    if not ffmpeg_program.is_file():
        raise RuntimeError("voice_lab:ffmpeg_runtime_missing")
    try:
        output, _ = (
            ffmpeg.input(audio_path, threads=0)
            .output(
                "-",
                format="f32le",
                acodec="pcm_f32le",
                ac=1,
                ar=int(sample_rate),
            )
            .run(
                cmd=[str(ffmpeg_program), "-nostdin"],
                capture_stdout=True,
                capture_stderr=True,
            )
        )
    except Exception as exc:
        raise RuntimeError("voice_lab:upstream_audio_decode_failed") from exc
    return np.frombuffer(output, np.float32).flatten()


def _module(name: str) -> types.ModuleType:
    module = types.ModuleType(name)
    module.__file__ = str(Path(__file__).resolve())
    module.__spec__ = importlib.machinery.ModuleSpec(name, loader=None)
    return module


def _install_english_only_import_shims() -> None:
    text_package = importlib.import_module("text")

    chinese_module = sys.modules.get("text.chinese")
    if chinese_module is None:
        chinese_module = _module("text.chinese")
        chinese_module.__getattr__ = _unsupported_chinese_attribute
        sys.modules["text.chinese"] = chinese_module
    setattr(text_package, "chinese", chinese_module)

    segmenter_module = sys.modules.get("text.LangSegmenter")
    if segmenter_module is None:
        segmenter_module = _module("text.LangSegmenter")
        segmenter_module.LangSegmenter = _EnglishOnlyLangSegmenter
        sys.modules["text.LangSegmenter"] = segmenter_module

    if "peft" not in sys.modules:
        peft_module = _module("peft")
        peft_module.LoraConfig = _unsupported_lora
        peft_module.get_peft_model = _unsupported_lora
        sys.modules["peft"] = peft_module


def _install_english_only_tts_model_init() -> None:
    tts_module = importlib.import_module("TTS_infer_pack.TTS")
    # Use the locked SoundFile decoder; torchaudio 2.11 load requires TorchCodec.
    tts_module.torchaudio = types.SimpleNamespace(
        load=_load_reference_audio,
        transforms=tts_module.torchaudio.transforms,
    )
    tts_class = tts_module.TTS
    if getattr(tts_class, "_translateit_english_only_model_init", False):
        return

    def _init_models(self: Any) -> None:
        self.bert_model = None
        self.bert_tokenizer = None
        self.init_t2s_weights(self.configs.t2s_weights_path)
        self.init_vits_weights(self.configs.vits_weights_path)
        self.init_cnhuhbert_weights(self.configs.cnhuhbert_base_path)

    tts_class._init_models = _init_models
    tts_class._translateit_english_only_model_init = True


def _load_reference_audio(path: Any) -> tuple[Any, int]:
    import soundfile
    import torch

    audio, sample_rate = soundfile.read(path, dtype="float32", always_2d=True)
    return torch.from_numpy(audio.T.copy()), sample_rate


def install_headless_my_utils(source_root: Path) -> None:
    import nltk

    source_root = source_root.resolve()
    gpt_sovits_root = source_root / "GPT_SoVITS"
    if not gpt_sovits_root.is_dir():
        raise RuntimeError("voice_lab:gpt_sovits_source_missing")

    nltk_root = str(source_root / "nltk_data")
    if nltk_root not in nltk.data.path:
        nltk.data.path.insert(0, nltk_root)

    for path in (str(gpt_sovits_root), str(source_root)):
        if path not in sys.path:
            sys.path.insert(0, path)

    _install_english_only_import_shims()

    tools_package = importlib.import_module("tools")
    module = _module("tools.my_utils")
    module.clean_path = clean_path
    module.load_audio = lambda file, sr: _load_audio(source_root, file, sr)
    sys.modules["tools.my_utils"] = module
    setattr(tools_package, "my_utils", module)

    _install_english_only_tts_model_init()
