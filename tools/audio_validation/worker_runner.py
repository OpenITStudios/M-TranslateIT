"""Explicit offline native stage diagnostics and legacy voice compatibility fixtures.

No GitHub CI, microphone, CPAL, Meeting session, cloud, or model downloads.
Native finalized ASR and translation use the existing Rust owner.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
import queue
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

from harness import (
    RESULT_SCHEMA,
    ROOT,
    fingerprint,
    inspect,
    metrics,
    regression_profile,
    specs,
    translation_receipt_complete,
)
from pipeline_checks import (
    finalized_asr_outcome,
    grade_finalized_asr,
    pipeline_contracts,
    score_pipeline,
    source_admission_passed,
)

WORKER = ROOT / "EngineData/Backend/LocalWorker/WorkerRuntime/realtime_local_worker.py"
MAX_FRAME = 1_000_000


class WorkerTransportError(RuntimeError):
    pass


def pinned_source(expected: str) -> None:
    if not re.fullmatch(r"[a-f0-9]{40}", expected):
        raise ValueError("external exact 40-character Local SHA required")
    for command, actual in [
        (("rev-parse", "HEAD"), expected),
        (("branch", "--show-current"), "Local"),
        (("status", "--porcelain", "--untracked-files=no"), ""),
    ]:
        result = subprocess.run(["git", "-C", str(ROOT), *command],
                                capture_output=True, text=True, check=True, timeout=10)
        if result.stdout.strip() != actual:
            raise ValueError("Local checkout branch/SHA/clean tracked status mismatch")



def validate_response_frame(frame: bytes | None, *, request_id: str, command: str) -> dict:
    """Strict, pure protocol guard for deterministic offline failure cases."""
    if not frame or len(frame) > MAX_FRAME or not frame.endswith(bytes([10])):
        raise WorkerTransportError("worker returned empty, oversized or partial response")
    try:
        response = json.loads(frame)
    except (UnicodeError, json.JSONDecodeError) as exc:
        raise WorkerTransportError("invalid worker JSON frame") from exc
    expected = "local_realtime_worker_preflight" if command == "status" else command
    if (not isinstance(response, dict) or response.get("request_id") != request_id
            or type(response.get("ok")) is not bool):
        raise WorkerTransportError("worker request identity/boolean mismatch")
    # The canonical Python request dispatcher deliberately maps raised handler
    # exceptions to stage=worker_error (ok=false). This is an owned failure,
    # not an invalid transport frame. Never treat worker_error as a success.
    if response.get("stage") == "worker_error":
        if response["ok"] or not str(response.get("blocker", "")).startswith("worker:handler_failed:"):
            raise WorkerTransportError("invalid worker handler failure envelope")
    elif response.get("stage") != expected:
        raise WorkerTransportError("worker response stage mismatch")
    return response


def await_worker_frame(messages: queue.Queue[bytes | None], timeout: float) -> bytes:
    """Shared bounded transport wait; EOF and timeout are explicit failures."""
    if not 0 < timeout <= 600:
        raise ValueError("transport wait must have a positive bounded deadline")
    try:
        frame = messages.get(timeout=timeout)
    except queue.Empty as exc:
        raise WorkerTransportError("worker:response_deadline_exceeded") from exc
    if frame is None:
        raise WorkerTransportError("worker:stdout_closed")
    return frame


class Worker:
    def __init__(self, python: Path, timeout: float):
        if not python.is_file() or not WORKER.is_file():
            raise ValueError("canonical pinned WorkerRuntime Python/entrypoint unavailable")
        if not 5 <= timeout <= 600:
            raise ValueError("worker request timeout outside 5–600 seconds")
        self.deadline = timeout
        self.count = 0
        self.messages: queue.Queue[bytes | None] = queue.Queue(maxsize=2)
        env = os.environ.copy()
        env.update({"HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1",
                    "HF_DATASETS_OFFLINE": "1", "TOKENIZERS_PARALLELISM": "false",
                    "PYTHONUTF8": "1", "PYTHONIOENCODING": "utf-8",
                    "PYTHONDONTWRITEBYTECODE": "1"})
        self.child = subprocess.Popen(
            [str(python), "-u", str(WORKER)], cwd=WORKER.parent,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            env=env, shell=False,
        )
        self.reader = threading.Thread(target=self._read, daemon=True,
                                       name="audio-fixture-worker-reader")
        self.reader.start()

    def _read(self) -> None:
        assert self.child.stdout is not None
        while True:
            raw = self.child.stdout.readline(MAX_FRAME + 1)
            self.messages.put(raw if raw else None)
            if not raw or len(raw) > MAX_FRAME:
                return

    def invoke(self, command: str, **fields: object) -> dict:
        self.count += 1
        request_id = f"fixture-{self.count}"
        if self.child.poll() is not None:
            raise WorkerTransportError("worker exited before command")
        assert self.child.stdin is not None
        body = {"command": command, "request_id": request_id, **fields}
        try:
            self.child.stdin.write((json.dumps(body, ensure_ascii=False) + "\n").encode("utf-8"))
            self.child.stdin.flush()
            frame = await_worker_frame(self.messages, self.deadline)
        except WorkerTransportError:
            self.close()
            raise
        except (BrokenPipeError, OSError) as exc:
            self.close()
            raise WorkerTransportError("persistent worker write failed") from exc
        try:
            return validate_response_frame(frame, request_id=request_id, command=command)
        except WorkerTransportError:
            self.close()
            raise

    def close(self) -> None:
        if self.child.poll() is None:
            self.child.terminate()
            try:
                self.child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.child.kill()
                self.child.wait(timeout=5)
        for stream in (self.child.stdin, self.child.stdout):
            if stream is not None:
                stream.close()
        self.reader.join(timeout=2)


def stage_result(response: dict, name: str) -> dict:
    result = {"ok": response["ok"], "blocker": str(response.get("blocker", ""))[:160],
              "elapsed_ms": response.get("elapsed_ms")}
    if name == "asr":
        if not isinstance(response.get("transcript_text", ""), str):
            raise WorkerTransportError("worker:non_text_asr_result")
        result["transcript_text"] = response.get("transcript_text", "")
        result["note"] = str(response.get("note", ""))[:1000]
        result.update({key: response.get(key) for key in ("model_id", "device", "compute_type")})
    elif name == "translation":
        if not isinstance(response.get("translated_text", ""), str):
            raise WorkerTransportError("worker:non_text_translation_result")
        result.update({
            "model_id": response.get("model_id"),
            "model_revision": response.get("model_revision"),
            "device": response.get("device"),
            "direction_pair": response.get("direction_pair"),
            "complete": response.get("complete") is True,
            "finished_with_eos": response.get("finished_with_eos") is True,
            "translated_text": response.get("translated_text", ""),
        })
    return result


def run_finalized(fixtures: Path, python: Path | None, expected_sha: str, selection: set[str],
                  result_path: Path, timeout: float, vad_observations: Path,
                  segments_dir: Path, *, pipeline: bool = False, rounds: int = 1,
                  vad_source_sha: str | None = None, profile: str | None = None,
                  source_checks: dict | None = None, artifacts_dir: Path | None = None) -> dict:
    """Diagnostic ASR on actual exported Rust segments, not app acceptance."""
    from vad_runner import CLAIM, verify_segments

    pinned_source(expected_sha)
    if type(rounds) is not int or not 1 <= rounds <= 20 or (not pipeline and (rounds != 1 or vad_source_sha is not None)):
        raise ValueError("repeated/retained inputs require finalized pipeline; rounds must be 1–20")
    input_sha = vad_source_sha or expected_sha
    if profile and input_sha != expected_sha:
        raise ValueError("named regression profiles require fresh same-source VAD; use custom IDs for retained replay")
    if not re.fullmatch(r"[a-f0-9]{40}", input_sha):
        raise ValueError("exact finalized VAD source SHA required")
    manifest = inspect(fixtures)
    suite, suite_id = specs()
    cases = {r["id"]: r for r in suite}
    if not selection or not selection.issubset(cases):
        raise ValueError("select known finalized scenario IDs")
    execution_profile = regression_profile(profile) if profile else None
    if execution_profile and (not pipeline or selection != set(execution_profile["scenario_ids"])
            or rounds != execution_profile["rounds"] or not source_admission_passed(source_checks, expected_sha)):
        raise ValueError("named profile requires its complete coverage and matching source admission")
    if result_path.exists() or result_path.is_symlink():
        raise ValueError("finalized ASR result already exists")
    vad_receipt_hash = fingerprint(vad_observations)
    prior = json.loads(vad_observations.read_text(encoding="utf-8"))
    if (not isinstance(prior, dict) or not isinstance(prior.get("source_identity"), dict)
            or prior.get("schema") != RESULT_SCHEMA or prior.get("scenario_fingerprint") != suite_id
            or prior.get("audio_fingerprint") != manifest["audio_fingerprint"]
            or prior.get("source_identity", {}).get("local_sha") != input_sha):
        raise ValueError("VAD source/audio/scenario identity differs")
    rows = prior.get("results", [])
    if not isinstance(rows, list) or len(rows) != len(cases):
        raise ValueError("incomplete finalized VAD observations")
    expected_hashes = {r["id"]: r["sha256"] for r in manifest["fixtures"]}
    observed = {}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("stages"), dict):
            raise TypeError("invalid finalized VAD observation")
        case = row.get("scenario_id")
        stage = row.get("stages", {}).get("vad", {})
        if (not isinstance(case, str) or not isinstance(stage, dict)
                or case not in cases or case in observed
                or row.get("audio_sha256") != expected_hashes[case]
                or stage.get("claim") != CLAIM or stage.get("test_only") is not True):
            raise ValueError("invalid/unbound finalized VAD observation")
        observed[case] = stage
    if observed.keys() != cases.keys():
        raise ValueError("incomplete finalized VAD observations")
    segment_id = verify_segments(segments_dir, observed)
    if prior.get("segment_fingerprint") != segment_id:
        raise ValueError("exported segment fingerprint differs")
    report = {"schema": "translateit.finalized_asr_probe.v1",
              "source_identity": {"local_sha": expected_sha, "context": "LOCAL_CODE_RUST_VAD_TO_ASR"},
              "scenario_fingerprint": suite_id, "audio_fingerprint": manifest["audio_fingerprint"],
              "segment_fingerprint": segment_id, "results": [],
              "downstream": "TRANSLATION_TTS_MEETING_NOT_EXECUTED",
              "proof": "FINALIZED_ASR_PROBE_NOT_EXECUTED"}
    if not pipeline:
        if python is not None:
            raise ValueError("native finalized ASR does not accept --python")
        report = collect_native_finalized_asr(report, cases, selection, observed, segments_dir, timeout)
        if (verify_segments(segments_dir, observed) != segment_id
                or fingerprint(vad_observations) != vad_receipt_hash):
            raise ValueError("finalized input evidence changed during ASR")
        pinned_source(expected_sha)
        report.update(grade_finalized_asr(report, cases, selection))
        if report.get("transport_error"):
            report["verdict"] = "FAIL"
        publish_report(result_path, report)
        return report
    if python is None:
        raise ValueError("legacy voice pipeline requires --python")
    if pipeline:
        contracts, _evaluator, contract_id = pipeline_contracts(cases)
        if execution_profile and any(cases[name]["kind"] == "speech" and name not in contracts for name in selection):
            raise ValueError("named regression profile lacks a speech semantic contract")
        report["source_identity"]["context"] = "LOCAL_CODE_RETAINED_FINALIZED_WAV_TO_WORKER_PIPELINE"
        report.update(schema="translateit.finalized_pipeline.v2", rounds=rounds,
                      selection=sorted(selection), execution_profile=execution_profile,
                      source_checks=source_checks,
                      expected_segments={name: observed[name]["segments"] for name in selection},
                      current_vad_required=bool(execution_profile),
                      vad_stage_results={name: {key: observed[name].get(key) for key in
                          ("ok", "finalized_count", "writer_valid", "overflow_count")} for name in selection},
                      input_source_identity=prior["source_identity"],
                      vad_receipt_sha256=vad_receipt_hash,
                      translation_contract_fingerprint=contract_id,
                      acceptance_scope="DECLARED_PUBLIC_FIXTURE_CRITICALS_AND_SPOKEN_CONTENT_PROXY_NOT_GENERAL_SEMANTIC_OR_NATURALNESS_ACCEPTANCE",
                      downstream="TRANSLATION_SELECTED_ENGLISH_VOICE_CONTENT_PROXY_NOT_MEETING",
                      proof="CURRENT_WORKER_ON_HASH_VERIFIED_FINALIZED_INPUTS_NOT_FRESH_VAD_OR_MEETING")
        if execution_profile:
            report["proof"] = "FRESH_CANONICAL_VAD_AND_WORKER_PIPELINE_NOT_NATIVE_MEETING"
            report["source_identity"]["context"] = "LOCAL_CODE_CURRENT_VAD_TO_WORKER_PIPELINE"
    # The canonical worker accepts CacheData/LogData inputs only. Stage a verified
    # byte-identical copy there; do not widen its allowed paths for test tooling.
    cache = ROOT / "UserData/CacheData"
    cache.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="finalized-asr-probe-", dir=cache) as work:
        worker = Worker(python, timeout)
        try:
            report["worker_status"] = stage_result(worker.invoke("status"), "status")
            report["asr_preload"] = worker.invoke("asr_preload")
            if pipeline:
                report["worker_pid"] = worker.child.pid
                report["translation_preload"] = worker.invoke("translation_preload", source_language="id", target_language="en")
                report["voice_preflight"] = (worker.invoke("voice_actor_preflight")
                    if any(cases[n]["kind"] == "speech" and cases[n]["language"] == "id" for n in selection) else {})
                report["snapshots"] = []
                report["preload_blockers"] = [key for key in ("asr_preload", "translation_preload", "voice_preflight")
                    if report[key] and report[key].get("ok") is not True]
            # Full outbound readiness also requires a selected voice. This
            # diagnostic admits only the independently executed ASR preload.
            if report["asr_preload"].get("ok") is True and not report.get("preload_blockers"):
                ordered = [(iteration, case) for iteration in range(1, rounds + 1)
                           for case in (suite if iteration % 2 else list(reversed(suite)))]
                for iteration, case in ordered:
                    name = case["id"]
                    if name not in selection:
                        continue
                    segments = observed[name]["segments"]
                    if not segments:
                        vad = observed[name]
                        rejected = (case["kind"] != "speech" and vad.get("ok") is True
                                    and vad.get("finalized_count") == 0 and vad.get("overflow_count") == 0)
                        report["results"].append({"scenario_id": name, "round": iteration,
                            "outcome": "VAD_REJECTED_NON_SPEECH" if rejected else "NO_VAD_SEGMENT",
                            "vad": {k: vad.get(k) for k in ("ok", "finalized_count", "overflow_count")},
                            "asr_executed": False})
                    for segment in segments:
                        source = segments_dir / segment["file"]
                        audio = Path(work) / segment["file"]
                        shutil.copyfile(source, audio)
                        if fingerprint(audio) != segment["sha256"]:
                            raise ValueError("staged finalized segment changed identity")
                        response = worker.invoke("transcribe", audio_path=str(audio),
                                                 language=case["language"], beam_size=1,
                                                 vad_filter=False)
                        if fingerprint(audio) != segment["sha256"]:
                            raise ValueError("ASR input changed while consumed")
                        row = {
                            "scenario_id": name, "round": iteration, "segment": segment,
                            "input_sha256": fingerprint(audio),
                            "asr": stage_result(response, "asr"),
                            "asr_runtime": {k: response.get(k) for k in ("model_id", "device", "compute_type")},
                            "outcome": finalized_asr_outcome(response, non_speech=case["kind"] != "speech"),
                        }
                        report["results"].append(row)
                        if pipeline and response["ok"] and case["kind"] == "speech":
                            target = "en" if case["language"] == "id" else "id"
                            trans = worker.invoke("translate", text=response["transcript_text"],
                                source_language=case["language"], target_language=target)
                            row["translation"] = stage_result(trans, "translation")
                            if translation_receipt_complete(row["translation"], f"{case['language']}->{target}") and target == "en":
                                preflight = report["voice_preflight"]
                                if not preflight.get("ok"):
                                    row["tts"] = {"ok": False, "blocker": "voice_actor:preflight_unavailable"}
                                    continue
                                output = Path(work) / f"round{iteration}-{name}-{segment['file']}-tts.wav"
                                produced = worker.invoke("voice_actor_synthesize", text=trans["translated_text"],
                                    output_path=str(output), expected_actor_token=preflight.get("actor_token", ""))
                                tts = stage_result(produced, "tts")
                                tts.update(actor_token=produced.get("actor_token"), output_path=produced.get("output_path"),
                                           expected_output_path=str(output))
                                row["tts"] = tts
                                tts["actor_identity_match"] = bool(preflight.get("actor_token")) and produced.get("actor_token") == preflight["actor_token"]
                                tts["output_identity_match"] = produced.get("output_path") == str(output)
                                if produced["ok"] and tts["actor_identity_match"] and tts["output_identity_match"] and output.is_file():
                                    measure = metrics(output)
                                    tts.update(measure, wav_sha256=fingerprint(output))
                                    row["roundtrip_asr"] = stage_result(worker.invoke("transcribe", audio_path=str(output),
                                        language="en", beam_size=1, vad_filter=False), "asr")
                                    if fingerprint(output) != tts["wav_sha256"]:
                                        raise ValueError("synthesized output changed during content check")
                    last = next(c["id"] for c in (reversed(suite) if iteration % 2 else suite) if c["id"] in selection)
                    if pipeline and name == last:
                        report["snapshots"].append({"round": iteration, "scenario_id": name,
                                                    "status": worker.invoke("status")})
                        print(f"audio-regression: round {iteration}/{rounds} completed", file=sys.stderr, flush=True)
        except WorkerTransportError as exc:
            report["transport_error"] = str(exc)
        finally:
            worker.close()
            if pipeline:
                report["fixture_worker_terminated"] = worker.child.poll() is not None
        if pipeline:
            report.update(score_pipeline(report, cases, suite_id, manifest["audio_fingerprint"]))
            failed = {(c["scenario_id"], c["round"]) for c in report["checks"] if c["status"] != "PASS"}
            for row in report["results"]:
                if (row["scenario_id"], row["round"]) not in failed or not row.get("tts", {}).get("wav_sha256"):
                    continue
                tts = row["tts"]
                original = Path(tts["expected_output_path"])
                if not original.resolve().is_relative_to(Path(work).resolve()) or fingerprint(original) != tts["wav_sha256"]:
                    raise ValueError("failed output artifact identity differs")
                retained = (artifacts_dir or result_path.with_suffix(result_path.suffix + ".artifacts")) / "failed_audio" / original.name
                retained.parent.mkdir(parents=True, exist_ok=True)
                with original.open("rb") as source, retained.open("xb") as target:
                    shutil.copyfileobj(source, target)
                if fingerprint(retained) != tts["wav_sha256"]:
                    raise ValueError("retained failure audio identity differs")
                tts["retained_output_path"] = str(retained)
    # Original exports remain available for reproducibility after worker cleanup.
    if verify_segments(segments_dir, observed) != segment_id:
        raise ValueError("exported finalized segments changed during ASR")
    if fingerprint(vad_observations) != vad_receipt_hash:
        raise ValueError("finalized VAD receipt changed during execution")
    pinned_source(expected_sha)
    publish_report(result_path, report)
    return report


NATIVE_RAW_AUDIO_PROOF = "NATIVE_OWNER_RAW_ASR_MT_ONLY_NOT_MEETING_ACCEPTANCE"


def run_native_raw_audio(fixtures: Path, expected_sha: str, selection: set[str],
                         result_path: Path, timeout: float) -> dict:
    pinned_source(expected_sha)
    if not 5 <= timeout <= 600:
        raise ValueError("native request timeout outside 5–600 seconds")
    if result_path.exists() or result_path.is_symlink():
        raise ValueError("observation file must be new, never overwrite")
    manifest = inspect(fixtures)
    suite, suite_id = specs()
    if not selection or not selection.issubset({case["id"] for case in suite}):
        raise ValueError("choose at least one known scenario ID")
    fixture_rows = {row["id"]: row for row in manifest["fixtures"]}
    requests, records = [], {}
    for case in suite:
        ident = case["id"]
        fixture = fixture_rows[ident]
        record = {"scenario_id": ident, "audio_sha256": fixture["sha256"], "stages": {}}
        records[ident] = record
        if ident not in selection:
            record["not_executed"] = "not_selected"
            continue
        measure = fixture["metrics"]
        if measure["rate_hz"] != 16000 or not 4800 <= measure["frames"] <= 960000:
            record["not_executed"] = "outside_native_asr_pcm_contract"
            continue
        requests.append({"case_id": ident,
            "audio_path": str((fixtures / "samples" / f"{ident}.wav").resolve(strict=True)),
            "audio_sha256": fixture["sha256"], "frames": measure["frames"],
            "language": case["language"], "translate": case["kind"] == "speech"})
    identity = {"local_sha": expected_sha, "context": "LOCAL_CODE_NATIVE_RAW_ASR_MT"}
    report = {"schema": RESULT_SCHEMA, "scenario_fingerprint": suite_id,
              "audio_fingerprint": manifest["audio_fingerprint"], "source_identity": identity,
              "selection": sorted(selection), "results": list(records.values()),
              "proof": "NATIVE_RAW_AUDIO_NOT_EXECUTED", "execution_complete": False,
              "transport_incomplete": False, "noise_suppression_enabled": False}
    if requests:
        bundle = {"schema": "translateit.native_audio.requests.v1",
                  "source_sha": expected_sha, "audio_fingerprint": manifest["audio_fingerprint"],
                  "timeout_ms": round(timeout * 1000), "requests": requests}
        try:
            receipt, rows = collect_native_audio_rows(bundle, NATIVE_RAW_AUDIO_PROOF,
                "audio_fingerprint", "translateit.native_audio.replay.v1")
            for row in rows:
                record = records[row["case_id"]]
                record["stages"]["asr"] = stage_result(row["response"], "asr")
                if "translation" in row:
                    record["stages"]["translation"] = stage_result(row["translation"], "translation")
                record["native_receipt"] = row
            report.update({k: receipt[k] for k in ("request_sha256", "dll_pins_sha256",
                                                  "cuda_pci_id", "worker_pid", "worker_terminated")})
            identity.update(asr_model=NATIVE_ASR_MODEL, translation_model=NATIVE_MT_MODEL, device="cuda")
            report["proof"] = NATIVE_RAW_AUDIO_PROOF
            report["execution_complete"] = len(requests) == len(selection)
        except (OSError, ValueError, WorkerTransportError, subprocess.TimeoutExpired) as error:
            report.update(transport_error=str(error), transport_incomplete=True,
                          proof="NATIVE_RAW_AUDIO_REPLAY_INCOMPLETE")
            for item in requests:
                records[item["case_id"]]["not_executed"] = "native_receipt_unverified"
    for ident in selection:
        if fingerprint(fixtures / "samples" / f"{ident}.wav") != fixture_rows[ident]["sha256"]:
            raise ValueError("raw fixture changed during replay")
    pinned_source(expected_sha)
    publish_report(result_path, report)
    return report


def run(fixtures: Path, python: Path | None, expected_sha: str,
        selection: set[str], result_path: Path, voice: bool, timeout: float) -> dict:
    if not voice:
        if python is not None:
            raise ValueError("native raw audio accepts no --python")
        return run_native_raw_audio(fixtures, expected_sha, selection, result_path, timeout)
    if python is None:
        raise ValueError("legacy selected-voice diagnostic requires --python")
    pinned_source(expected_sha)
    manifest = inspect(fixtures)
    suite, suite_id = specs()
    available = {x["id"] for x in suite}
    if not selection or not selection.issubset(available):
        raise ValueError("choose at least one known scenario ID")
    if result_path.exists() or result_path.is_symlink():
        raise ValueError("observation file must be new, never overwrite")
    fixture_rows = {x["id"]: x for x in manifest["fixtures"]}
    scratch_root = ROOT / "UserData/CacheData"
    scratch_root.mkdir(parents=True, exist_ok=True)
    records: list[dict] = []
    status: dict = {}
    stopped = False
    with tempfile.TemporaryDirectory(prefix="audio-fixture-run-", dir=scratch_root) as tmp:
        working = Path(tmp)
        worker = Worker(python, timeout)
        try:
            status = worker.invoke("status")
            asr_preload = worker.invoke("asr_preload")
            for case in suite:
                ident = case["id"]
                record = {"scenario_id": ident,
                          "audio_sha256": fixture_rows[ident]["sha256"], "stages": {}}
                records.append(record)
                if ident not in selection:
                    record["not_executed"] = "not_selected"
                    continue
                if not asr_preload.get("ok"):
                    record["not_executed"] = "asr_preload_failed"
                    continue
                audio = working / f"{ident}.wav"
                shutil.copyfile(fixtures/"samples"/f"{ident}.wav", audio)
                if fingerprint(audio) != record["audio_sha256"]:
                    raise ValueError("staged audio changed identity")
                started = time.monotonic()
                try:
                    asr = worker.invoke("transcribe", audio_path=str(audio),
                                        language=case["language"], beam_size=1, vad_filter=False)
                    record["stages"]["asr"] = stage_result(asr, "asr")
                    record["stages"]["asr"]["host_elapsed_ms"] = round((time.monotonic()-started)*1000)
                    if asr["ok"] and case["kind"] == "speech":
                        translated = worker.invoke(
                            "translate", text=str(asr.get("transcript_text", "")),
                            source_language=case["language"],
                            target_language="en" if case["language"] == "id" else "id",
                        )
                        record["stages"]["translation"] = stage_result(translated, "translation")
                        if voice and case["language"] == "id" and translated["ok"]:
                            preflight = worker.invoke("voice_actor_preflight")
                            record["stages"]["voice_preflight"] = stage_result(preflight, "voice")
                            if not preflight["ok"]:
                                record["stages"]["tts"] = {
                                    "ok": False, "blocker": "voice_actor:preflight_unavailable",
                                }
                            else:
                                wav_out = working/f"{ident}-tts.wav"
                                produced = worker.invoke(
                                    "voice_actor_synthesize",
                                    text=str(translated.get("translated_text", "")),
                                    output_path=str(wav_out),
                                    expected_actor_token=preflight.get("actor_token", ""),
                                )
                                result = stage_result(produced, "tts")
                                # Synthesis must identify the exact actor and intended
                                # output, not an unrelated or stale WAV on the host.
                                actor_ok = (isinstance(preflight.get("actor_token"), str)
                                            and bool(preflight["actor_token"])
                                            and produced.get("actor_token") == preflight["actor_token"])
                                path_ok = (produced.get("output_path") == str(wav_out))
                                if produced["ok"] and actor_ok and path_ok and wav_out.is_file():
                                    measure = metrics(wav_out)
                                    result.update({
                                        "wav_sha256": fingerprint(wav_out),
                                        "duration_ms": measure["duration_ms"],
                                        "rms": measure["rms"],
                                        "peak": measure["peak"],
                                        "rate_hz": measure["rate_hz"],
                                    })
                                    if measure["rms"] <= 0.0005:
                                        result.update({"ok": False, "blocker": "voice_actor:silent_audio"})
                                elif produced["ok"]:
                                    result.update({"ok": False, "blocker": "voice_actor:output_identity_invalid"})
                                record["stages"]["tts"] = result
                except (WorkerTransportError, OSError) as exc:
                    record["transport_error"] = type(exc).__name__
                    stopped = True
                    break
        finally:
            worker.close()
    processed = {r["scenario_id"] for r in records}
    for case in suite:
        if case["id"] not in processed:
            records.append({"scenario_id": case["id"],
                            "audio_sha256": fixture_rows[case["id"]]["sha256"],
                            "stages": {}, "not_executed": "worker_transport_failed"})
    output = {
        "schema": RESULT_SCHEMA, "scenario_fingerprint": suite_id,
        "audio_fingerprint": manifest["audio_fingerprint"],
        "source_identity": {
            "local_sha": expected_sha, "context": "LOCAL_CODE_WORKER_ONLY",
            "worker_status_ok": status.get("ok") is True,
            "asr_model": status.get("asr_active_model_id", "unverified"),
            "translation_model": status.get("translation_model_id", "unverified"),
            "device": status.get("selected_device", "unverified"),
        },
        "results": records,
        "proof": "EXECUTED_DIRECT_WORKER_STAGES_NO_VAD_MICROPHONE_OR_RECEIVER",
        "transport_incomplete": stopped,
    }
    result_path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=result_path.parent, mode="w",
                                     encoding="utf-8", suffix=".tmp", delete=False) as handle:
        handle.write(json.dumps(output, ensure_ascii=False, indent=2)+"\n")
        staged = Path(handle.name)
    try:
        if result_path.exists():
            raise ValueError("observation destination appeared concurrently")
        os.replace(staged, result_path)
    finally:
        staged.unlink(missing_ok=True)
    return output


NATIVE_TRANSLATION_PROOF = "NATIVE_OWNER_TRANSLATION_ONLY_NOT_MEETING_ACCEPTANCE"
NATIVE_MT_MODEL = "hy-mt2-1.8b-q6_k"
NATIVE_MT_REVISION = "a0c709d9fac510f2c807aa3af52872340dc37a4a"


NATIVE_ASR_PROOF = "NATIVE_OWNER_FINALIZED_ASR_ONLY_NOT_MEETING_ACCEPTANCE"
NATIVE_ASR_MODEL = "qwen3-asr-1.7b-q8_0"


def collect_native_audio_rows(bundle: dict, proof: str, fingerprint_key: str,
                              receipt_schema: str) -> tuple[dict, list]:
    """Validate native ASR and optional MT before any result is published."""
    receipt = native_quality_replay(bundle, test_name="explicit_native_asr_quality_replay",
                                    receipt_schema=receipt_schema)
    pci = receipt.get("cuda_pci_id")
    rows = receipt.get("results")
    if (receipt.get("proof") != proof
            or receipt.get("source_sha") != bundle["source_sha"]
            or receipt.get(fingerprint_key) != bundle[fingerprint_key]
            or receipt.get("noise_suppression_enabled") is not False
            or not isinstance(pci, str)
            or not re.fullmatch(r"(?:[a-f0-9]{4}|[a-f0-9]{8}):[a-f0-9]{2}:[a-f0-9]{2}\.[0-7]", pci)
            or not re.fullmatch(r"[a-f0-9]{64}", str(receipt.get("dll_pins_sha256", "")))
            or type(receipt.get("worker_pid")) is not int or receipt["worker_pid"] <= 0
            or receipt.get("worker_terminated") is not True
            or receipt.get("execution_error") is not None
            or not isinstance(rows, list) or len(rows) != len(bundle["requests"])):
        raise WorkerTransportError("native ASR replay identity, coverage or cleanup invalid")
    seen = set()
    for item, row in zip(bundle["requests"], rows, strict=True):
        ident = item["case_id"]
        response = row.get("response") if isinstance(row, dict) else None
        if not isinstance(response, dict):
            raise WorkerTransportError("native ASR response missing")
        request_id = response.get("request_id")
        text, ok = response.get("transcript_text"), response.get("ok")
        if (row.get("case_id") != ident or row.get("audio_sha256") != item["audio_sha256"]
                or row.get("frames") != item["frames"]
                or not isinstance(request_id, str) or not re.fullmatch(r"helper-[1-9][0-9]*", request_id)
                or request_id in seen or response.get("stage") != "transcribe"
                or response.get("device") != "cuda" or response.get("cuda_pci_id") != pci
                or response.get("model_id") != NATIVE_ASR_MODEL or response.get("compute_type") != "q8_0"
                or response.get("language") != item["language"] or type(ok) is not bool
                or not isinstance(text, str)
                or type(response.get("elapsed_ms")) is not int or response["elapsed_ms"] < 0
                or not ((ok and text.strip() and response.get("blocker") == "")
                        or (not ok and text == "" and response.get("blocker") == "asr:empty_transcript"))):
            raise WorkerTransportError("native ASR segment response contract invalid")
        seen.add(request_id)
        translation = row.get("translation")
        if ok and item.get("translate", False):
            target = "en" if item["language"] == "id" else "id"
            pair = f"{item['language']}->{target}"
            mt_id = translation.get("request_id") if isinstance(translation, dict) else None
            if (not isinstance(translation, dict) or not translation_receipt_complete(translation, pair)
                    or not isinstance(mt_id, str) or not re.fullmatch(r"helper-[1-9][0-9]*", mt_id)
                    or mt_id in seen or translation.get("stage") != "translate"
                    or translation.get("device") != "cuda" or translation.get("cuda_pci_id") != pci
                    or translation.get("model_id") != NATIVE_MT_MODEL
                    or translation.get("model_revision") != NATIVE_MT_REVISION
                    or translation.get("precision") != "q6_k"
                    or translation.get("source_language") != item["language"]
                    or translation.get("target_language") != target
                    or translation.get("translation_contract") != "canonical_bidirectional_id_en"
                    or translation.get("translation_degraded") is not False
                    or translation.get("translation_fallback_reason") != ""
                    or translation.get("hit_token_ceiling") is not False or translation.get("blocker") != ""
                    or type(translation.get("elapsed_ms")) is not int or translation["elapsed_ms"] < 0):
                raise WorkerTransportError("native audio translation response contract invalid")
            seen.add(mt_id)
        elif "translation" in row:
            raise WorkerTransportError("unrequested native audio translation")
    return receipt, rows


def collect_native_finalized_asr(report: dict, cases: dict, selection: set[str],
                                 observed: dict, segments_dir: Path, timeout: float) -> dict:
    """Collect every verified segment in canonical order; no fake Meeting or voice."""
    if not 5 <= timeout <= 600:
        raise ValueError("native request timeout outside 5–600 seconds")
    requests, segments = [], {}
    report.update(schema="translateit.finalized_asr.native_run.v1", proof=NATIVE_ASR_PROOF,
                  selection=sorted(selection), noise_suppression_enabled=False, execution_complete=False)
    report["source_identity"]["context"] = "LOCAL_CODE_NATIVE_FINALIZED_ASR_ONLY"
    for name, case in cases.items():
        if name not in selection:
            continue
        vad = observed[name]
        if not vad["segments"]:
            rejected = (case["kind"] != "speech" and vad.get("ok") is True
                        and vad.get("finalized_count") == 0 and vad.get("overflow_count") == 0)
            report["results"].append({"scenario_id": name, "round": 1,
                "outcome": "VAD_REJECTED_NON_SPEECH" if rejected else "NO_VAD_SEGMENT",
                "vad": {k: vad.get(k) for k in ("ok", "finalized_count", "overflow_count")},
                "asr_executed": False})
        for segment in vad["segments"]:
            ident = segment["file"]
            segments[ident] = (name, segment)
            requests.append({"case_id": ident, "audio_path": str((segments_dir / ident).resolve(strict=True)),
                "audio_sha256": segment["sha256"], "frames": segment["frames"], "language": case["language"]})
    if not requests:
        report["proof"] = "VAD_ONLY_NO_NATIVE_ASR_REQUESTS"
        return report
    bundle = {"schema": "translateit.native_asr.requests.v1",
              "source_sha": report["source_identity"]["local_sha"],
              "segment_fingerprint": report["segment_fingerprint"],
              "timeout_ms": round(timeout * 1000), "requests": requests}
    try:
        receipt, rows = collect_native_audio_rows(bundle, NATIVE_ASR_PROOF,
            "segment_fingerprint", "translateit.native_asr.replay.v1")
        accepted = []
        for row in rows:
            ident, response = row["case_id"], row["response"]
            name, segment = segments[ident]
            accepted.append({"scenario_id": name, "round": 1, "segment": segment,
                "input_sha256": row["audio_sha256"], "asr": stage_result(response, "asr"),
                "asr_runtime": {k: response[k] for k in ("model_id", "device", "compute_type")},
                "native_response": response,
                "outcome": finalized_asr_outcome(response, non_speech=cases[name]["kind"] != "speech")})
        report["results"].extend(accepted)
        report.update({k: receipt[k] for k in ("request_sha256", "dll_pins_sha256",
                                              "cuda_pci_id", "worker_pid", "worker_terminated")})
        report["execution_complete"] = True
    except (OSError, ValueError, WorkerTransportError, subprocess.TimeoutExpired) as error:
        report["transport_error"] = str(error)
        report["proof"] = "NATIVE_ASR_REPLAY_INCOMPLETE"
    return report


def native_translation_requests(corpus: dict, expected_sha: str, timeout: float, grader) -> dict:
    if not 5 <= timeout <= 600:
        raise ValueError("native request timeout outside 5–600 seconds")
    # This is model-stage evaluation, not a fake Meeting generation.
    # Context pairs remain in the corpus only. No history is sent.
    return {
        "schema": "translateit.native_translation.requests.v1",
        "source_sha": expected_sha, "corpus_fingerprint": grader.corpus_fingerprint(corpus),
        "timeout_ms": round(timeout * 1000),
        "requests": [
            {"case_id": item["case_id"], **{key: item["request"][key] for key in
                ("text", "source_language", "target_language")}}
            for item in grader.emit_requests(corpus)["requests"]
        ],
    }


def native_quality_replay(requests: dict, *, test_name: str, receipt_schema: str) -> dict:
    """One explicit offline Rust test process, one resident native owner."""
    with tempfile.TemporaryDirectory(prefix="native-quality-") as work:
        root = Path(work).resolve()
        request_path, receipt_path = root / "requests.json", root / "receipt.json"
        request_path.write_text(json.dumps(requests, ensure_ascii=False), encoding="utf-8")
        request_hash = fingerprint(request_path)
        env = {**os.environ, "CARGO_NET_OFFLINE": "true", "CARGO_BUILD_JOBS": "2",
               "RUST_TEST_THREADS": "1", "HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1",
               "TRANSLATEIT_NATIVE_QUALITY_REQUESTS": str(request_path),
               "TRANSLATEIT_NATIVE_QUALITY_RESULTS": str(receipt_path),
               "TRANSLATEIT_NATIVE_QUALITY_REQUEST_SHA256": request_hash}
        stage_count = sum(1 + bool(item.get("translate")) for item in requests["requests"])
        timeout = 900 + stage_count * (requests["timeout_ms"] / 1000 + 15)
        result = subprocess.run(
            ["cargo", "test", "--locked", "--offline", test_name,
             "--", "--ignored", "--test-threads=1"],
            cwd=ROOT / "EngineData/Frontend/RustApp/src-tauri", env=env, shell=False,
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=timeout,
        )
        if result.returncode != 0 or not receipt_path.is_file() or receipt_path.is_symlink():
            raise WorkerTransportError("native replay or cleanup failed: " + (result.stderr[-700:] or result.stdout[-700:]))
        with receipt_path.open("rb") as stream:
            data = stream.read(16 * 1024 * 1024 + 1)
        if len(data) > 16 * 1024 * 1024:
            raise WorkerTransportError("native receipt exceeds bound")
        receipt = json.loads(data)
        if (not isinstance(receipt, dict) or receipt.get("request_sha256") != request_hash
                or receipt.get("schema") != receipt_schema):
            raise WorkerTransportError("native request identity mismatch")
        return receipt


def native_translation_replay(requests: dict) -> dict:
    return native_quality_replay(requests, test_name="explicit_native_translation_quality_replay",
                                 receipt_schema="translateit.native_translation.replay.v1")


def run_translation_quality(expected_sha: str, suite: str, timeout: float) -> dict:
    """Native translation-only replay and the unchanged semantic grader."""
    pinned_source(expected_sha)
    quality_root = ROOT / "tools/translation_quality"
    spec = importlib.util.spec_from_file_location("worker_translation_quality", quality_root / "evaluate_translation_quality.py")
    grader = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(grader)
    corpus = grader.select_suite(grader.load_corpus(quality_root / "corpus/translation_quality_v1.json"), suite)
    requests = native_translation_requests(corpus, expected_sha, timeout, grader)
    records, rejected, receipt = [], {}, {}
    error = None
    started = time.monotonic()
    try:
        receipt = native_translation_replay(requests)
        pci = receipt.get("cuda_pci_id")
        rows = receipt.get("results")
        if (receipt.get("proof") != NATIVE_TRANSLATION_PROOF
                or receipt.get("source_sha") != expected_sha
                or receipt.get("corpus_fingerprint") != requests["corpus_fingerprint"]
                or not isinstance(pci, str) or not re.fullmatch(r"(?:[a-f0-9]{4}|[a-f0-9]{8}):[a-f0-9]{2}:[a-f0-9]{2}\.[0-7]", pci)
                or not re.fullmatch(r"[a-f0-9]{64}", str(receipt.get("dll_pins_sha256", "")))
                or not re.fullmatch(r"[a-f0-9]{64}", str(receipt.get("request_sha256", "")))
                or type(receipt.get("worker_pid")) is not int or receipt["worker_pid"] <= 0
                or receipt.get("worker_terminated") is not True
                or not isinstance(rows, list) or len(rows) > len(requests["requests"])):
            raise WorkerTransportError("native replay receipt identity or cleanup invalid")
        seen = set()
        for item, row in zip(requests["requests"], rows):
            ident = item["case_id"]
            direction = f"{item['source_language']}->{item['target_language']}"
            response = row.get("response") if isinstance(row, dict) else None
            request_id = response.get("request_id") if isinstance(response, dict) else None
            if (not isinstance(row, dict) or row.get("case_id") != ident or not isinstance(response, dict)
                    or not isinstance(request_id, str) or not re.fullmatch(r"helper-[1-9][0-9]*", request_id)
                    or request_id in seen or not translation_receipt_complete(response, direction)
                    or response.get("stage") != "translate" or response.get("device") != "cuda"
                    or response.get("cuda_pci_id") != pci or response.get("precision") != "q6_k"
                    or response.get("model_id") != NATIVE_MT_MODEL or response.get("model_revision") != NATIVE_MT_REVISION
                    or response.get("source_language") != item["source_language"]
                    or response.get("target_language") != item["target_language"]
                    or response.get("translation_contract") != "canonical_bidirectional_id_en"
                    or response.get("translation_degraded") is not False
                    or response.get("translation_fallback_reason") != ""
                    or response.get("blocker") != "" or response.get("hit_token_ceiling") is not False):
                rejected[ident] = {"stage": "translation_output_contract", "response": response}
                error = "translation:incomplete_or_unbound_output"
                break
            seen.add(request_id)
            records.append({"case_id": ident, "translated_text": response["translated_text"], "response": response})
        if receipt.get("execution_error") is not None or len(records) != len(requests["requests"]):
            error = error or str(receipt.get("execution_error") or "native replay incomplete")
    except (OSError, ValueError, WorkerTransportError, subprocess.TimeoutExpired) as exc:
        error = str(exc)
    try:
        pinned_source(expected_sha)
    except ValueError as exc:
        error = f"source_identity:{exc}"
    identity = f"{expected_sha}:{NATIVE_MT_MODEL}@{NATIVE_MT_REVISION}"
    values = {row["case_id"]: row["translated_text"] for row in records}
    evaluation = grader.evaluate(corpus, values, result_corpus_fingerprint=grader.corpus_fingerprint(corpus), source_identity=identity)
    checks = []
    for index, row in enumerate(evaluation["cases"]):
        ident = row["case_id"]
        stage = rejected.get(ident, {}).get("stage")
        status = "FAIL" if stage else "UNVERIFIED" if ident not in values else row["quality_status"]
        checks.append({"case_id": ident, "status": status,
                       "first_wrong_stage": stage or ("worker_execution" if ident not in values else "translation_quality_check" if status in ("FAIL", "REVIEW") else None),
                       "requires_linguistic_review": ident in values and not row["critical_pass"],
                       "expected": "complete canonical model output and declared semantic invariants",
                       "evidence": f"#/rejected/{ident}" if stage else f"#/evaluation/cases/{index}"})
    complete = not error and receipt.get("worker_terminated") is True and evaluation["complete_result_set"]
    return {"schema": "translateit.translation_quality.native_run.v1",
            "execution_profile": {"name": f"translation-{suite}"},
            "proof": NATIVE_TRANSLATION_PROOF,
            "source_identity": identity, "corpus_fingerprint": grader.corpus_fingerprint(corpus),
            "selection": corpus.get("selection"),
            "dll_pins_sha256": receipt.get("dll_pins_sha256"), "request_sha256": receipt.get("request_sha256"),
            "cuda_pci_id": receipt.get("cuda_pci_id"),
            "worker_terminated": receipt.get("worker_terminated") is True,
            "worker_pid": receipt.get("worker_pid"), "execution_complete": bool(complete), "execution_error": error,
            "elapsed_ms": round((time.monotonic() - started) * 1000),
            "results": records, "rejected": rejected, "evaluation": evaluation, "checks": checks,
            "counts": {state: sum(row["status"] == state for row in checks) for state in ("PASS", "REVIEW", "FAIL", "UNVERIFIED")},
            "verdict": evaluation["verdict"] if complete else "FAIL"}


def run_source_checks(expected_sha: str) -> dict:
    """Bounded existing validators; no model imports, downloads or native capture."""
    pinned_source(expected_sha)
    commands = [
        ("audio_contracts", [sys.executable, "-m", "unittest", "tools.tests.test_audio_validation",
                            "tools.tests.test_audio_vad_receipts", "tools.tests.test_audio_pipeline",
                            "tools.tests.test_audio_fault_runner", "tools.tests.test_translation_worker"]),
        ("asr_evaluator", [sys.executable, "tools/asr_quality/self_test.py"]),
        ("translation_evaluator", [sys.executable, "tools/translation_quality/self_test.py"]),
        ("tts_evaluator", [sys.executable, "tools/tts_quality/self_test.py"]),
    ]
    checks = []
    for name, command in commands:
        started = time.monotonic()
        try:
            result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, encoding="utf-8",
                                    timeout=120, env={**os.environ, "PYTHONUTF8": "1"}, check=False)
            check = {"name": name, "status": "PASS" if result.returncode == 0 else "FAIL",
                     "expected": "validator exit 0", "exit_code": result.returncode,
                     "stdout": result.stdout, "stderr": result.stderr}
        except subprocess.TimeoutExpired:
            check = {"name": name, "status": "FAIL", "expected": "validator completes within 120 seconds",
                     "error": "source_validator_deadline_exceeded"}
        check["elapsed_ms"] = round((time.monotonic() - started) * 1000)
        checks.append(check)
    pinned_source(expected_sha)
    counts = {state: sum(c["status"] == state for c in checks) for state in ("PASS", "FAIL", "UNVERIFIED")}
    return {"schema": "translateit.audio_source_checks.v1", "source_identity": {"local_sha": expected_sha},
            "checks": checks, "counts": counts, "verdict": "FAIL" if counts["FAIL"] else "PASS",
            "proof": "EXECUTED_SOURCE_VALIDATORS_NOT_MODEL_OR_NATIVE_ACCEPTANCE"}


def publish_report(path: Path, report: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("x", encoding="utf-8") as handle:
        json.dump(report, handle, ensure_ascii=False, indent=2)
        handle.write("\n")


def report_summary(report: dict, output: Path) -> dict:
    failed = [c for c in report.get("checks", []) if c.get("status") != "PASS"]
    return {"profile": report.get("execution_profile", {}).get("name") if report.get("execution_profile") else "custom",
            "verdict": report.get("verdict", "NOT_GRADED"), "counts": report.get("counts"),
            "proof": report["proof"], "output": str(output), "failures": failed[:5],
            "additional_failures_in_report": max(0, len(failed) - 5),
            "execution_error": report.get("execution_error") or report.get("transport_error") or report.get("source_admission_error")}


def admission_failure(profile: dict, source_sha: str, source_checks: dict, stage: str, error: str) -> dict:
    checks = [{"scenario_id": name, "round": iteration, "status": "UNVERIFIED", "first_wrong_stage": stage,
               "expected": "source validators and current canonical VAD complete before worker inference",
               "actual": error, "evidence": ["#/source_checks"]}
              for iteration in range(1, profile["rounds"] + 1) for name in profile["scenario_ids"]]
    return {"schema": "translateit.finalized_pipeline.v2", "execution_profile": profile,
            "source_identity": {"local_sha": source_sha}, "source_checks": source_checks,
            "verdict": "FAIL", "counts": {"PASS": 0, "FAIL": 0, "UNVERIFIED": len(checks)}, "checks": checks,
            "source_admission_error": error, "proof": "ADMISSION_FAILED_PIPELINE_NOT_EXECUTED"}


def main() -> int:
    parser = argparse.ArgumentParser(description="Explicit local AI worker audio smoke; never CI")
    parser.add_argument("--fixtures", type=Path)
    parser.add_argument("--python", type=Path)
    parser.add_argument("--source-sha", required=True)
    selection = parser.add_mutually_exclusive_group(required=True)
    selection.add_argument("--ids", nargs="+")
    selection.add_argument("--profile", choices=("source", "core", "endurance"))
    selection.add_argument("--translation-suite", choices=("fast", "full"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--vad-timeout", type=int, default=600)
    parser.add_argument("--include-voice", action="store_true")
    parser.add_argument("--vad-observations", type=Path)
    parser.add_argument("--segments-dir", type=Path)
    parser.add_argument("--finalized-pipeline", action="store_true")
    parser.add_argument("--rounds", type=int)
    parser.add_argument("--vad-source-sha", help="Explicit retained VAD source; never current VAD proof")
    args = parser.parse_args()
    if args.output.exists() or args.output.is_symlink():
        parser.error("output must be a new report; previous evidence is immutable")
    if args.translation_suite:
        if any((args.python, args.fixtures, args.include_voice, args.finalized_pipeline,
                args.vad_observations, args.segments_dir, args.vad_source_sha, args.rounds is not None)):
            parser.error("native translation suite accepts no Python worker or audio/round arguments")
        result = run_translation_quality(args.source_sha, args.translation_suite, args.timeout)
        publish_report(args.output, result)
        print(json.dumps(report_summary(result, args.output), ensure_ascii=False, indent=2))
        return 0 if result["verdict"] == "PASS" else 2
    if args.profile and args.rounds is not None:
        parser.error("named profiles own their round count; --rounds is for custom --ids only")
    source_checks = None
    if args.profile == "source":
        if any((args.fixtures, args.python, args.vad_observations, args.segments_dir, args.include_voice,
                args.finalized_pipeline, args.vad_source_sha)):
            parser.error("source profile accepts no model/audio arguments")
        result = run_source_checks(args.source_sha)
        result["execution_profile"] = {"name": "source"}
        publish_report(args.output, result)
        print(json.dumps(report_summary(result, args.output), ensure_ascii=False, indent=2))
        return 0 if result["verdict"] == "PASS" else 2
    if args.fixtures is None:
        parser.error("audio execution requires --fixtures")
    legacy_voice = bool(args.profile or args.finalized_pipeline or args.include_voice)
    if not legacy_voice and args.python is not None:
        parser.error("native audio diagnostics accept no --python")
    if legacy_voice and args.python is None:
        parser.error("legacy voice diagnostics require --python")
    profile = regression_profile(args.profile) if args.profile else None
    rounds = profile["rounds"] if profile else (args.rounds if args.rounds is not None else 1)
    ids = set(profile["scenario_ids"] if profile else args.ids)
    if profile:
        if any((args.vad_observations, args.segments_dir, args.include_voice, args.vad_source_sha)):
            parser.error("core/endurance run fresh VAD themselves; retained inputs require custom --ids")
        artifacts = args.output.with_suffix(args.output.suffix + ".artifacts")
        if artifacts.exists() or artifacts.is_symlink():
            parser.error("regression artifacts must use a new destination")
        source_checks = run_source_checks(args.source_sha)
        if not source_admission_passed(source_checks, args.source_sha):
            result = admission_failure(profile, args.source_sha, source_checks, "source_admission", "source validators failed; no models started")
            publish_report(args.output, result)
            print(json.dumps(report_summary(result, args.output), ensure_ascii=False, indent=2))
            return 2
        from vad_runner import run as run_vad

        args.vad_observations = artifacts / "vad.json"
        args.segments_dir = artifacts / "segments"
        print("audio-regression: source admission passed; running canonical VAD", file=sys.stderr, flush=True)
        try:
            run_vad(args.fixtures, args.source_sha, args.vad_observations, args.vad_timeout, segments_dir=args.segments_dir)
        except (OSError, ValueError, subprocess.TimeoutExpired, subprocess.CalledProcessError) as error:
            result = admission_failure(profile, args.source_sha, source_checks, "vad_execution", str(error))
            publish_report(args.output, result)
            print(json.dumps(report_summary(result, args.output), ensure_ascii=False, indent=2))
            return 2
    if args.vad_observations is not None or args.segments_dir is not None:
        if args.vad_observations is None or args.segments_dir is None or args.include_voice:
            parser.error("finalized input requires both VAD arguments; use --finalized-pipeline for selected voice")
        result = run_finalized(args.fixtures, args.python, args.source_sha, ids,
                               args.output, args.timeout, args.vad_observations, args.segments_dir,
                               pipeline=args.finalized_pipeline or bool(profile), rounds=rounds, vad_source_sha=args.vad_source_sha,
                               profile=args.profile, source_checks=source_checks, artifacts_dir=artifacts if profile else None)
    else:
        if args.finalized_pipeline or rounds != 1 or args.vad_source_sha:
            parser.error("finalized pipeline options require finalized VAD/WAV inputs")
        result = run(args.fixtures, args.python, args.source_sha,
                     ids, args.output, args.include_voice, args.timeout)
    print(json.dumps(report_summary(result, args.output), ensure_ascii=False, indent=2))
    if args.vad_observations is not None:
        return 0 if result["verdict"] == "PASS" else 2
    if not args.include_voice:
        return 0 if result["execution_complete"] else 2
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, TypeError, OSError, WorkerTransportError, subprocess.CalledProcessError) as error:
        print(f"audio-runner: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from None
