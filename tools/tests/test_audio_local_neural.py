"""Model download and admission failures must never publish accepted previews."""
from __future__ import annotations

import hashlib
import io
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "audio_validation"))
import harness
import local_neural_comparison as neural


class LocalNeuralAdmissionTests(unittest.TestCase):
    def test_complete_neural_fixture_publication_and_inspection_roundtrip(self):
        # Exercise the real 48-case publication/inspection boundary; only model
        # synthesis is replaced by public deterministic PCM, never called inference.
        with tempfile.TemporaryDirectory() as work, \
                patch.object(neural, "load_engine", return_value=object()), \
                patch.object(neural, "speech_pcm", return_value=[1200] * 16000):
            root = Path(work) / "fixtures"
            generated = harness.generate(root, "", 42, neural_model=Path(work))
            verified = harness.inspect(root)
            self.assertEqual(verified["audio_fingerprint"], generated["audio_fingerprint"])
            self.assertEqual(len(verified["fixtures"]), 48)
            self.assertEqual(verified["generator"]["voice"], "M1")
            sample = root / "samples/A01.wav"
            raw = bytearray(sample.read_bytes())
            raw[-1] ^= 1
            sample.write_bytes(raw)
            with self.assertRaisesRegex(ValueError, "hash/reference/WAV differs"):
                harness.inspect(root)

    def test_neural_failure_cannot_fall_back_or_publish_fixtures(self):
        with tempfile.TemporaryDirectory() as work:
            destination = Path(work) / "fixtures"
            with patch.object(neural, "load_engine", side_effect=ValueError("asset rejected")), \
                    patch.object(harness.subprocess, "run") as external:
                with self.assertRaisesRegex(ValueError, "asset rejected"):
                    harness.generate(destination, "", 42, neural_model=Path(work))
                external.assert_not_called()
            self.assertFalse(destination.exists())

    def test_neural_speech_keeps_canonical_pause_and_quiet_tail(self):
        with tempfile.TemporaryDirectory() as work:
            calls = []

            def speech(text, row, seed):
                calls.append((text, row["language"], seed))
                return [1000] * 1600

            audio = harness.generate_samples(
                {"id": "A08", "kind": "speech", "language": "id", "reference": "Satu, dua.",
                 "comma_pause_ms": 1100, "suffix_ms": 350}, "", Path(work), 42, speech)
            self.assertEqual(calls, [("Satu", "id", 42), ("dua.", "id", 42)])
            self.assertEqual(audio[:1600], [1000] * 1600)
            self.assertEqual(audio[1600:19200], [0] * 17600)
            self.assertEqual(audio[19200:20800], [1000] * 1600)
            self.assertEqual(audio[20800:], [0] * 5600)

    def model(self):
        content = b"pinned-public-test-asset"
        return content, {"repo": "public/model", "revision": "a" * 40,
                         "files": [{"path": "onnx/model.onnx", "size": len(content),
                                    "sha256": hashlib.sha256(content).hexdigest()}]}

    def test_verified_download_publishes_once_without_overwriting(self):
        content, model = self.model()
        with tempfile.TemporaryDirectory() as work, patch.object(neural, "pins", return_value=model):
            destination = Path(work) / "model"
            with patch.object(neural, "urlopen", return_value=io.BytesIO(content)):
                neural.prepare(destination)
            neural.verify_assets(destination)
            with patch.object(neural, "urlopen") as network:
                with self.assertRaisesRegex(ValueError, "already exists"):
                    neural.prepare(destination)
                network.assert_not_called()
            self.assertEqual((destination / "onnx/model.onnx").read_bytes(), content)

    def test_bad_download_never_publishes_model(self):
        content, model = self.model()
        for bad in (content[:-1], b"x" * len(content), content + b"extra"):
            with self.subTest(size=len(bad)), tempfile.TemporaryDirectory() as work:
                destination = Path(work) / "model"
                with patch.object(neural, "pins", return_value=model), \
                        patch.object(neural, "urlopen", return_value=io.BytesIO(bad)), \
                        self.assertRaises(ValueError):
                    neural.prepare(destination)
                self.assertFalse(destination.exists())
                self.assertEqual(list(Path(work).iterdir()), [])

    def test_tampered_asset_blocks_generation_before_sdk_loading(self):
        content, model = self.model()
        with tempfile.TemporaryDirectory() as work, patch.object(neural, "pins", return_value=model):
            root = Path(work) / "model"
            (root / "onnx").mkdir(parents=True)
            (root / "onnx/model.onnx").write_bytes(b"x" * len(content))
            with patch.object(neural, "version") as package_check:
                with self.assertRaisesRegex(ValueError, "hash mismatch"):
                    neural.generate(root, Path(work) / "preview")
                package_check.assert_not_called()
            self.assertFalse((Path(work) / "preview").exists())


if __name__ == "__main__":
    unittest.main()
