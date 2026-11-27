"""Offline, maintainer-authorized export of the exact legacy My Voice verifier.

Not a runtime dependency, install step, network fetch, or release. Run only
in the separately approved artifact preparation environment. The application
must use reviewed immutable ONNX bytes and a build-time digest, never this
script or a dynamically trusted receipt.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys

REVISION = "d523079fc05d9a8028d6085bffe4a2757c32abb6"
CHECKPOINT_SHA256 = "4f5a0bf73c61eb41b174e1bb54e7ee3c83233892be8e0af1f187024e8e581a35"
SOURCE_BLOBS = {
    "GPT_SoVITS/eres2net/ERes2NetV2.py": "3ddc549520e4de6710cc781712587ddde94b5830",
    "GPT_SoVITS/eres2net/fusion.py": "d156a55c8c7e5a730d40c2fe30879c5b1c7cba61",
    "GPT_SoVITS/eres2net/pooling_layers.py": "c3e0eab659befa56048da550218b2a557c61d8b3",
    "GPT_SoVITS/eres2net/kaldi.py": "a24efdca00620ade610d692a2eccc7e237db2750",
    "GPT_SoVITS/sv.py": "22e703692b47e6affca73e03d3a631bcfe200706",
}
DIM = 20480
LIMIT = 512 * 1024 * 1024


def sha256(path: pathlib.Path) -> str:
    if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= LIMIT:
        raise ValueError("untrusted or oversized artifact")
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_source(root: pathlib.Path) -> None:
    for name, expected in SOURCE_BLOBS.items():
        path = root / name
        if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= 1024 * 1024:
            raise ValueError(f"untrusted source: {name}")
        data = path.read_bytes()
        digest = hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()
        if digest != expected:
            raise ValueError(f"source identity mismatch: {name}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", type=pathlib.Path, required=True)
    parser.add_argument("--checkpoint", type=pathlib.Path, required=True)
    parser.add_argument("--output-dir", type=pathlib.Path, required=True)
    args = parser.parse_args()
    root = args.source_root.resolve(strict=True)
    verify_source(root)
    if sha256(args.checkpoint) != CHECKPOINT_SHA256:
        raise ValueError("legacy checkpoint identity mismatch")
    # No automatic install/download; imports use the controlled build tooling.
    import numpy as np
    import onnx
    import onnxruntime as ort
    import torch

    sys.path.insert(0, str(root / "GPT_SoVITS" / "eres2net"))
    from ERes2NetV2 import ERes2NetV2
    import kaldi

    class LegacyForward3(torch.nn.Module):
        def __init__(self):
            super().__init__()
            self.model = ERes2NetV2(baseWidth=24, scale=4, expansion=4)

        def forward(self, x):
            return self.model.forward3(x)

    module = LegacyForward3().eval()
    module.model.load_state_dict(torch.load(args.checkpoint, map_location="cpu", weights_only=True), strict=True)
    module.float()
    # Require a new directory: failed conversion never overwrites accepted artifacts.
    args.output_dir.mkdir(parents=False, exist_ok=False)
    output = args.output_dir / "eres2netv2-forward3-f32.onnx"
    example = torch.zeros(1, 298, 80, dtype=torch.float32)
    with torch.inference_mode():
        torch.onnx.export(module, example, str(output), opset_version=17,
                          input_names=["x"], output_names=["embedding"],
                          dynamic_axes={"x": {1: "frames"}}, dynamo=False, external_data=False)
    graph = onnx.load(str(output), load_external_data=False)
    onnx.checker.check_model(graph, full_check=True)
    if any(t.data_location == onnx.TensorProto.EXTERNAL for t in graph.graph.initializer):
        raise ValueError("external graph data forbidden")
    session = ort.InferenceSession(str(output), providers=["CPUExecutionProvider"])

    # Source fixtures only: synthetic audio exercises varying frame lengths.
    # This verifies graph conversion, NOT perceptual speaker fidelity and NOT
    # the Rust feature frontend. Save exact feature/output oracle data for
    # authorized local parity comparison. No personal audio is used.
    fixtures = []
    with torch.inference_mode():
        for length in (4800, 16000, 48000):
            t = torch.arange(length, dtype=torch.float32) / 16000
            waveform = (0.12 * torch.sin(2 * torch.pi * 220 * t) +
                        0.04 * torch.sin(2 * torch.pi * (700 * t + 80 * t * t)))
            features = kaldi.fbank(waveform.unsqueeze(0), num_mel_bins=80,
                                   sample_frequency=16000, dither=0).unsqueeze(0)
            expected = module(features).numpy()
            actual = session.run(["embedding"], {"x": features.numpy()})[0]
            if actual.shape != (1, DIM) or expected.shape != (1, DIM) or not np.isfinite(actual).all():
                raise ValueError("forward3 shape or finite mismatch")
            np.testing.assert_allclose(actual, expected, rtol=2e-3, atol=2e-4)
            denom = np.linalg.norm(expected) * np.linalg.norm(actual)
            if not np.isfinite(denom) or denom <= 1e-8:
                raise ValueError("invalid forward3 output")
            cosine = float(np.sum(expected * actual) / denom)
            if cosine < 0.9999:
                raise ValueError("forward3 graph parity failed")
            # A flat F32LE oracle avoids any additional Rust ZIP/NPY dependency.
            name = f"oracle-{length}.f32le"
            with (args.output_dir / name).open("xb") as stream:
                for array in (waveform.numpy(), features.numpy(), expected):
                    stream.write(np.asarray(array, dtype="<f4").tobytes(order="C"))
            fixtures.append({"samples": length, "frames": int(features.shape[1]), "oracle": name,
                             "layout": "pcm, fbank, forward3", "sha256": sha256(args.output_dir / name),
                             "graph_cosine": cosine})

    receipt = {
        "schema": "translateit.speaker_export.v1", "source_revision": REVISION,
        "checkpoint_sha256": CHECKPOINT_SHA256, "forward": "forward3",
        "input": "x:F32[1,T,80]", "output": "embedding:F32[1,20480]",
        "onnx_sha256": sha256(output), "onnx_bytes": output.stat().st_size,
        "torch": torch.__version__, "onnx": onnx.__version__, "onnxruntime": ort.__version__,
        "fixtures": fixtures, "rust_frontend_parity": "NOT_EXECUTED",
        "quality_acceptance": "NOT_EXECUTED", "release_authorized": False,
    }
    (args.output_dir / "export-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
