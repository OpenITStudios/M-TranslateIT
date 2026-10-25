from __future__ import annotations

import hashlib
import io
import tarfile
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
ROOT = REPO_ROOT / "EngineData" / "Backend" / "LocalWorker" / "PythonRuntime"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def download(url: str) -> bytes:
    print(f"[release-stage][license] download {url}", flush=True)
    with urllib.request.urlopen(url, timeout=120) as response:
        return response.read()


def dist_info(name: str) -> Path:
    path = ROOT / name
    if not path.is_dir():
        raise SystemExit(f"missing reviewed distribution: {name}")
    out = path / "licenses"
    out.mkdir(parents=True, exist_ok=True)
    return out


def write_material(root: Path, name: str, data: bytes, expected: str) -> None:
    actual = sha256(data)
    if actual != expected:
        raise SystemExit(f"license hash mismatch {root.parent.name}/{name}: {actual}")
    (root / name).write_bytes(data)
    print(f"[release-stage][license] {root.parent.name}/{name} sha256={actual}", flush=True)


def write_source(root: Path, lines: list[str]) -> None:
    (root / "TRANSLATEIT_SOURCE.txt").write_text(
        "\n".join(lines) + "\n", encoding="utf-8", newline="\n"
    )


def raw_package(
    dist: str,
    source_url: str,
    material_name: str,
    expected: str,
    source_lines: list[str],
) -> None:
    root = dist_info(dist)
    write_material(root, material_name, download(source_url), expected)
    write_source(root, source_lines)


def verify_sdist(url: str, expected: str) -> bytes:
    data = download(url)
    actual = sha256(data)
    if actual != expected:
        raise SystemExit(f"sdist hash mismatch: {actual} != {expected}")
    print(f"[release-stage][license] sdist sha256={actual}", flush=True)
    return data


def sdist_package(
    dist: str,
    url: str,
    archive_hash: str,
    material: dict[str, tuple[str, str]],
    source_lines: list[str],
) -> None:
    data = verify_sdist(url, archive_hash)
    root = dist_info(dist)
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for output_name, (source_path, expected_hash) in material.items():
            member = archive.extractfile(source_path)
            if member is None:
                raise SystemExit(f"sdist license source missing: {source_path}")
            write_material(root, output_name, member.read(), expected_hash)
    write_source(root, source_lines)


raw_package(
    "ctranslate2-4.8.3.dist-info",
    "https://raw.githubusercontent.com/OpenNMT/CTranslate2/8d91aa08225004755360daa43deb62b35b1ed65d/LICENSE",
    "LICENSE",
    "54aa79d9fe3c09e67a16dcd95b9e88676405a6ec174efda31036983cf7672ecb",
    [
        "package=ctranslate2==4.8.3",
        "source_repo=OpenNMT/CTranslate2",
        "source_ref=v4.8.3",
        "source_commit=8d91aa08225004755360daa43deb62b35b1ed65d",
    ],
)

raw_package(
    "flatbuffers-25.12.19.dist-info",
    "https://raw.githubusercontent.com/google/flatbuffers/7e163021e59cca4f8e1e35a7c828b5c6b7915953/LICENSE",
    "LICENSE",
    "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30",
    [
        "package=flatbuffers==25.12.19",
        "source_repo=google/flatbuffers",
        "source_ref=v25.12.19",
        "source_commit=7e163021e59cca4f8e1e35a7c828b5c6b7915953",
    ],
)

jieba_sdist = "https://files.pythonhosted.org/packages/c6/cb/18eeb235f833b726522d7ebed54f2278ce28ba9438e3135ab0278d9792a2/jieba-0.42.1.tar.gz"
verify_sdist(
    jieba_sdist,
    "055ca12f62674fafed09427f176506079bc135638a14e23e25be909131928db2",
)
raw_package(
    "jieba-0.42.1.dist-info",
    "https://raw.githubusercontent.com/fxsjy/jieba/1e20c89b66f56c9301b0feed211733ffaa1bd72a/LICENSE",
    "LICENSE",
    "18ba0984839f85853b29fadaf992f7dba8fd0ca0fbeae34de2b8735222dc7a37",
    [
        "package=jieba==0.42.1",
        "sdist_sha256=055ca12f62674fafed09427f176506079bc135638a14e23e25be909131928db2",
        "source_repo=fxsjy/jieba",
        "source_ref=v0.42.1",
        "source_commit=1e20c89b66f56c9301b0feed211733ffaa1bd72a",
    ],
)

