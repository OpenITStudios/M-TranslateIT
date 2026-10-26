"""Explicit, offline persistent-worker smoke for hashed public audio fixtures.

No GitHub CI, microphone, CPAL, Meeting session, cloud, or model downloads.
This exercises direct worker ASR/translation/optional selected voice only.
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
    CORPUS,
    RESULT_SCHEMA,
    ROOT,
    fingerprint,
    inspect,
    invariant_result,
    load_corpus,
    metrics,
    specs,
    translation_receipt_complete,
    voice_receipt_valid,
    word_error_rate,
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
        result["transcript_text"] = str(response.get("transcript_text", ""))[:4000]
        result["note"] = str(response.get("note", ""))[:1000]
    elif name == "translation":
        result.update({
            "model_id": response.get("model_id"),
            "model_revision": response.get("model_revision"),
            "device": response.get("device"),
            "direction_pair": response.get("direction_pair"),
            "complete": response.get("complete") is True,
            "finished_with_eos": response.get("finished_with_eos") is True,
            "translated_text": str(response.get("translated_text", ""))[:4000],
        })
    return result


def finalized_asr_outcome(response: dict, *, non_speech: bool) -> str:
    text = response.get("transcript_text", "")
    if non_speech:
        if response.get("ok") is True and isinstance(text, str) and text.strip():
            return "NON_SPEECH_TRANSCRIPT"
        if (response.get("ok") is False and not text
                and response.get("blocker") == "asr:empty_transcript"):
            return "ASR_REJECTED_EMPTY"
        return "UNVERIFIED_ASR_FAILURE"
    return ("SPEECH_TRANSCRIBED" if response.get("ok") is True
            and isinstance(text, str) and text.strip() else "SPEECH_ASR_FAILED")


def grade_finalized_asr(report: dict, cases: dict, selection: set[str]) -> dict:
    """Use canonical ASR reference/invariant scoring; failed controls cannot pass."""
    corpus = {r["id"]: r for r in load_corpus(CORPUS)["cases"]}
    checks = []
    for name in sorted(selection):
        case = cases[name]
        rows = [r for r in report["results"] if r["scenario_id"] == name]
        outcomes = [r["outcome"] for r in rows]
        check = {"scenario_id": name, "status": "UNVERIFIED"}
        if any(r in ("SPEECH_ASR_FAILED", "NON_SPEECH_TRANSCRIPT") for r in outcomes):
            check["status"] = "FAIL"
        elif (case["kind"] != "speech" and len(rows) == 1
              and outcomes == ["VAD_REJECTED_NON_SPEECH"]
              and rows[0].get("asr_executed") is False and not rows[0].get("asr")
              and rows[0].get("vad") == {"ok": True, "finalized_count": 0, "overflow_count": 0}):
            check.update({"status": "PASS", "proof": "CANONICAL_VAD_REJECTED_BEFORE_ASR"})
        elif rows and all(r == "ASR_REJECTED_EMPTY" for r in outcomes) and case["kind"] != "speech":
            check["status"] = "PASS"
        elif rows and all(r == "SPEECH_TRANSCRIBED" for r in outcomes) and case["kind"] == "speech":
            transcript = " ".join(r["asr"]["transcript_text"].strip() for r in rows)
            wer = word_error_rate(case["reference"], transcript)
            critical = (invariant_result(corpus[case["case_id"]], transcript)
                        if case.get("case_id") else {"critical_pass": True})
            check.update({"wer": wer, "critical": critical,
                          "status": "PASS" if wer <= case["max_wer"] and critical["critical_pass"] else "FAIL"})
        checks.append(check)
    counts = {name: sum(r["status"] == name for r in checks) for name in ("PASS", "FAIL", "UNVERIFIED")}
    verdict = ("FAIL" if counts["FAIL"] else "UNVERIFIED"
               if counts["UNVERIFIED"] or report.get("transport_error") else "PASS")
    return {"verdict": verdict, "counts": counts, "checks": checks}


def pipeline_contracts(cases: dict) -> tuple[dict, object, str]:
    """Semantic expectations remain in the canonical translation corpus."""
    root = ROOT / "tools/translation_quality"
    spec = importlib.util.spec_from_file_location("pipeline_translation_quality", root / "evaluate_translation_quality.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    corpus = module.load_corpus(root / "corpus/translation_quality_v1.json")
    contracts = {}
    for item in corpus["cases"]:
        name = item.get("audio_scenario_id")
        if name is None:
            continue
        if (name not in cases or name in contracts or item["source"] != cases[name]["reference"]
                or item["direction"] != ("id-en" if cases[name]["language"] == "id" else "en-id")):
            raise ValueError("translation contract differs from canonical audio scenario")
        contracts[name] = item
    return contracts, module, module.corpus_fingerprint(corpus)


def grade_pipeline(report: dict, cases: dict, selection: set[str], rounds: int,
                   contracts: dict, evaluator: object) -> dict:
    checks = []
    for iteration in range(1, rounds + 1):
        rows = [r for r in report["results"] if r.get("round") == iteration]
        asr = grade_finalized_asr({"results": rows}, cases, selection)
        for first in asr["checks"]:
            name = first["scenario_id"]
            group = [r for r in rows if r["scenario_id"] == name]
            check = {**first, "round": iteration, "first_wrong_stage": "asr" if first["status"] != "PASS" else None}
            if check["status"] != "PASS":
                checks.append(check)
                continue
            if cases[name]["kind"] != "speech":
                if (len(group) != 1 or group[0].get("outcome") != "VAD_REJECTED_NON_SPEECH"
                        or any(key in group[0] for key in ("translation", "tts", "roundtrip_asr"))):
                    check.update(status="FAIL", first_wrong_stage="vad_suppression")
                checks.append(check)
                continue
            translated = [r.get("translation", {}) for r in group]
            direction = "id->en" if cases[name]["language"] == "id" else "en->id"
            if not all(translation_receipt_complete(r, direction) for r in translated):
                check.update(status="FAIL", first_wrong_stage="translation_envelope")
            elif name not in contracts:
                check.update(status="UNVERIFIED", first_wrong_stage="translation_contract_missing")
            else:
                text = " ".join(r["translated_text"] for r in translated)
                check["translation_critical"] = evaluator.invariant_result(contracts[name], text)
                if not check["translation_critical"]["critical_pass"]:
                    check.update(status="FAIL", first_wrong_stage="translation_critical")
                elif cases[name]["language"] == "id":
                    if not all(voice_receipt_valid(r.get("tts", {}), report.get("voice_preflight"))
                               and r["tts"].get("actor_identity_match") is True
                               and r["tts"].get("output_identity_match") is True for r in group):
                        check.update(status="FAIL", first_wrong_stage="tts_identity_or_audio")
                    elif not all(r.get("roundtrip_asr", {}).get("ok") is True for r in group):
                        check.update(status="FAIL", first_wrong_stage="tts_roundtrip_asr")
                    else:
                        spoken = " ".join(r["roundtrip_asr"]["transcript_text"] for r in group)
                        check["spoken_critical"] = evaluator.invariant_result(contracts[name], spoken)
                        check["spoken_wer"] = word_error_rate(text, spoken)
                        if not check["spoken_critical"]["critical_pass"]:
                            check.update(status="FAIL", first_wrong_stage="tts_critical_content")
                        elif check["spoken_wer"] != 0:
                            check.update(status="UNVERIFIED", first_wrong_stage="tts_content_difference")
                elif any("tts" in r or "roundtrip_asr" in r for r in group):
                    check.update(status="FAIL", first_wrong_stage="incoming_unexpected_tts")
            checks.append(check)
    counts = {status: sum(c["status"] == status for c in checks) for status in ("PASS", "FAIL", "UNVERIFIED")}
    if report.get("preload_blockers"):
        checks = [{**c, "status": "FAIL", "first_wrong_stage": "worker_preload"} for c in checks]
        counts = {"PASS": 0, "FAIL": len(checks), "UNVERIFIED": 0}
    verdict = "FAIL" if counts["FAIL"] else "UNVERIFIED" if counts["UNVERIFIED"] or report.get("transport_error") else "PASS"
    return {"checks": checks, "counts": counts, "verdict": verdict}


def run_finalized(fixtures: Path, python: Path, expected_sha: str, selection: set[str],
                  result_path: Path, timeout: float, vad_observations: Path,
                  segments_dir: Path, *, pipeline: bool = False, rounds: int = 1,
                  vad_source_sha: str | None = None) -> dict:
    """Diagnostic ASR on actual exported Rust segments, not app acceptance."""
    from vad_runner import CLAIM, verify_segments

    pinned_source(expected_sha)
    if type(rounds) is not int or not 1 <= rounds <= 20 or (not pipeline and (rounds != 1 or vad_source_sha is not None)):
        raise ValueError("repeated/retained inputs require finalized pipeline; rounds must be 1–20")
    input_sha = vad_source_sha or expected_sha
    if not re.fullmatch(r"[a-f0-9]{40}", input_sha):
        raise ValueError("exact finalized VAD source SHA required")
    manifest = inspect(fixtures)
    suite, suite_id = specs()
    cases = {r["id"]: r for r in suite}
    if not selection or not selection.issubset(cases):
        raise ValueError("select known finalized scenario IDs")
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
    if pipeline:
        contracts, evaluator, contract_id = pipeline_contracts(cases)
        report["source_identity"]["context"] = "LOCAL_CODE_RETAINED_FINALIZED_WAV_TO_WORKER_PIPELINE"
        report.update(schema="translateit.finalized_pipeline.v1", rounds=rounds,
                      input_source_identity=prior["source_identity"],
                      vad_receipt_sha256=vad_receipt_hash,
                      translation_contract_fingerprint=contract_id,
                      acceptance_scope="DECLARED_PUBLIC_FIXTURE_CRITICALS_AND_SPOKEN_CONTENT_PROXY_NOT_GENERAL_SEMANTIC_OR_NATURALNESS_ACCEPTANCE",
                      downstream="TRANSLATION_SELECTED_ENGLISH_VOICE_CONTENT_PROXY_NOT_MEETING",
                      proof="CURRENT_WORKER_ON_HASH_VERIFIED_FINALIZED_INPUTS_NOT_FRESH_VAD_OR_MEETING")
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
                        if not pipeline:
                            report["proof"] = "EXPORTED_CANONICAL_INCOMING_VAD_WAV_TO_REAL_WORKER_ASR_ONLY"
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
                                tts.update(actor_token=produced.get("actor_token"), output_path=produced.get("output_path"))
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
                    if pipeline:
                        report["snapshots"].append({"round": iteration, "scenario_id": name,
                                                    "status": worker.invoke("status")})
        except WorkerTransportError as exc:
            report["transport_error"] = str(exc)
        finally:
            worker.close()
            if pipeline:
                report["fixture_worker_terminated"] = worker.child.poll() is not None
    # Original exports remain available for reproducibility after worker cleanup.
    if verify_segments(segments_dir, observed) != segment_id:
        raise ValueError("exported finalized segments changed during ASR")
    if fingerprint(vad_observations) != vad_receipt_hash:
        raise ValueError("finalized VAD receipt changed during execution")
    pinned_source(expected_sha)
    report.update(grade_pipeline(report, cases, selection, rounds, contracts, evaluator)
                  if pipeline else grade_finalized_asr(report, cases, selection))
    result_path.parent.mkdir(parents=True, exist_ok=True)
    with result_path.open("x", encoding="utf-8") as output:
        json.dump(report, output, ensure_ascii=False, indent=2)
        output.write("\n")
    return report


def run(fixtures: Path, python: Path, expected_sha: str,
        selection: set[str], result_path: Path, voice: bool, timeout: float) -> dict:
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


def main() -> int:
    parser = argparse.ArgumentParser(description="Explicit local AI worker audio smoke; never CI")
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--ids", nargs="+", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--include-voice", action="store_true")
    parser.add_argument("--vad-observations", type=Path)
    parser.add_argument("--segments-dir", type=Path)
    parser.add_argument("--finalized-pipeline", action="store_true")
    parser.add_argument("--rounds", type=int, default=1)
    parser.add_argument("--vad-source-sha", help="Explicit retained VAD source; never current VAD proof")
    args = parser.parse_args()
    if args.vad_observations is not None or args.segments_dir is not None:
        if args.vad_observations is None or args.segments_dir is None or args.include_voice:
            parser.error("finalized input requires both VAD arguments; use --finalized-pipeline for selected voice")
        result = run_finalized(args.fixtures, args.python, args.source_sha, set(args.ids),
                               args.output, args.timeout, args.vad_observations, args.segments_dir,
                               pipeline=args.finalized_pipeline, rounds=args.rounds, vad_source_sha=args.vad_source_sha)
    else:
        if args.finalized_pipeline or args.rounds != 1 or args.vad_source_sha:
            parser.error("finalized pipeline options require finalized VAD/WAV inputs")
        result = run(args.fixtures, args.python, args.source_sha,
                     set(args.ids), args.output, args.include_voice, args.timeout)
    print(json.dumps({"cases": len(result["results"]), "proof": result["proof"],
                      "output": str(args.output), "verdict": result.get("verdict", "NOT_GRADED")}, indent=2))
    return 2 if args.vad_observations is not None and result["verdict"] != "PASS" else 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, TypeError, OSError, WorkerTransportError, subprocess.CalledProcessError) as error:
        print(f"audio-runner: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from None
