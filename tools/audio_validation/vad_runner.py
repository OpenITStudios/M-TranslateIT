"""Manually execute the ignored, canonical Rust VAD/WAV fixture replay.

No CI, microphone, model inference or production Meeting input substitution.
A successful cargo exit only proves the test executed: per-case stage failures
are preserved for the existing harness scorer.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from contextlib import nullcontext
from pathlib import Path

from harness import (
    RATE,
    RESULT_SCHEMA,
    ROOT,
    digest,
    fingerprint,
    inspect,
    metrics,
    specs,
)
from worker_runner import pinned_source

PREFIX = "AUDIO_VAD_RECEIPT_JSON="
RECEIPT_SCHEMA = "translateit.audio_vad_receipt.v1"
CLAIM = "CANONICAL_INCOMING_VAD_AND_PCM16_WRITER_OFFLINE_ONLY"
CARGO_ROOT = ROOT / "EngineData/Frontend/RustApp/src-tauri"


def parse_receipts(output: str, manifest: dict, scenario_hash: str) -> dict[str, dict]:
    expected = {r["id"]: r["sha256"] for r in manifest["fixtures"]}
    observed: dict[str, dict] = {}
    for line in output.splitlines():
        if PREFIX not in line:
            continue
        start = line.index(PREFIX) + len(PREFIX)
        payload = json.loads(line[start:].strip())
        if not isinstance(payload, dict):
            raise TypeError("Rust replay receipt must be an object")
        case = payload.get("scenario_id")
        if (case not in expected or case in observed
                or payload.get("schema") != RECEIPT_SCHEMA
                or payload.get("audio_fingerprint") != manifest["audio_fingerprint"]
                or payload.get("scenario_fingerprint") != scenario_hash
                or payload.get("audio_sha256") != expected[case]):
            raise ValueError("unbound/duplicate Rust fixture receipt")
        stage = payload.get("stage")
        if (not isinstance(stage, dict)
                or type(stage.get("ok")) is not bool
                or type(stage.get("finalized_count")) is not int
                or type(stage.get("overflow_count")) is not int
                or type(stage.get("writer_ok")) is not bool
                or stage.get("test_only") is not True
                or stage.get("claim") != CLAIM):
            raise ValueError(f"{case}: invalid Rust VAD proof contract")
        observed[case] = stage
    if observed.keys() != expected.keys():
        raise ValueError(f"incomplete Rust replay receipts: {sorted(expected.keys() - observed.keys())}")
    return observed


def make_observations(
    source_sha: str, manifest: dict, scenarios: list[dict], scenario_hash: str,
    observed: dict[str, dict], prior: dict | None,
) -> dict:
    prior_by_id: dict[str, dict] = {}
    identity = {"local_sha": source_sha, "context": "LOCAL_CODE_RUST_VAD_ONLY"}
    if prior is not None:
        if (prior.get("schema") != RESULT_SCHEMA
                or prior.get("scenario_fingerprint") != scenario_hash
                or prior.get("audio_fingerprint") != manifest["audio_fingerprint"]
                or not isinstance(prior.get("source_identity"), dict)
                or prior["source_identity"].get("local_sha") != source_sha):
            raise ValueError("worker observations belong to a different source/audio set")
        records = prior.get("results")
        if not isinstance(records, list) or len(records) != len(scenarios):
            raise ValueError("worker observations have incomplete case coverage")
        prior_by_id = {row["scenario_id"]: row for row in records}
        if len(prior_by_id) != len(scenarios):
            raise ValueError("worker observations contain duplicate cases")
        identity = {**prior["source_identity"], "vad_context": "LOCAL_CODE_RUST_CANONICAL_VAD"}
    hashes = {r["id"]: r["sha256"] for r in manifest["fixtures"]}
    results = []
    for scenario in scenarios:
        name = scenario["id"]
        prior_row = prior_by_id.get(name, {})
        if prior is not None and (prior_row.get("audio_sha256") != hashes[name]
                                  or not isinstance(prior_row.get("stages"), dict)
                                  or "vad" in prior_row["stages"]):
            raise ValueError(f"{name}: prior evidence missing or duplicate VAD stage")
        stages = dict(prior_row.get("stages", {}))
        stages["vad"] = observed[name]
        results.append({
            "scenario_id": name,
            "audio_sha256": hashes[name],
            "stages": stages,
        })
    return {
        "schema": RESULT_SCHEMA,
        "scenario_fingerprint": scenario_hash,
        "audio_fingerprint": manifest["audio_fingerprint"],
        "source_identity": identity,
        "results": results,
        "proof": "EXPLICIT_RUST_CANONICAL_VAD_PCM16_WRITER_OFFLINE_ONLY",
    }


def verify_segments(root: Path, observed: dict[str, dict]) -> str:
    """Verify the actual Rust writer output, never the original fixture WAV."""
    if root.is_symlink() or not root.is_dir():
        raise ValueError("explicit real segment directory required")
    expected = set()
    records = []
    for case, stage in observed.items():
        if not re.fullmatch(r"A[0-9]{2}", case):
            raise ValueError("invalid segment scenario ID")
        rows = stage.get("segments")
        attempts = stage.get("writer_attempts")
        if (type(attempts) is not int or not 0 <= attempts <= 1000
                or not isinstance(rows, list) or len(rows) != stage.get("writer_valid")):
            raise ValueError(f"{case}: complete exported writer receipts required")
        for row in rows:
            if not isinstance(row, dict):
                raise TypeError("invalid finalized segment receipt")
            name = row.get("file", "")
            # Exact ordinal names exclude paths, aliases and original WAVs.
            valid_names = {f"{case}_{i}.wav" for i in range(1, attempts + 1)}
            if name not in valid_names or name in expected:
                raise ValueError("invalid/duplicate finalized segment name")
            path = root / name
            checked = metrics(path)
            if (checked["rate_hz"] != RATE or row.get("rate_hz") != RATE
                    or checked["frames"] != row.get("frames")
                    or fingerprint(path) != row.get("sha256")):
                raise ValueError("finalized segment hash/PCM identity differs")
            expected.add(name)
            records.append({"scenario_id": case, **row})
    if {p.name for p in root.iterdir()} != expected:
        raise ValueError("segment directory has missing or untracked files")
    return digest(records)


def run(fixtures: Path, source_sha: str, output: Path, timeout: int,
        worker_observations: Path | None = None, segments_dir: Path | None = None) -> dict:
    if not 30 <= timeout <= 3600:
        raise ValueError("timeout must be 30–3600 seconds")
    pinned_source(source_sha)
    manifest = inspect(fixtures)
    scenarios, scenario_hash = specs()
    if output.exists() or output.is_symlink():
        raise ValueError("observation destination already exists; never overwrite a receipt")
    prior = json.loads(worker_observations.read_text(encoding="utf-8")) if worker_observations else None
    if prior is not None and not isinstance(prior, dict):
        raise ValueError("prior worker evidence must be an object")
    env = dict(os.environ)
    env.pop("TRANSLATEIT_AUDIO_SEGMENT_ROOT", None)
    env.update({
        "TRANSLATEIT_AUDIO_FIXTURE_ROOT": str(fixtures.resolve(strict=True)),
        "TRANSLATEIT_AUDIO_FINGERPRINT": manifest["audio_fingerprint"],
        "CARGO_NET_OFFLINE": "true",
        "CARGO_BUILD_JOBS": "2",
        "RUST_TEST_THREADS": "1",
        "HF_HUB_OFFLINE": "1",
        "TRANSFORMERS_OFFLINE": "1",
    })
    invocation = [
        "cargo", "test", "--locked", "--offline",
        "explicit_public_audio_fixture_replay_receipts", "--",
        "--ignored", "--nocapture", "--test-threads=1",
    ]
    if segments_dir is not None:
        if segments_dir.exists() or segments_dir.is_symlink():
            raise ValueError("exported segment destination already exists")
        segments_dir.parent.mkdir(parents=True, exist_ok=True)
    context = (tempfile.TemporaryDirectory(prefix="audio-vad-export-", dir=segments_dir.parent)
               if segments_dir is not None else nullcontext(None))
    with context as work:
        if work is not None:
            staged = Path(work).resolve() / "segments"
            env["TRANSLATEIT_AUDIO_SEGMENT_ROOT"] = str(staged)
        result = subprocess.run(invocation, check=False, cwd=CARGO_ROOT, env=env, shell=False,
                                capture_output=True, text=True, timeout=timeout)
        if result.returncode != 0:
            raise ValueError("Rust fixture replay failed; compile/runtime proof unavailable: "
                             + (result.stderr[-700:] or result.stdout[-700:]))
        observed = parse_receipts(result.stdout, manifest, scenario_hash)
        pinned_source(source_sha)
        document = make_observations(source_sha, manifest, scenarios, scenario_hash, observed, prior)
        if segments_dir is not None:
            document["segment_fingerprint"] = verify_segments(staged, observed)
            staged.rename(segments_dir)
    output.parent.mkdir(parents=True, exist_ok=True)
    # Create-only file mode prevents a competing run from replacing past proof.
    with output.open("x", encoding="utf-8") as stream:
        json.dump(document, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    return document


def main() -> int:
    parser = argparse.ArgumentParser(description="Explicit canonical offline Rust VAD fixture run")
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--worker-observations", type=Path)
    parser.add_argument("--segments-dir", type=Path)
    parser.add_argument("--timeout", type=int, default=900)
    args = parser.parse_args()
    result = run(args.fixtures, args.source_sha, args.output, args.timeout,
                 args.worker_observations, args.segments_dir)
    print(json.dumps({"output": str(args.output), "cases": len(result["results"]),
                      "proof": result["proof"]}, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, TypeError, OSError, json.JSONDecodeError, subprocess.TimeoutExpired) as error:
        print(f"audio-vad-runner: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from None