jieba_fast_sdist = "https://files.pythonhosted.org/packages/87/6f/9c22f7b0ecc043f8e7d324e30767fdc9ce8d3cf5fd66e60823dd2b84432e/jieba_fast-0.53.tar.gz"
verify_sdist(
    jieba_fast_sdist,
    "e92089d52faa91d51b6a7c1e6e4c4c85064a0e36f6a29257af2254b9e558ddd0",
)
raw_package(
    "jieba_fast-0.53.dist-info",
    "https://raw.githubusercontent.com/deepcs233/jieba_fast/5e6b21dece184e1004a35bfd802c8772059de3ab/LICENSE",
    "LICENSE",
    "18ba0984839f85853b29fadaf992f7dba8fd0ca0fbeae34de2b8735222dc7a37",
    [
        "package=jieba-fast==0.53",
        "sdist_sha256=e92089d52faa91d51b6a7c1e6e4c4c85064a0e36f6a29257af2254b9e558ddd0",
        "source_repo=deepcs233/jieba_fast",
        "source_commit=5e6b21dece184e1004a35bfd802c8772059de3ab",
        "upstream_release_tag=none",
    ],
)

loguru_sdist = "https://files.pythonhosted.org/packages/3a/05/a1dae3dffd1116099471c643b8924f5aa6524411dc6c63fdae648c4f1aca/loguru-0.7.3.tar.gz"
verify_sdist(
    loguru_sdist,
    "19480589e77d47b8d85b2c827ad95d49bf31b0dcde16593892eb51dd18706eb6",
)
raw_package(
    "loguru-0.7.3.dist-info",
    "https://raw.githubusercontent.com/Delgan/loguru/ae3bfd1b85b6b4a3db535f69b975687c79498be4/LICENSE",
    "LICENSE",
    "b35d026cc7aca9d5859a02eb87ddf7a386a24c986838651bd1f283f94e003327",
    [
        "package=loguru==0.7.3",
        "sdist_sha256=19480589e77d47b8d85b2c827ad95d49bf31b0dcde16593892eb51dd18706eb6",
        "source_repo=Delgan/loguru",
        "source_ref=0.7.3",
        "source_commit=ae3bfd1b85b6b4a3db535f69b975687c79498be4",
    ],
)

onnx_root = dist_info("onnxruntime-1.31.0.dist-info")
write_material(
    onnx_root,
    "LICENSE",
    download(
        "https://raw.githubusercontent.com/microsoft/onnxruntime/402979ac1bef89dcd55add41eaa7277c407599eb/LICENSE"
    ),
    "2f07c72751aed99790b8a4869cf2311df85a860b22ded05fa22803587a48922c",
)
write_material(
    onnx_root,
    "ThirdPartyNotices.txt",
    download(
        "https://raw.githubusercontent.com/microsoft/onnxruntime/402979ac1bef89dcd55add41eaa7277c407599eb/ThirdPartyNotices.txt"
    ),
    "143764b952fdb1a7c69ce653bfba74a7744d6a8a573bfb73e235fba356c83de3",
)
write_source(
    onnx_root,
    [
        "package=onnxruntime==1.31.0",
        "source_repo=microsoft/onnxruntime",
        "source_ref=v1.31.0",
        "source_commit=402979ac1bef89dcd55add41eaa7277c407599eb",
    ],
)

