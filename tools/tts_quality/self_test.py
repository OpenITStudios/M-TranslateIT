from __future__ import annotations

import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EVALUATOR = ROOT / "evaluate_tts_quality.py"
CORPUS = ROOT / "corpus" / "tts_quality_v1.json"


def load_evaluator():
    spec = importlib.util.spec_from_file_location("translateit_tts_quality_evaluator", EVALUATOR)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def evidence(corpus: dict, similarity: float = 0.9) -> dict:
    return {
        case["id"]: {
            "intelligibility_text": case["text"],
            "speaker_similarity": similarity,
            "artifact_flags": [],
            "wav_sha256": "a" * 64,
        }
        for case in corpus["cases"]
    }


def main() -> int:
    evaluator = load_evaluator()
    corpus = evaluator.load_corpus(CORPUS)
    validation = evaluator.validate_corpus(corpus)
    assert validation["ok"], validation
    assert validation["case_count"] == 30
    aliases = [['15', 'fifteen'], ['50', 'fifty']]
    assert evaluator.word_error_rate('I mean fifteen, not 50.', 'I mean 15, not fifty.', number_aliases=aliases) == 0
    assert evaluator.word_error_rate('I mean 15, not 50.', 'I mean 50, not 15.', number_aliases=aliases) > 0
    assert evaluator.word_error_rate('125', '125th', number_aliases=[['125', 'one hundred and twenty-five']]) > 0
    assert evaluator.word_error_rate('15', '-15', number_aliases=aliases) > 0
    assert {
        "meeting_core",
        "technical",
        "long_form",
        "prosody_challenge",
        "questions",
        "acronyms",
        "lists",
        "repetition_stability",
    }.issubset(set(validation["categories"]))

    fingerprint = evaluator.corpus_fingerprint(corpus)
    perfect = evidence(corpus)
    report = evaluator.evaluate(
        corpus,
        perfect,
        result_corpus_fingerprint=fingerprint,
        source_identity="baseline-sha",
    )
    assert report["complete_result_set"] is True
    assert report["critical_failures"] == 0
    assert report["mean_intelligibility_wer"] == 0.0
    assert report["mean_speaker_similarity"] == 0.9
    assert report["native_timing"] is None  # historical bundles stay valid
    for bad, expected in [("capital", "API"), ("191320", "19132"),
                          ("unconfirmed", "confirm"), ("different", "if"),
                          ("1.21.4.5", "1.21.4"), ("-19132", "19132")]:
        assert not evaluator.contains_critical_phrase(evaluator.critical_tokens(bad), expected)
    for good, expected in [("The API, endpoint.", "API"), ("not yet!", "not yet"),
                           ("version 1.21.4.", "1.21.4"), ("eight forty-five", "eight forty five")]:
        assert evaluator.contains_critical_phrase(evaluator.critical_tokens(good), expected)
    timed = evidence(corpus)
    for index, case in enumerate(corpus["cases"], 1):
        timed[case["id"]].update(elapsed_ms=index * 100, sample_count=44_100)
    def timing(rows, identity="native-fixture", fp=fingerprint):
        return evaluator.evaluate(corpus, rows, result_corpus_fingerprint=fp,
                                  source_identity=identity)["native_timing"]
    assert timing(timed) == {
        "case_count": 30, "first_ms": 100, "p50_ms": 1500,
        "p95_ms": 2900, "max_ms": 3000,
        "audio_sample_count": 30 * 44_100, "aggregate_rtf": 1.55,
    }
    assert timing(timed, identity="") is None
    assert timing(timed, fp="") is None
    first = corpus["cases"][0]["id"]
    for field, value in [("elapsed_ms", None), ("elapsed_ms", True),
                         ("elapsed_ms", -1), ("elapsed_ms", 1.5),
                         ("sample_count", 0), ("sample_count", 44_100 * 60 + 1)]:
        invalid = {key: dict(row) for key, row in timed.items()}
        invalid[first][field] = value
        assert timing(invalid) is None
    incomplete = dict(timed)
    del incomplete[first]
    assert timing(incomplete) is None
    assert timing({**timed, "unknown": timed[first]}) is None

    # Complete rows and source identity cannot substitute for the corpus identity.
    missing_fingerprint = evaluator.evaluate(
        corpus,
        perfect,
        source_identity="baseline-sha",
    )
    assert missing_fingerprint["provenance_matches_corpus"] is False
    assert missing_fingerprint["complete_result_set"] is False
    incorrect_fingerprint = evaluator.evaluate(
        corpus,
        perfect,
        result_corpus_fingerprint="0" * 64,
        source_identity="baseline-sha",
    )
    assert incorrect_fingerprint["provenance_matches_corpus"] is False
    assert incorrect_fingerprint["complete_result_set"] is False

    healthy_candidate = evidence(corpus, 0.92)
    valid_comparison = evaluator.compare_reports(
        corpus,
        perfect,
        healthy_candidate,
        baseline_corpus_fingerprint=fingerprint,
        candidate_corpus_fingerprint=fingerprint,
        baseline_source_identity="baseline-sha",
        candidate_source_identity="candidate-sha",
    )
    assert valid_comparison["complete_result_sets"] is True
    assert valid_comparison["promotion_provenance_complete"] is True
    assert valid_comparison["promotion_safe_on_declared_critical_invariants"] is True
    assert valid_comparison["promotion_safe_on_quality_metrics"] is True

    def compare(base, cand):
        return evaluator.compare_reports(
            corpus, base, cand,
            baseline_corpus_fingerprint=fingerprint,
            candidate_corpus_fingerprint=fingerprint,
            baseline_source_identity="baseline-sha",
            candidate_source_identity="candidate-sha",
        )
    degraded = evidence(corpus, 0.5)
    assert compare(perfect, degraded)["promotion_safe_on_declared_critical_invariants"] is True
    assert compare(perfect, degraded)["promotion_safe_on_quality_metrics"] is False
    degraded = evidence(corpus)
    degraded["tts-meeting-001"]["intelligibility_text"] += " extra words"
    wer_regression = compare(perfect, degraded)
    assert wer_regression["promotion_safe_on_declared_critical_invariants"] is True
    assert wer_regression["promotion_safe_on_quality_metrics"] is False
    assert wer_regression["intelligibility_wer_regressions"] == ["tts-meeting-001"]
    base_artifact = evidence(corpus)
    more_artifacts = evidence(corpus)
    base_artifact["tts-meeting-001"]["artifact_flags"] = ["clipping"]
    more_artifacts["tts-meeting-001"]["artifact_flags"] = ["clipping", "dropout"]
    assert compare(base_artifact, more_artifacts)["artifact_regressions"] == ["tts-meeting-001"]
    # An already-failing case can still acquire a new missing invariant.
    base_bad = evidence(corpus)
    cand_bad = evidence(corpus)
    base_bad["tts-technical-001"]["intelligibility_text"] = "Do not change the API endpoint before the final review."
    cand_bad["tts-technical-001"]["intelligibility_text"] = "Do not change the endpoint before the final review."
    assert compare(base_bad, cand_bad)["critical_regressions"] == ["tts-technical-001"]

    # Even a perfect candidate may not be promoted without an exact fingerprint.
    for baseline_fingerprint, candidate_fingerprint in (
        ("", fingerprint),
        (fingerprint, ""),
        (fingerprint, "0" * 64),
    ):
        unproven_comparison = evaluator.compare_reports(
            corpus,
            perfect,
            healthy_candidate,
            baseline_corpus_fingerprint=baseline_fingerprint,
            candidate_corpus_fingerprint=candidate_fingerprint,
            baseline_source_identity="baseline-sha",
            candidate_source_identity="candidate-sha",
        )
        assert unproven_comparison["complete_result_sets"] is False
        assert unproven_comparison["promotion_provenance_complete"] is False
        assert unproven_comparison["promotion_safe_on_declared_critical_invariants"] is False

    candidate = evidence(corpus, 0.92)
    candidate["tts-negation-001"]["intelligibility_text"] = (
        "I approved the payment, so please send it."
    )
    candidate["tts-negation-001"]["artifact_flags"] = ["truncation"]
    comparison = evaluator.compare_reports(
        corpus,
        perfect,
        candidate,
        baseline_corpus_fingerprint=fingerprint,
        candidate_corpus_fingerprint=fingerprint,
        baseline_source_identity="baseline-sha",
        candidate_source_identity="candidate-sha",
    )
    assert comparison["mean_speaker_similarity_delta"] > 0
    assert comparison["critical_regressions"] == ["tts-negation-001"]
    assert comparison["artifact_regressions"] == ["tts-negation-001"]
    assert comparison["promotion_safe_on_declared_critical_invariants"] is False

    missing_provenance = evaluator.compare_reports(corpus, perfect, perfect)
    assert missing_provenance["promotion_provenance_complete"] is False
    assert missing_provenance["promotion_safe_on_declared_critical_invariants"] is False

    plan = evaluator.fixture_plan(corpus)
    assert plan["corpus_fingerprint"] == fingerprint
    assert len(plan["fixtures"]) == validation["case_count"]
    print(json.dumps({"ok": True, "case_count": validation["case_count"]}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
