"""Offline finalized-input grading; no worker, device or model is started here.

ASR and translation semantics remain owned by their canonical evaluators.
"""
from __future__ import annotations

import importlib.util
import math
import re
import statistics
import sys

from harness import (
    CORPUS,
    ROOT,
    invariant_result,
    load_corpus,
    regression_profile,
    translation_receipt_complete,
    voice_receipt_valid,
    word_error_rate,
)

sys.path.insert(0, str(ROOT / "tools/tts_quality"))
from evaluate_tts_quality import word_error_rate as spoken_word_error_rate

SOURCE_CHECK_NAMES = ("audio_contracts", "asr_evaluator", "translation_evaluator", "tts_evaluator")


def source_admission_passed(report: dict | None, expected_sha: str) -> bool:
    if not isinstance(report, dict):
        return False
    checks = report.get("checks", [])
    return (report.get("source_identity") == {"local_sha": expected_sha}
            and report.get("verdict") == "PASS" and isinstance(checks, list)
            and len(checks) == len(SOURCE_CHECK_NAMES)
            and all(isinstance(c, dict) for c in checks)
            and {c.get("name") for c in checks} == set(SOURCE_CHECK_NAMES)
            and all(c.get("status") == "PASS" and type(c.get("exit_code")) is int and c["exit_code"] == 0 for c in checks))


def execution_metrics(report: dict) -> dict:
    timing = {}
    for name in ("asr", "translation", "tts", "roundtrip_asr"):
        samples = sorted(row[name]["elapsed_ms"] for row in report["results"]
                         if type(row.get(name, {}).get("elapsed_ms")) in (int, float)
                         and math.isfinite(row[name]["elapsed_ms"]) and row[name]["elapsed_ms"] >= 0)
        if samples:
            timing[name] = {"count": len(samples), "p50_ms": statistics.median(samples),
                            "p95_ms": samples[math.ceil(len(samples) * 0.95) - 1], "max_ms": samples[-1]}
    memory = [{"round": item["round"], **item["status"]["gpu"]["torch_allocator"]}
              for item in report.get("snapshots", [])
              if item.get("status", {}).get("gpu", {}).get("torch_allocator", {}).get("state") == "measured"]
    return {"stage_timing": timing, "memory_samples": memory,
            "boundary": "WORKER_ELAPSED_AND_ALLOCATOR_SAMPLES_NOT_NATIVE_LATENCY_OR_LEAK_FREE_ACCEPTANCE"}


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


def grade_finalized_asr(report: dict, cases: dict, selection: set[str], *, corpus: dict | None = None) -> dict:
    """Use canonical ASR reference/invariant scoring; failed controls cannot pass."""
    if corpus is None:
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
    corpus = {r["id"]: r for r in load_corpus(CORPUS)["cases"]}
    for iteration in range(1, rounds + 1):
        rows = [r for r in report["results"] if r.get("round") == iteration]
        for name in sorted(selection):
            group = [r for r in rows if r["scenario_id"] == name]
            if report.get("current_vad_required"):
                vad = report.get("vad_stage_results", {}).get(name, {})
                if vad.get("ok") is not True:
                    checks.append({"scenario_id": name, "round": iteration,
                                   "status": "FAIL" if vad.get("ok") is False else "UNVERIFIED",
                                   "first_wrong_stage": "vad"})
                    continue
            expected = report.get("expected_segments", {}).get(name)
            coverage = {"scenario_id": name, "round": iteration, "status": "UNVERIFIED",
                        "first_wrong_stage": "input_coverage"}
            if not isinstance(expected, list) or not group:
                checks.append(coverage)
                continue
            actual_files = [r.get("segment", {}).get("file") for r in group]
            expected_files = [s["file"] for s in expected]
            if expected:
                if len(set(actual_files)) != len(actual_files) or not set(actual_files).issubset(expected_files):
                    checks.append({**coverage, "status": "FAIL"})
                    continue
                if len(actual_files) != len(expected_files):
                    checks.append(coverage)
                    continue
                if actual_files != expected_files or any(
                    row.get("segment") != segment or row.get("input_sha256") != segment["sha256"]
                    for row, segment in zip(group, expected, strict=True)
                ):
                    checks.append({**coverage, "status": "FAIL"})
                    continue
            elif len(group) != 1 or actual_files != [None]:
                checks.append({**coverage, "status": "FAIL"})
                continue
            if expected and not all(isinstance(r.get("asr"), dict) and r["asr"].get("ok") is True
                                    and isinstance(r["asr"].get("transcript_text"), str) for r in group):
                checks.append({**coverage, "status": "FAIL", "first_wrong_stage": "asr"})
                continue
            first = grade_finalized_asr({"results": group}, cases, {name}, corpus=corpus)["checks"][0]
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
                               and bool(report["voice_preflight"].get("actor_token"))
                               and r["tts"].get("actor_token") == report["voice_preflight"]["actor_token"]
                               and bool(r["tts"].get("expected_output_path"))
                               and r["tts"].get("output_path") == r["tts"]["expected_output_path"]
                               and r["tts"].get("output_identity_match") is True for r in group):
                        check.update(status="FAIL", first_wrong_stage="tts_identity_or_audio")
                    elif not all(r.get("roundtrip_asr", {}).get("ok") is True for r in group):
                        check.update(status="FAIL", first_wrong_stage="tts_roundtrip_asr")
                    else:
                        spoken = " ".join(r["roundtrip_asr"]["transcript_text"] for r in group)
                        check["spoken_critical"] = evaluator.invariant_result(contracts[name], spoken)
                        aliases = []
                        for sequence in contracts[name].get("ordered_any", []):
                            for cue in sequence:
                                if isinstance(cue, list) and any(option.isdecimal() for option in cue) and cue not in aliases:
                                    aliases.append(cue)
                        check["spoken_wer"] = spoken_word_error_rate(text, spoken, number_aliases=aliases)
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
    for check in checks:
        if check["status"] == "PASS":
            continue
        name, iteration = check["scenario_id"], check["round"]
        indices = [i for i, row in enumerate(report["results"])
                   if row["scenario_id"] == name and row.get("round") == iteration]
        check["expected"] = {"input": cases[name]["reference"], "segments": report.get("expected_segments", {}).get(name),
                             "translation_contract": contracts.get(name),
                             "voice_actor_token": report.get("voice_preflight", {}).get("actor_token")}
        check["actual"] = [{key: report["results"][i].get(key) for key in
                            ("outcome", "asr", "translation", "tts", "roundtrip_asr")} for i in indices]
        if check["first_wrong_stage"] == "vad":
            check["actual"] = report.get("vad_stage_results", {}).get(name)
        check["evidence"] = ([f"#/results/{i}" for i in indices] or [f"#/expected_segments/{name}"])
        if check["first_wrong_stage"] == "worker_preload":
            check["evidence"].append("#/preload_blockers")
    verdict = "FAIL" if counts["FAIL"] else "UNVERIFIED" if counts["UNVERIFIED"] or report.get("transport_error") else "PASS"
    return {"checks": checks, "counts": counts, "verdict": verdict, "metrics": execution_metrics(report)}


