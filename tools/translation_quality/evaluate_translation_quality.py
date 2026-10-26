from __future__ import annotations

import argparse
import hashlib
import json
import re
import unicodedata
from collections import defaultdict
from pathlib import Path

SCHEMA = "translateit.translation_quality.v1"


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
    return data


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
    unsigned = r"(?<!minus )(?<!negative )(?<!plus )(?<!positive )" if any(option.isdecimal() for option in options) else ""
    return unsigned + "(?:" + "|".join(
        (r"(?<![\w.+-])" if option[0].isdecimal() else r"(?<!\w)")
        + re.escape(re.sub(r"[^\w\s]", " ", normalize(option)))
        + (r"(?!\w|\.[0-9])" if option[-1].isdecimal() else r"(?!\w)")
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
    ordered_text = re.sub(r"[^\w\s.+-]", " ", ordered_text)
    ordered_text = re.sub(r"\s+", " ", ordered_text)
    ordered_match = True
    if case.get("ordered_any"):
        ordered_match = any(re.search(
            r".*?".join(ordered_cue_pattern(cue) for cue in sequence), ordered_text
        ) is not None for sequence in case["ordered_any"])
    return {
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
        "critical_pass": not missing_literals and not missing_concepts and not forbidden_hits and ordered_match,
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
        "corpus_fingerprint": corpus_fingerprint(corpus),
        "case_count": len(corpus["cases"]),
        "legacy_context_fixture_count": legacy_context_fixtures,
        "directions": dict(sorted(directions.items())),
        "categories": dict(sorted(categories.items())),
        "risk_tags": dict(sorted(risk_tags.items())),
    }


def load_result_bundle(path: Path) -> dict:
    raw = json.loads(path.read_text(encoding="utf-8"))
    rows = raw.get("results") if isinstance(raw, dict) else raw
    if not isinstance(rows, list):
        raise ValueError("results must be a list or an object containing results")
    output: dict[str, str] = {}
    for row in rows:
        case_id = str(row.get("case_id", "")).strip()
        translated = str(row.get("translated_text", "")).strip()
        if not case_id or case_id in output:
            raise ValueError(f"invalid or duplicate result case_id: {case_id!r}")
        output[case_id] = translated
    return {
        "results": output,
        "corpus_fingerprint": (
            str(raw.get("corpus_fingerprint", "")).strip()
            if isinstance(raw, dict)
            else ""
        ),
        "source_identity": (
            str(raw.get("source_identity", "")).strip()
            if isinstance(raw, dict)
            else ""
        ),
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
    return {
        "schema": "translateit.translation_quality.report.v1",
        "corpus_fingerprint": expected_fingerprint,
        "result_corpus_fingerprint": result_corpus_fingerprint or None,
        "source_identity": source_identity or None,
        "provenance_matches_corpus": provenance_matches,
        "complete_result_set": not missing and not unknown and provenance_matches,
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
        "note": "Character n-gram F1 and declared invariants are regression signals, not standalone proof of translation quality.",
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
    return {
        "schema": "translateit.translation_quality.comparison.v1",
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
        "promotion_provenance_complete": (
            bool(baseline["source_identity"])
            and bool(candidate["source_identity"])
            and baseline["provenance_matches_corpus"]
            and candidate["provenance_matches_corpus"]
        ),
        "promotion_safe_on_declared_critical_invariants": (
            baseline["complete_result_set"]
            and candidate["complete_result_set"]
            and bool(baseline["source_identity"])
            and bool(candidate["source_identity"])
            and baseline["provenance_matches_corpus"]
            and candidate["provenance_matches_corpus"]
            and not critical_regressions
        ),
        "note": (
            "Comparison detects newly introduced declared-invariant failures, including within already-failed cases. "
            "It does not prove semantic superiority or replace human review."
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
        "corpus_fingerprint": corpus_fingerprint(corpus),
        "requests": requests,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command",
        choices=("validate-corpus", "stats", "emit-requests", "evaluate", "compare"),
    )
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--results", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    args = parser.parse_args()
    corpus = load_corpus(args.corpus)
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
    return 0 if report["complete_result_set"] and report["critical_failures"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
