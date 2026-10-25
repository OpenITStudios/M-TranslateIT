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
import local_neural_comparison as neural


class LocalNeuralAdmissionTests(unittest.TestCase):
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
