from __future__ import annotations

import argparse
import hashlib
import json
import re
import unicodedata
from collections import defaultdict
from pathlib import Path

SCHEMA = "translateit.translation_quality.v1"
MAX_RESULT_BUNDLE_BYTES = 16 * 1024 * 1024
MAX_RESULT_ROWS = 1024
MAX_RESULT_TEXT_CHARS = 4_000  # Current canonical worker output ceiling.


def normalize(text: str) -> str:
    text = unicodedata.normalize("NFKC", text).casefold()
    text = re.sub(r"\s+", " ", text).strip()
    return text


def ngrams(text: str, n: int) -> dict[str, int]:
    compact = normalize(text)
    if len(compact) < n:
        return {compact: 1} if compact else {}
    counts: dict[str, int] = {}
    for i in range(len(compact) - n + 1):
        token = compact[i : i + n]
        counts[token] = counts.get(token, 0) + 1
    return counts


def char_ngram_f1(candidate: str, reference: str, max_n: int = 6) -> float:
    scores: list[float] = []
    for n in range(1, max_n + 1):
        cand = ngrams(candidate, n)
        ref = ngrams(reference, n)
        if not cand or not ref:
            continue
        overlap = sum(min(count, ref.get(token, 0)) for token, count in cand.items())
        precision = overlap / sum(cand.values())
        recall = overlap / sum(ref.values())
        scores.append(0.0 if precision + recall == 0 else 2 * precision * recall / (precision + recall))
    return sum(scores) / len(scores) if scores else 0.0