sdist_package(
    "sentencepiece-0.2.2.dist-info",
    "https://files.pythonhosted.org/packages/cc/33/ea3cb3839607eb175da835244a798f797f478c5ddf0e8ecdf57ea85a4c70/sentencepiece-0.2.2.tar.gz",
    "3d2b5e824b5622038dc7b490897efe05ebbbb9e7350fc142f3ecc8789ef9bdf6",
    {
        "LICENSE": (
            "sentencepiece-0.2.2/sentencepiece/LICENSE",
            "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30",
        ),
        "ABSEIL_LICENSE": (
            "sentencepiece-0.2.2/sentencepiece/third_party/abseil-cpp/LICENSE",
            "c79a7fea0e3cac04cd43f20e7b648e5a0ff8fa5344e644b0ee09ca1162b62747",
        ),
        "DARTS_CLONE_LICENSE": (
            "sentencepiece-0.2.2/sentencepiece/third_party/darts_clone/LICENSE",
            "155f59997298ee336602c49f9c1110f268ac394ca2197eb02647a3555935ad52",
        ),
        "ESAXX_LICENSE": (
            "sentencepiece-0.2.2/sentencepiece/third_party/esaxx/LICENSE",
            "7c28553d1d3312d65fe309f76a22ebaf33a3d76c8c1e3b98a88ee4654ecb53db",
        ),
        "PROTOBUF_LITE_LICENSE": (
            "sentencepiece-0.2.2/sentencepiece/third_party/protobuf-lite/LICENSE",
            "6e5e117324afd944dcf67f36cf329843bc1a92229a8cd9bb573d7a83130fea7d",
        ),
    },
    [
        "package=sentencepiece==0.2.2",
        "sdist_sha256=3d2b5e824b5622038dc7b490897efe05ebbbb9e7350fc142f3ecc8789ef9bdf6",
    ],
)

raw_package(
    "tensorboard_data_server-0.7.2.dist-info",
    "https://raw.githubusercontent.com/tensorflow/tensorboard/81150b898a306b89cde90e949358c2eefe018eaa/LICENSE",
    "LICENSE",
    "d7c9068d896188264b60827b8cd1e25fdb9f5b5cad0b1589e90b96c87729c404",
    [
        "package=tensorboard-data-server==0.7.2",
        "source_repo=tensorflow/tensorboard",
        "source_commit=81150b898a306b89cde90e949358c2eefe018eaa",
        "source_commit_message=tensorboard-data-server 0.7.2",
    ],
)

sdist_package(
    "tokenizers-0.22.2.dist-info",
    "https://files.pythonhosted.org/packages/73/6f/f80cfef4a312e1fb34baf7d85c72d4411afde10978d4657f8cdd811d3ccc/tokenizers-0.22.2.tar.gz",
    "473b83b915e547aa366d1eee11806deaf419e17be16310ac0a14077f1e28f917",
    {
        "LICENSE": (
            "tokenizers-0.22.2/tokenizers/LICENSE",
            "c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4",
        )
    },
    [
        "package=tokenizers==0.22.2",
        "sdist_sha256=473b83b915e547aa366d1eee11806deaf419e17be16310ac0a14077f1e28f917",
    ],
)

sdist_package(
    "wordsegment-1.3.1.dist-info",
    "https://files.pythonhosted.org/packages/64/68/08112f4c2888f41520e54e2d0b22dcec5adb28cddf4eeca344eb9da04177/wordsegment-1.3.1.tar.gz",
    "3dcc7cd1e9bba3f3ffe6a0e54d98377bc502fc34e9e9d8c8199ac5636924f023",
    {
        "LICENSE": (
            "wordsegment-1.3.1/LICENSE",
            "8fe4d37c518608a57c6b0f24e26915144f8daf460eb4e1572146596ec3673294",
        )
    },
    [
        "package=wordsegment==1.3.1",
        "sdist_sha256=3dcc7cd1e9bba3f3ffe6a0e54d98377bc502fc34e9e9d8c8199ac5636924f023",
    ],
)

print("[release-stage][license] reviewed exceptional Python license materials -> PASS")