def score_pipeline(report: dict, cases: dict, scenario_fingerprint: str, audio_fingerprint: str) -> dict:
    """Single decision path for execution and offline regrading; starts no models."""
    contracts, evaluator, contract_id = pipeline_contracts(cases)
    selection = report.get("selection")
    rounds = report.get("rounds")
    if not isinstance(report.get("source_identity"), dict):
        raise TypeError("pipeline source identity missing")
    for field in ("expected_segments", "vad_stage_results", "voice_preflight", "input_source_identity"):
        if field in report and not isinstance(report[field], dict):
            raise TypeError(f"malformed pipeline {field}")
    if (report.get("schema") != "translateit.finalized_pipeline.v2"
            or not re.fullmatch(r"[a-f0-9]{40}", str(report.get("source_identity", {}).get("local_sha", "")))
            or report.get("scenario_fingerprint") != scenario_fingerprint
            or report.get("audio_fingerprint") != audio_fingerprint
            or report.get("translation_contract_fingerprint") != contract_id
            or not isinstance(selection, list) or not selection
            or not all(isinstance(name, str) for name in selection)
            or len(set(selection)) != len(selection) or not set(selection).issubset(cases)
            or type(rounds) is not int or not 1 <= rounds <= 20):
        raise ValueError("pipeline source/audio/semantic contract/selection identity differs")
    profile = report.get("execution_profile")
    if profile:
        if not isinstance(profile, dict):
            raise ValueError("invalid execution profile")
        expected = regression_profile(profile.get("name"))
        if (profile != expected or set(selection) != set(expected["scenario_ids"]) or rounds != expected["rounds"]
                or report.get("input_source_identity", {}).get("local_sha") != report["source_identity"]["local_sha"]
                or report.get("current_vad_required") is not True
                or not source_admission_passed(report.get("source_checks"), report["source_identity"]["local_sha"])):
            raise ValueError("named pipeline profile coverage/source admission differs")
    rows = report.get("results")
    if not isinstance(rows, list) or any(not isinstance(row, dict)
            or row.get("scenario_id") not in selection or type(row.get("round")) is not int
            or not 1 <= row["round"] <= rounds or not isinstance(row.get("outcome"), str) for row in rows):
        raise ValueError("unexpected pipeline observation or round")
    for row in rows:
        for stage in ("asr", "translation", "tts", "roundtrip_asr"):
            if stage in row and (not isinstance(row[stage], dict) or type(row[stage].get("ok")) is not bool):
                raise ValueError(f"malformed {stage} receipt")
        for stage, key in (("asr", "transcript_text"), ("roundtrip_asr", "transcript_text"), ("translation", "translated_text")):
            if stage in row and not isinstance(row[stage].get(key, ""), str):
                raise ValueError(f"malformed {stage} text")
        expected = report.get("expected_segments", {}).get(row["scenario_id"], [])
        if not isinstance(expected, list) or any(not isinstance(s, dict) or not isinstance(s.get("file"), str)
                or not isinstance(s.get("sha256"), str) for s in expected):
            raise ValueError("malformed expected segment identity")
        if "segment" in row and (not isinstance(row["segment"], dict) or not isinstance(row["segment"].get("file"), str)):
            raise ValueError("malformed observed segment identity")
    return grade_pipeline(report, cases, set(selection), rounds, contracts, evaluator)