def corpus_fingerprint(corpus: dict) -> str:
    canonical = json.dumps(
        corpus,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def load_corpus(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("schema") != SCHEMA:
        raise ValueError(f"unsupported corpus schema: {data.get('schema')!r}")
    cases = data.get("cases")
    if not isinstance(cases, list) or not cases:
        raise ValueError("corpus must contain non-empty cases")
    seen: set[str] = set()
    for case in cases:
        case_id = str(case.get("id", "")).strip()
        if not case_id or case_id in seen:
            raise ValueError(f"invalid or duplicate case id: {case_id!r}")
        seen.add(case_id)
        if case.get("direction") not in {"id-en", "en-id"}:
            raise ValueError(f"{case_id}: unsupported direction")
        if not str(case.get("source", "")).strip():
            raise ValueError(f"{case_id}: empty source")
        refs = case.get("references")
        if not isinstance(refs, list) or not refs or not all(str(x).strip() for x in refs):
            raise ValueError(f"{case_id}: references required")
        for field in ("preserve", "required_any", "forbidden"):
            if field not in case:
                raise ValueError(f"{case_id}: missing {field}")
        strict = case.get("strict_preserve", [])
        if not isinstance(strict, list) or any(not isinstance(item, str) or item not in case["preserve"] for item in strict):
            raise ValueError(f"{case_id}: strict literals must be declared in preserve")
        exceptions = case.get("forbidden_exceptions", {})
        if not isinstance(exceptions, dict) or any(
            cue not in case["forbidden"] or not isinstance(phrases, list) or not phrases
            or any(not isinstance(phrase, str) or not normalize(cue)
                   or normalize(cue) not in normalize(phrase)
                   or normalize(cue) == normalize(phrase) for phrase in phrases)
            for cue, phrases in exceptions.items()
        ):
            raise ValueError(f"{case_id}: invalid forbidden phrase exceptions")
        risk_tags = case.get("risk_tags", [])
        ordered = case.get("ordered_any")
        if ordered is not None and (not isinstance(ordered, list) or not ordered or any(
            not isinstance(sequence, list) or len(sequence) < 2
            or not all(all(isinstance(option, str) and option.strip() for option in (cue if isinstance(cue, list) else [cue]))
                       and bool(cue) for cue in sequence)
            for sequence in ordered
        )):
            raise ValueError(f"{case_id}: invalid ordered critical cues")
        if (
            not isinstance(risk_tags, list)
            or not risk_tags
            or len(risk_tags) > 8
            or not all(isinstance(tag, str) and tag.strip() for tag in risk_tags)
        ):
            raise ValueError(f"{case_id}: risk_tags must contain one to eight non-empty strings")
        context_pairs = case.get("context_pairs")
        # Historic pairs are preserved for corpus identity only; they never reach MiLMMT.
        if context_pairs is not None:
            if case["direction"] != "id-en":
                raise ValueError(f"{case_id}: legacy context fixture must be outbound id-en")
            if (
                not isinstance(context_pairs, list)
                or not context_pairs
                or len(context_pairs) > 3
            ):
                raise ValueError(f"{case_id}: context_pairs must contain one to three pairs")
            for pair in context_pairs:
                if (
                    not isinstance(pair, list)
                    or len(pair) != 2
                    or not all(isinstance(value, str) and value.strip() for value in pair)
                ):
                    raise ValueError(f"{case_id}: invalid context pair")
    if "fast_suite" in data:
        select_suite(data, "fast")
    return data


def select_suite(corpus: dict, name: str) -> dict:
    if name == "full":
        return corpus
    if name != "fast":
        raise ValueError(f"unknown quality suite: {name}")
    spec = corpus.get("fast_suite", {})
    if not isinstance(spec, dict):
        raise ValueError("invalid fast quality suite: object required")
    reasons = spec.get("cases", {})
    risks = spec.get("required_risk_tags", [])
    known = {case["id"] for case in corpus["cases"]}
    if (not isinstance(reasons, dict) or not reasons or not set(reasons) < known
            or any(not isinstance(reason, str) or not reason.strip() for reason in reasons.values())
            or not isinstance(risks, list) or not risks
            or any(not isinstance(tag, str) or not tag.strip() for tag in risks)
            or len(risks) != len(set(risks))):
        raise ValueError("invalid fast quality suite: declared cases, reasons and risks required")
    cases = [case for case in corpus["cases"] if case["id"] in reasons]
    for direction in ("id-en", "en-id"):
        covered = {tag for case in cases if case["direction"] == direction for tag in case["risk_tags"]}
        if missing := set(risks) - covered:
            raise ValueError(f"fast quality suite lacks {direction} risks: {sorted(missing)}")
    return {
        **{key: value for key, value in corpus.items() if key != "fast_suite"},
        "cases": cases,
        "selection": {"name": "fast", "full_corpus_fingerprint": corpus_fingerprint(corpus),
                      "full_case_count": len(corpus["cases"]), "required_risk_tags": risks,
                      "case_reasons": reasons},
    }


_POLARITY_TERMS = {
    "not", "no", "never", "cannot", "can't", "won't", "don't",
    "didn't", "doesn't", "isn't", "aren't", "tidak", "bukan", "jangan", "belum", "tak",
}


def declared_cue_present(normalized_text: str, cue: str) -> bool:
    """Avoid false critical passes; preserve deliberate substring stem checks."""
    needle = normalize(cue)
    if not needle:
        return False
    offset = 0
    while (index := normalized_text.find(needle, offset)) >= 0:
        end = index + len(needle)
        left = normalized_text[index - 1] if index else ""
        right = normalized_text[end] if end < len(normalized_text) else ""
        embedded_number = (
            (needle[0].isdecimal() and left.isdecimal())
            or (needle[-1].isdecimal() and right.isdecimal())
        )
        embedded_polarity = needle in _POLARITY_TERMS and (
            (left and (left.isalnum() or left == "_"))
            or (right and (right.isalnum() or right == "_"))
        )
        if not embedded_number and not embedded_polarity:
            return True
        offset = index + 1
    return False


def ordered_cue_pattern(cue: str | list[str]) -> str:
    options = cue if isinstance(cue, list) else [cue]
    unsigned = r"(?<!minus )(?<!negative )(?<!plus )(?<!positive )" if any(option[-1].isdecimal() for option in options) else ""
    return unsigned + "(?:" + "|".join(
        (r"(?<![\w.,+-])" if option[-1].isdecimal() else r"(?<!\w)")
        + re.escape(re.sub(r"[^\w\s.,+-]", " ", re.sub(r"(?<=[a-z])-(?=[a-z])", " ", normalize(option))))
        + (r"(?!\w|[.,][0-9])" if option[-1].isdecimal() else r"(?!\w)")
        for option in options
    ) + ")"


def invariant_result(case: dict, translated: str) -> dict:
    norm = normalize(translated)
    missing_literals = [item for item in case["preserve"] if not declared_cue_present(norm, item)]
    missing_concepts = [
        group for group in case["required_any"]
        if not any(declared_cue_present(norm, option) for option in group)
    ]
    forbidden_hits = []
    for item in case["forbidden"]:
        remaining = norm
        for phrase in case.get("forbidden_exceptions", {}).get(item, []):
            remaining = re.sub(r"(?<!\w)" + re.escape(normalize(phrase)) + r"(?!\w)", " ", remaining)
        if declared_cue_present(remaining, item):
            forbidden_hits.append(item)
    ordered_text = re.sub(r"(?<=[a-z])-(?=[a-z])", " ", norm.replace("−", "-"))
    ordered_text = re.sub(r"[^\w\s.,+-]", " ", ordered_text)
    ordered_text = re.sub(r"\s+", " ", ordered_text)
    ordered_match = True
    if case.get("ordered_any"):
        ordered_match = any(re.search(
            r".*?".join(ordered_cue_pattern(cue) for cue in sequence), ordered_text
        ) is not None for sequence in case["ordered_any"])
    passed = bool(norm) and not missing_literals and not missing_concepts and not forbidden_hits and ordered_match
    strict_missing = any(item in case.get("strict_preserve", []) for item in missing_literals)
    quality_status = "FAIL" if not norm or forbidden_hits or strict_missing else "PASS" if passed else "REVIEW"
    return {
        "quality_status": quality_status,
        "missing_literals": missing_literals,
        "missing_concepts": missing_concepts,
        "forbidden_hits": forbidden_hits,
        "ordered_match": ordered_match,
        "critical_issue_types": [
            name for name, present in (
                ("literal_omission", bool(missing_literals)),
                ("meaning_cue_omission", bool(missing_concepts)),
                ("forbidden_meaning", bool(forbidden_hits)),
                ("critical_order", not ordered_match),
            )
            if present
        ],
        "critical_pass": passed,
    }


def validate_corpus(corpus: dict) -> dict:
    failures: list[dict] = []
    for case in corpus["cases"]:
        if not any(invariant_result(case, ref)["critical_pass"] for ref in case["references"]):
            failures.append({"case_id": case["id"], "reason": "no reference satisfies declared invariants"})
    directions = sorted({case["direction"] for case in corpus["cases"]})
    categories = sorted({case["category"] for case in corpus["cases"]})
    risk_tags = sorted({tag for case in corpus["cases"] for tag in case.get("risk_tags", [])})
    return {
        "ok": not failures,
        "case_count": len(corpus["cases"]),
        "directions": directions,
        "categories": categories,
        "risk_tags": risk_tags,
        "failures": failures,
    }


def corpus_stats(corpus: dict) -> dict:
    directions: dict[str, int] = defaultdict(int)
    categories: dict[str, int] = defaultdict(int)
    risk_tags: dict[str, int] = defaultdict(int)
    legacy_context_fixtures = 0
    for case in corpus["cases"]:
        directions[case["direction"]] += 1
        categories[case["category"]] += 1
        for tag in case.get("risk_tags", []):
            risk_tags[tag] += 1
        if case.get("context_pairs"):
            legacy_context_fixtures += 1
    return {
        "schema": "translateit.translation_quality.stats.v1",
        "selection": corpus.get("selection"),
        "corpus_fingerprint": corpus_fingerprint(corpus),
        "case_count": len(corpus["cases"]),
        "legacy_context_fixture_count": legacy_context_fixtures,
        "directions": dict(sorted(directions.items())),
        "categories": dict(sorted(categories.items())),
        "risk_tags": dict(sorted(risk_tags.items())),
    }


def load_result_bundle(path: Path) -> dict:
    # A captured report is untrusted input, even when it came from local tooling.
    if path.is_symlink() or not path.is_file():
        raise ValueError("result bundle must be a regular non-symlink file")
    with path.open("rb") as stream:
        raw_bytes = stream.read(MAX_RESULT_BUNDLE_BYTES + 1)
    if not raw_bytes:
        raise ValueError("result bundle is empty")
    if len(raw_bytes) > MAX_RESULT_BUNDLE_BYTES:
        raise ValueError("result bundle exceeds size limit")
    raw = json.loads(raw_bytes.decode("utf-8"))
    if not isinstance(raw, (list, dict)):
        raise ValueError("result bundle must be an object or list")
    if isinstance(raw, dict):
        schema = raw.get("schema")
        if schema is not None and schema != "translateit.translation_quality.worker_run.v1":
            raise ValueError("unsupported result bundle schema")
        if schema == "translateit.translation_quality.worker_run.v1" and raw.get("execution_complete") is not True:
            raise ValueError("incomplete worker execution cannot be used as a quality result bundle")
        for field in ("corpus_fingerprint", "source_identity"):
            if field in raw and not isinstance(raw[field], str):
                raise ValueError(f"result {field} must be a string")
    rows = raw.get("results") if isinstance(raw, dict) else raw
    if not isinstance(rows, list):
        raise ValueError("results must be a list or an object containing results")
    if len(rows) > MAX_RESULT_ROWS:
        raise ValueError("too many result rows")
    output: dict[str, str] = {}
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError("each result row must be an object")
        case_id = row.get("case_id")
        translated = row.get("translated_text")
        if not isinstance(case_id, str) or not case_id.strip():
            raise ValueError("result case_id must be a nonempty string")
        case_id = case_id.strip()
        if case_id in output:
            raise ValueError(f"duplicate result case_id: {case_id!r}")
        if not isinstance(translated, str):
            raise ValueError(f"{case_id}: translated_text must be a string")
        if len(translated) > MAX_RESULT_TEXT_CHARS:
            raise ValueError(f"{case_id}: translated_text exceeds size limit")
        # Empty text is retained for normal FAIL grading; never synthesize "None".
        output[case_id] = translated.strip()
    return {
        "results": output,
        "result_file_sha256": hashlib.sha256(raw_bytes).hexdigest(),
        "corpus_fingerprint": (raw.get("corpus_fingerprint", "").strip() if isinstance(raw, dict) else ""),
        "source_identity": (raw.get("source_identity", "").strip() if isinstance(raw, dict) else ""),
    }

def load_results(path: Path) -> dict[str, str]:
    return load_result_bundle(path)["results"]


def evaluate(
    corpus: dict,
    results: dict[str, str],
    *,
    result_corpus_fingerprint: str = "",
    source_identity: str = "",
) -> dict:
    expected = {case["id"] for case in corpus["cases"]}
    missing = sorted(expected - results.keys())
    unknown = sorted(results.keys() - expected)
    rows = []
    grouped: dict[str, list[float]] = defaultdict(list)
    grouped_critical: dict[str, list[bool]] = defaultdict(list)
    risk_grouped_critical: dict[str, list[bool]] = defaultdict(list)
    critical_failures = 0
    for case in corpus["cases"]:
        translated = results.get(case["id"], "")
        inv = invariant_result(case, translated)
        if case["id"] not in results:
            inv["quality_status"] = "UNVERIFIED"
        score = max((char_ngram_f1(translated, ref) for ref in case["references"]), default=0.0)
        if not inv["critical_pass"]:
            critical_failures += 1
        grouped[case["direction"]].append(score)
        grouped[case["category"]].append(score)
        grouped_critical[case["direction"]].append(inv["critical_pass"])
        grouped_critical[case["category"]].append(inv["critical_pass"])
        for tag in case.get("risk_tags", []):
            risk_grouped_critical[tag].append(inv["critical_pass"])
        rows.append({
            "case_id": case["id"],
            "direction": case["direction"],
            "category": case["category"],
            "risk_tags": list(case.get("risk_tags", [])),
            "char_ngram_f1": round(score, 4),
            **inv,
        })
    expected_fingerprint = corpus_fingerprint(corpus)
    provenance_matches = (
        not result_corpus_fingerprint
        or result_corpus_fingerprint == expected_fingerprint
    )
    complete = not missing and not unknown and provenance_matches
    quality_counts = {state: sum(row["quality_status"] == state for row in rows)
                      for state in ("PASS", "REVIEW", "FAIL", "UNVERIFIED")}
    verdict = "UNVERIFIED" if not complete else "FAIL" if quality_counts["FAIL"] else "REVIEW" if quality_counts["REVIEW"] else "PASS"
    return {
        "schema": "translateit.translation_quality.report.v1",
        "selection": corpus.get("selection"),
        "corpus_fingerprint": expected_fingerprint,
        "result_corpus_fingerprint": result_corpus_fingerprint or None,
        "source_identity": source_identity or None,
        "provenance_matches_corpus": provenance_matches,
        "complete_result_set": complete,
        "quality_counts": quality_counts,
        "verdict": verdict,
        "missing_case_ids": missing,
        "unknown_case_ids": unknown,
        "critical_failures": critical_failures,
        "critical_pass_rate": round((len(rows) - critical_failures) / len(rows), 4) if rows else 0.0,
        "mean_char_ngram_f1": round(sum(row["char_ngram_f1"] for row in rows) / len(rows), 4) if rows else 0.0,
        "group_means": {
            key: round(sum(values) / len(values), 4)
            for key, values in sorted(grouped.items())
        },
        "group_critical_pass_rates": {
            key: round(sum(values) / len(values), 4)
            for key, values in sorted(grouped_critical.items())
        },
        "risk_tag_critical_pass_rates": {
            key: round(sum(values) / len(values), 4)
            for key, values in sorted(risk_grouped_critical.items())
        },
        "cases": rows,
        "note": "Similarity is informational. Missing lexical cues require REVIEW; declared contrary patterns or missing strict literals FAIL. Legacy critical_failures counts unmatched checks, not confirmed meaning errors. PASS is limited to declared checks, not general semantic proof.",
    }


def compare_reports(
    corpus: dict,
    baseline_results: dict[str, str],
    candidate_results: dict[str, str],
    *,
    baseline_corpus_fingerprint: str = "",
    candidate_corpus_fingerprint: str = "",
    baseline_source_identity: str = "",
    candidate_source_identity: str = "",
) -> dict:
    baseline = evaluate(
        corpus,
        baseline_results,
        result_corpus_fingerprint=baseline_corpus_fingerprint,
        source_identity=baseline_source_identity,
    )
    candidate = evaluate(
        corpus,
        candidate_results,
        result_corpus_fingerprint=candidate_corpus_fingerprint,
        source_identity=candidate_source_identity,
    )
    baseline_cases = {row["case_id"]: row for row in baseline["cases"]}
    candidate_cases = {row["case_id"]: row for row in candidate["cases"]}

    critical_regressions = []
    critical_regression_details = {}
    critical_recoveries = []
    score_deltas = {}
    regressions_by_risk_tag: dict[str, list[str]] = defaultdict(list)
    for case_id in sorted(baseline_cases):
        base_row = baseline_cases[case_id]
        cand_row = candidate_cases[case_id]
        # Compare each declared failure: an already-failed case can still lose
        # another fact or acquire a new forbidden meaning.
        added_failures = {
            field: [item for item in cand_row[field] if item not in base_row[field]]
            for field in ("missing_literals", "missing_concepts", "forbidden_hits")
        }
        order_regressed = base_row["ordered_match"] and not cand_row["ordered_match"]
        if any(added_failures.values()) or order_regressed:
            critical_regressions.append(case_id)
            critical_regression_details[case_id] = {
                **added_failures,
                "order_regressed": order_regressed,
            }
            for tag in cand_row["risk_tags"]:
                regressions_by_risk_tag[tag].append(case_id)
        elif not base_row["critical_pass"] and cand_row["critical_pass"]:
            critical_recoveries.append(case_id)
        score_deltas[case_id] = round(
            cand_row["char_ngram_f1"] - base_row["char_ngram_f1"],
            4,
        )

    group_keys = sorted(set(baseline["group_means"]) | set(candidate["group_means"]))
    group_mean_deltas = {
        key: round(
            candidate["group_means"].get(key, 0.0) - baseline["group_means"].get(key, 0.0),
            4,
        )
        for key in group_keys
    }
    critical_group_keys = sorted(
        set(baseline["group_critical_pass_rates"])
        | set(candidate["group_critical_pass_rates"])
    )
    group_critical_pass_rate_deltas = {
        key: round(
            candidate["group_critical_pass_rates"].get(key, 0.0)
            - baseline["group_critical_pass_rates"].get(key, 0.0),
            4,
        )
        for key in critical_group_keys
    }
    risk_keys = sorted(
        set(baseline["risk_tag_critical_pass_rates"])
        | set(candidate["risk_tag_critical_pass_rates"])
    )
    risk_tag_critical_pass_rate_deltas = {
        key: round(
            candidate["risk_tag_critical_pass_rates"].get(key, 0.0)
            - baseline["risk_tag_critical_pass_rates"].get(key, 0.0),
            4,
        )
        for key in risk_keys
    }
    # Legacy result bundles without request fingerprints remain gradeable, but
    # cannot prove the two result sets belong to this exact regression corpus.
    promotion_provenance_complete = (
        bool(baseline["source_identity"])
        and bool(candidate["source_identity"])
        and bool(baseline["result_corpus_fingerprint"])
        and bool(candidate["result_corpus_fingerprint"])
        and baseline["provenance_matches_corpus"]
        and candidate["provenance_matches_corpus"]
    )

    return {
        "schema": "translateit.translation_quality.comparison.v1",
        "selection": corpus.get("selection"),
        "full_regression_evaluated": not bool(corpus.get("selection")),
        "complete_result_sets": (
            baseline["complete_result_set"] and candidate["complete_result_set"]
        ),
        "corpus_fingerprint": corpus_fingerprint(corpus),
        "baseline_source_identity": baseline["source_identity"],
        "candidate_source_identity": candidate["source_identity"],
        "baseline_provenance_matches_corpus": baseline["provenance_matches_corpus"],
        "candidate_provenance_matches_corpus": candidate["provenance_matches_corpus"],
        "baseline_critical_failures": baseline["critical_failures"],
        "candidate_critical_failures": candidate["critical_failures"],
        "candidate_quality_counts": candidate["quality_counts"],
        "critical_regressions": critical_regressions,
        "critical_regression_details": critical_regression_details,
        "critical_recoveries": critical_recoveries,
        "critical_regressions_by_risk_tag": dict(sorted(regressions_by_risk_tag.items())),
        "mean_char_ngram_f1_delta": round(
            candidate["mean_char_ngram_f1"] - baseline["mean_char_ngram_f1"],
            4,
        ),
        "group_mean_deltas": group_mean_deltas,
        "group_critical_pass_rate_deltas": group_critical_pass_rate_deltas,
        "risk_tag_critical_pass_rate_deltas": risk_tag_critical_pass_rate_deltas,
        "case_score_deltas": score_deltas,
        "promotion_provenance_complete": promotion_provenance_complete,
        "promotion_safe_on_declared_critical_invariants": (
            not corpus.get("selection")
            and baseline["complete_result_set"]
            and candidate["complete_result_set"]
            and promotion_provenance_complete
            and candidate["verdict"] == "PASS"
            and not critical_regressions
        ),
        "note": (
            "Comparison detects newly introduced declared-invariant failures, including within already-failed cases. "
            "It does not prove semantic superiority or replace human review."
        ),
    }


def regrade_retained(original_corpus: dict, current_corpus: dict, bundle: dict) -> dict:
    """Regrade identical requests, never certify new model execution or release."""
    original_fingerprint = corpus_fingerprint(original_corpus)
    current_fingerprint = corpus_fingerprint(current_corpus)
    if not bundle.get("corpus_fingerprint") or bundle["corpus_fingerprint"] != original_fingerprint:
        raise ValueError("retained result corpus fingerprint does not match original")
    if not bundle.get("source_identity"):
        raise ValueError("retained result source identity is required")
    if not re.fullmatch(r"[0-9a-f]{64}", str(bundle.get("result_file_sha256", ""))):
        raise ValueError("retained result file digest is required")
    if not validate_corpus(original_corpus)["ok"] or not validate_corpus(current_corpus)["ok"]:
        raise ValueError("retained regrade requires valid original and current corpora")

    original_requests = emit_requests(original_corpus)["requests"]
    current_requests = emit_requests(current_corpus)["requests"]
    if original_requests != current_requests:
        raise ValueError("retained regrade worker requests differ")
    case_ids = [item["case_id"] for item in current_requests]
    if len(bundle["results"]) != len(case_ids) or set(bundle["results"]) != set(case_ids):
        raise ValueError("retained regrade requires the complete exact result set")

    before = evaluate(
        original_corpus, bundle["results"],
        result_corpus_fingerprint=original_fingerprint,
        source_identity=bundle["source_identity"],
    )
    # Preserve the ORIGINAL fingerprint when grading against changed contracts.
    # The current-corpus evaluation therefore cannot masquerade as a fresh match.
    after = evaluate(
        current_corpus, bundle["results"],
        result_corpus_fingerprint=original_fingerprint,
        source_identity=bundle["source_identity"],
    )
    before_rows = {row["case_id"]: row for row in before["cases"]}
    after_rows = {row["case_id"]: row for row in after["cases"]}
    diagnostic_fields = (
        "quality_status", "critical_pass", "missing_literals", "missing_concepts",
        "forbidden_hits", "ordered_match", "char_ngram_f1",
    )
    changed_cases = []
    for case_id in case_ids:
        older = {field: before_rows[case_id][field] for field in diagnostic_fields}
        newer = {field: after_rows[case_id][field] for field in diagnostic_fields}
        if older != newer:
            changed_cases.append({"case_id": case_id, "before": older, "after": newer})

    request_bytes = json.dumps(
        original_requests, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return {
        "schema": "translateit.translation_quality.retained_regrade.v1",
        "source_identity": bundle["source_identity"],
        "retained_result_sha256": bundle["result_file_sha256"],
        "original_corpus_fingerprint": original_fingerprint,
        "current_corpus_fingerprint": current_fingerprint,
        "identical_worker_requests_sha256": hashlib.sha256(request_bytes).hexdigest(),
        "request_equivalence": "EXACT_DECLARED_REQUESTS",
        "case_count": len(case_ids),
        "before_quality_counts": before["quality_counts"],
        "after_quality_counts": after["quality_counts"],
        "changed_cases": changed_cases,
        "new_model_inference_executed": False,
        "release_promotion_eligible": False,
        "note": (
            "Regraded retained output against revised declared checks only. "
            "Matching source requests do not independently authenticate historical "
            "model execution, prove general semantic fidelity, or allow promotion."
        ),
    }

def emit_requests(corpus: dict) -> dict:
    requests = []
    for case in corpus["cases"]:
        source_language, target_language = case["direction"].split("-")
        request = {
            "command": "translate",
            "request_kind": "standalone_text",
            "text": case["source"],
            "source_language": source_language,
            "target_language": target_language,
        }
        # Historic cases use direct Meeting-equivalent final-utterance translation.
        # The original context pairs stay in the corpus, never in requests.
        if case.get("context_pairs"):
            request.update(
                {
                    "request_kind": "meeting_finalized_utterance",
                    "meeting_lane": "you",
                    "meeting_session_id": "translation-quality-eval",
                    "meeting_generation": 1,
                }
            )
        requests.append({"case_id": case["id"], "request": request})
    return {
        "schema": "translateit.translation_quality.requests.v1",
        "selection": corpus.get("selection"),
        "corpus_fingerprint": corpus_fingerprint(corpus),
        "requests": requests,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command",
        choices=("validate-corpus", "stats", "emit-requests", "evaluate", "compare", "regrade-retained"),
    )
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--original-corpus", type=Path)
    parser.add_argument("--suite", choices=("full", "fast"), default="full",
                        help="fast is diagnostic only; full is required for promotion comparison")
    parser.add_argument("--results", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    args = parser.parse_args()
    corpus = select_suite(load_corpus(args.corpus), args.suite)
    if args.command == "validate-corpus":
        report = validate_corpus(corpus)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0 if report["ok"] else 1
    if args.command == "stats":
        print(json.dumps(corpus_stats(corpus), ensure_ascii=False, indent=2))
        return 0
    if args.command == "emit-requests":
        print(json.dumps(emit_requests(corpus), ensure_ascii=False, indent=2))
        return 0
    if args.command == "regrade-retained":
        if args.original_corpus is None or args.results is None:
            parser.error("--original-corpus and --results required for retained regrading")
        try:
            original = select_suite(load_corpus(args.original_corpus), args.suite)
            bundle = load_result_bundle(args.results)
            report = regrade_retained(original, corpus, bundle)
        except OSError:
            print(json.dumps({"ok": False, "blocker": "retained_result_unreadable"}, indent=2))
            return 1
        except ValueError as error:
            print(json.dumps({"ok": False, "blocker": str(error)}, ensure_ascii=False, indent=2))
            return 1
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0
    if args.command == "compare":
        if args.baseline is None or args.candidate is None:
            parser.error("--baseline and --candidate are required for compare")
        baseline = load_result_bundle(args.baseline)
        candidate = load_result_bundle(args.candidate)
        report = compare_reports(
            corpus,
            baseline["results"],
            candidate["results"],
            baseline_corpus_fingerprint=baseline["corpus_fingerprint"],
            candidate_corpus_fingerprint=candidate["corpus_fingerprint"],
            baseline_source_identity=baseline["source_identity"],
            candidate_source_identity=candidate["source_identity"],
        )
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0 if report["promotion_safe_on_declared_critical_invariants"] else 1

    if args.results is None:
        parser.error("--results is required for evaluate")
    result_bundle = load_result_bundle(args.results)
    report = evaluate(
        corpus,
        result_bundle["results"],
        result_corpus_fingerprint=result_bundle["corpus_fingerprint"],
        source_identity=result_bundle["source_identity"],
    )
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["verdict"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
