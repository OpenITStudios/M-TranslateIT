from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
from tempfile import TemporaryDirectory

ROOT = Path(__file__).resolve().parent
EVALUATOR = ROOT / "evaluate_translation_quality.py"
CORPUS = ROOT / "corpus" / "translation_quality_v1.json"
HELDOUT = ROOT / "heldout" / "translation_quality_heldout_v1.json"


def load_evaluator():
    spec = importlib.util.spec_from_file_location("translateit_quality_evaluator", EVALUATOR)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    evaluator = load_evaluator()
    corpus = evaluator.load_corpus(CORPUS)
    validation = evaluator.validate_corpus(corpus)
    assert validation["ok"], validation
    cases_by_id = {case["id"]: case for case in corpus["cases"]}
    faithful_variants = {
        "en-id-entity-001": "Hantarkan laporan tersebut ke team@example.com dan tetap buka tiket MIV-204.",
        "en-id-code-switch-001": "Silakan periksa cabang sebelum Anda meng-push hotfix tersebut.",
        "id-en-disfluency-001": "So, uh, maybe we should hold off on this one; don't send it now.",
        "id-en-meet-015": "Can we get back to the second point for a moment?",
        "en-id-dis-011": "Jadi, um, mungkin sebaiknya tetap tidak berubah untuk saat ini.",
        "en-id-technical-001": "Janganlah membuat ulang lock file atau mengubah kontrak API.",
        "en-id-omit-011": "Mikrofon diperlukan; suara rapat opsional.",
    }
    for ident, translated in faithful_variants.items():
        result = evaluator.invariant_result(cases_by_id[ident], translated)
        assert result["quality_status"] == "PASS", (ident, result)
        assert evaluator.char_ngram_f1(translated, cases_by_id[ident]["references"][0]) < 1
    meaning_changes = {
        "en-id-technical-001": "Janganlah merestorasi file lock file atau mengubah kontrak API.",
        "id-en-unicode-001": "The file name is Plan—Final_v2.1 and the symbol is © 2026.",
        "id-en-negation-010": "I haven't signed the contract yet.",
        "en-id-meet-010": "Mari kita batalkan topik ini dan kembali membicarakannya nanti.",
        "en-id-quant-011": "Sebagian besar hanya ada lima item yang masih belum selesai diproses.",
        "en-id-omit-011": "Mikrofon opsional; suara rapat wajib.",
        "en-id-budget-purpose-001": "Anggaran pembangunannya adalah lima belas juta rupiah.",
    }
    for ident, translated in meaning_changes.items():
        assert evaluator.invariant_result(cases_by_id[ident], translated)["quality_status"] == "FAIL", ident
    ambiguous = evaluator.invariant_result(cases_by_id["en-id-omit-011"], "Mikrofon diperlukan; sambungan audio opsional.")
    assert ambiguous["quality_status"] == "REVIEW"
    assert ambiguous["missing_concepts"]  # accepting required/diperlukan must not erase the unresolved sound concept
    changed_entity = evaluator.invariant_result(cases_by_id["en-id-entity-001"], "Kirim laporan ke other@example.com dan biarkan tiket MIV-204 tetap terbuka.")
    assert changed_entity["quality_status"] == "FAIL"
    fast = evaluator.select_suite(corpus, "fast")
    assert len(fast["cases"]) < len(corpus["cases"])
    assert {"en-id-budget-purpose-001", "en-id-meet-010", "id-en-informal-workplace-005"}.issubset(
        {case["id"] for case in fast["cases"]}
    )
    assert evaluator.select_suite(corpus, "full") is corpus
    for invalid in (
        {**corpus, "fast_suite": []},
        {**corpus, "fast_suite": {**corpus["fast_suite"], "cases": {"unknown": "invalid case"}}},
        {**corpus, "fast_suite": {**corpus["fast_suite"], "required_risk_tags": ["uncovered"]}},
    ):
        try:
            evaluator.select_suite(invalid, "fast")
        except ValueError:
            pass
        else:
            raise AssertionError("accepted invalid fast selection")
    assert set(validation["directions"]) == {"en-id", "id-en"}
    assert {"meaning", "number", "negation", "omission", "hallucination", "entity", "terminology"}.issubset(
        set(validation["risk_tags"])
    )

    heldout = evaluator.load_corpus(HELDOUT)
    heldout_validation = evaluator.validate_corpus(heldout)
    assert heldout_validation["ok"], heldout_validation
    assert heldout_validation["case_count"] == 40
    assert set(heldout_validation["directions"]) == {"en-id", "id-en"}
    assert {
        "prompt_boundary",
        "unicode",
        "repetition",
        "input_normalization",
        "contextual_meeting",
        "modality_uncertainty",
        "conditional_meaning",
        "quantifier_scope",
        "spoken_disfluency",
    }.issubset(set(validation["categories"]))

    fingerprint = evaluator.corpus_fingerprint(corpus)
    assert len(fingerprint) == 64

    perfect = {case["id"]: case["references"][0] for case in corpus["cases"]}
    fast_perfect = {case["id"]: perfect[case["id"]] for case in fast["cases"]}
    fast_fingerprint = evaluator.corpus_fingerprint(fast)
    assert evaluator.evaluate(fast, fast_perfect, result_corpus_fingerprint=fast_fingerprint)["complete_result_set"]
    assert not evaluator.evaluate(corpus, fast_perfect)["complete_result_set"]
    assert not evaluator.evaluate(fast, perfect)["complete_result_set"]
    same_fast = evaluator.compare_reports(
        fast, fast_perfect, fast_perfect,
        baseline_corpus_fingerprint=fast_fingerprint, candidate_corpus_fingerprint=fast_fingerprint,
        baseline_source_identity="baseline-sha", candidate_source_identity="candidate-sha",
    )
    assert same_fast["complete_result_sets"]
    assert not same_fast["full_regression_evaluated"]
    assert not same_fast["promotion_safe_on_declared_critical_invariants"]
    bad_fast = {**fast_perfect, "en-id-budget-purpose-001": "Anggaran pembangunannya adalah lima belas juta rupiah."}
    bad_budget = next(row for row in evaluator.evaluate(fast, bad_fast)["cases"] if row["case_id"] == "en-id-budget-purpose-001")
    assert not bad_budget["critical_pass"]
    assert {item["case_id"]: item["request"] for item in evaluator.emit_requests(fast)["requests"]} == {
        item["case_id"]: item["request"] for item in evaluator.emit_requests(corpus)["requests"]
        if item["case_id"] in fast_perfect
    }
    report = evaluator.evaluate(corpus, perfect)
    assert report["complete_result_set"] is True
    assert report["critical_failures"] == 0
    assert report["critical_pass_rate"] == 1.0
    assert report["verdict"] == "PASS"
    assert report["quality_counts"]["REVIEW"] == report["quality_counts"]["FAIL"] == 0
    assert report["corpus_fingerprint"] == fingerprint
    assert report["provenance_matches_corpus"] is True
    assert report["group_critical_pass_rates"]["id-en"] == 1.0
    assert report["group_critical_pass_rates"]["contextual_meeting"] == 1.0
    assert report["risk_tag_critical_pass_rates"]["meaning"] == 1.0
    assert report["risk_tag_critical_pass_rates"]["number"] == 1.0
    unresolved = {**perfect, "id-en-correction-001": "I meant Thursday instead of Tuesday."}
    unresolved_comparison = evaluator.compare_reports(
        corpus, unresolved, unresolved,
        baseline_corpus_fingerprint=fingerprint, candidate_corpus_fingerprint=fingerprint,
        baseline_source_identity="baseline-sha", candidate_source_identity="candidate-sha",
    )
    assert not unresolved_comparison["critical_regressions"]
    assert unresolved_comparison["candidate_quality_counts"]["REVIEW"] == 1
    assert not unresolved_comparison["promotion_safe_on_declared_critical_invariants"]

    mismatched = evaluator.evaluate(
        corpus,
        perfect,
        result_corpus_fingerprint="0" * 64,
        source_identity="candidate-sha",
    )
    assert mismatched["provenance_matches_corpus"] is False
    assert mismatched["complete_result_set"] is False

    bad = dict(perfect)
    bad["id-en-negation-001"] = "I will attend the meeting tomorrow."
    failed = evaluator.evaluate(corpus, bad)
    target = next(case for case in failed["cases"] if case["case_id"] == "id-en-negation-001")
    assert target["critical_pass"] is False
    assert target["forbidden_hits"]
    assert target["critical_issue_types"] == ["meaning_cue_omission", "forbidden_meaning"] or target["critical_issue_types"] == ["forbidden_meaning"]
    assert evaluator.declared_cue_present("tahun 2021", "21") is False
    assert evaluator.declared_cue_present("tahun 21 september", "21") is True
    assert evaluator.declared_cue_present("notebook", "not") is False
    assert evaluator.declared_cue_present("not ready", "not") is True
    assert evaluator.declared_cue_present("tombolnya besar", "tombol") is True
    numeric_case = {"preserve": ["21"], "required_any": [["not"]], "forbidden": []}
    assert evaluator.invariant_result(numeric_case, "2021 notebook")["critical_pass"] is False
    assert evaluator.invariant_result(numeric_case, "21 not ready")["critical_pass"] is True

    money = next(case for case in corpus["cases"] if case["id"] == "id-en-number-001")
    for accepted, rejected in (("Rp2.750.000", "Rp2.570.000"), ("Rp2,750,000", "Rp2,570,000")):
        for valid in (f"The budget is {accepted}, not {rejected}.", f"Not {rejected}, but {accepted}."):
            assert evaluator.invariant_result(money, valid)["critical_pass"], valid
        for wrong in (f"The budget is {rejected}, not {accepted}.",
                      f"Not {accepted}, but {rejected}.",
                      f"The budget is -{accepted}, not {rejected}.",
                      f"The budget is minus {accepted}, not {rejected}.",
                      f"The budget is {accepted}5, not {rejected}.",
                      f"The budget is {accepted},50, not {rejected}.",
                      f"The budget is {accepted}.50, not {rejected}."):
            assert not evaluator.invariant_result(money, wrong)["critical_pass"], wrong

    # Decimal value and correction role cannot be inferred from matching
    # isolated numerals and negation words alone.
    conversion = cases_by_id["id-en-number-002"]
    for faithful in (
        "The conversion target is 12.5%, not 15%.",
        "The conversion target is 12,5%, not 15%.",
        "Not 15%, but 12.5% is the conversion target.",
    ):
        assert evaluator.invariant_result(conversion, faithful)["critical_pass"], faithful
    for changed_meaning in (
        "The conversion target is 12%, not 15%.",
        "The conversion target is 15%, not 12.5%.",
        "The conversion target is 12.5%, not 15.5%.",
    ):
        assert not evaluator.invariant_result(conversion, changed_meaning)["critical_pass"], changed_meaning

    file_size = cases_by_id["en-id-number-002"]
    for faithful in (
        "Ukuran filenya 2,4 GB, bukan 24 GB.",
        "Ukuran filenya 2.4 GB, bukan 24 GB.",
        "Bukan 24 GB, melainkan 2,4 GB ukuran filenya.",
    ):
        assert evaluator.invariant_result(file_size, faithful)["critical_pass"], faithful
    for reversed_meaning in (
        "Ukuran filenya 24 GB, bukan 2,4 GB.",
        "Bukan 2,4 GB, melainkan 24 GB ukuran filenya.",
    ):
        assert not evaluator.invariant_result(file_size, reversed_meaning)["critical_pass"], reversed_meaning

    # Preserve numeric roles and complete magnitudes, rather than accepting
    # disjoint numbers and a negation cue in any order.
    correction_roles = (
        (
            "id-en-num-011",
            ("The correct version is 2.10.7, not 2.1.7.",
             "Not 2.1.7; the correct version is 2.10.7."),
            ("The correct version is 2.1.7, not 2.10.7.",
             "Not 2.10.7; the correct version is 2.1.7."),
        ),
        (
            "en-id-num-011",
            ("Gunakan port 19132, bukan 19123.",
             "Bukan 19123, gunakan port 19132."),
            ("Gunakan port 19123, bukan 19132.",
             "Bukan 19132, gunakan port 19123."),
        ),
    )
    for case_id, faithful_outputs, reversed_outputs in correction_roles:
        for translated in faithful_outputs:
            assert evaluator.invariant_result(cases_by_id[case_id], translated)["critical_pass"], (case_id, translated)
        for translated in reversed_outputs:
            assert not evaluator.invariant_result(cases_by_id[case_id], translated)["critical_pass"], (case_id, translated)

    numeric_values = (
        (
            "en-id-number-001",
            ("Batas unggahnya adalah 250 MB per file.",
             "Batas per file adalah MB 250."),
            ("Batas unggahnya adalah 250.5 MB per file.",
             "Batas unggahnya adalah 250,5 MB per file.",
             "Batas unggahnya adalah -250 MB per file."),
        ),
        (
            "id-en-num-010",
            ("The total is 1,250 units with a tolerance of ±2%.",
             "At ±2% tolerance, the total is 1250 units."),
            ("The total is 1,250.5 units with a tolerance of ±2%.",
             "The total is 1,2500 units with a tolerance of ±2%.",
             "The total is -1250 units with a tolerance of ±2%."),
        ),
        (
            "en-id-num-014",
            ("Paket itu beratnya 2,75 kg.",
             "Paket itu memiliki berat 2.75 kg."),
            ("Paket itu beratnya 2,750 kg.",
             "Paket itu beratnya 12,75 kg.",
             "Paket itu beratnya -2,75 kg."),
        ),
    )
    for case_id, faithful_outputs, altered_values in numeric_values:
        for translated in faithful_outputs:
            assert evaluator.invariant_result(cases_by_id[case_id], translated)["critical_pass"], (case_id, translated)
        for translated in altered_values:
            assert not evaluator.invariant_result(cases_by_id[case_id], translated)["critical_pass"], (case_id, translated)

    # Complete numeric magnitudes, dates and start/end order matter more than
    # isolated tokens; faithful punctuation/word-order variants remain accepted.
    complete_numeric_cases = (
        (
            "id-en-num-014",
            ("The discount is 7.5% for 40 tickets.",
             "For 40 tickets, the discount is 7,5%."),
            ("The discount is 75% for 40 tickets.",
             "The discount is 7.5 for 40 tickets.",
             "The discount is -7.5% for 40 tickets."),
        ),
        (
            "en-id-num-010",
            ("Batas anggarannya adalah USD 18.500.",
             "Anggaran maksimal adalah 18500 USD."),
            ("Batas anggarannya adalah USD 18.",
             "Batas anggarannya adalah USD 18.500,5.",
             "Anggarannya USD 18.500."),
        ),
        (
            "id-en-date-001",
            ("The server will be offline on 21 September 2026 at 19:30 WIB.",
             "At 19.30 WIB on 21 September 2026, the server will shut down."),
            ("The server will be offline on 21 September 2026 at 19:31 WIB.",
             "The server will be offline on 21 October 2026 at 19:30 WIB."),
        ),
        (
            "en-id-date-001",
            ("Panggilan dimulai pukul 08:45 pada 3 Oktober 2026.",
             "Pada 3 Oktober 2026, panggilan dimulai 8.45."),
            ("Panggilan dimulai pukul 08:46 pada 3 Oktober 2026.",
             "Panggilan dimulai pukul 08:45 pada 3 November 2026."),
        ),
        (
            "id-en-time-002",
            ("It starts at 08:45 and finishes around 10:15.",
             "It starts at 8.45 and ends about 10.15."),
            ("It starts at 10:15 and finishes around 08:45.",
             "It starts at 08:40 and finishes around 10:15."),
        ),
        (
            "en-id-num-012",
            ("Batas waktunya adalah 28 September 2026 pukul 17:30 WIB.",
             "Pada 17.30 WIB, 28 September 2026 adalah batas waktu."),
            ("Batas waktunya adalah 28 September 2026 pukul 17:31 WIB.",
             "Batas waktunya adalah 28 Oktober 2026 pukul 17:30 WIB."),
        ),
    )
    for case_id, valid_outputs, altered_outputs in complete_numeric_cases:
        for translated in valid_outputs:
            assert evaluator.invariant_result(cases_by_id[case_id], translated)["critical_pass"], (case_id, translated)
        for translated in altered_outputs:
            assert not evaluator.invariant_result(cases_by_id[case_id], translated)["critical_pass"], (case_id, translated)

    incoming_correction = next(case for case in corpus["cases"] if case["id"] == "en-id-correction-001")
    for valid in ("Saya bilang lima belas, bukan lima puluh.", "Bukan lima puluh, melainkan lima belas."):
        assert evaluator.invariant_result(incoming_correction, valid)["critical_pass"], valid
    for reversed_meaning in ("Saya bilang lima puluh, bukan lima belas.", "Bukan lima belas, melainkan lima puluh."):
        assert not evaluator.invariant_result(incoming_correction, reversed_meaning)["critical_pass"], reversed_meaning
    spoken_number = next(case for case in corpus["cases"] if case["id"] == "audio-a11")
    assert evaluator.invariant_result(spoken_number, "The correct number is one hundred and twenty-five, not one hundred and fifty.")["critical_pass"]
    assert not evaluator.invariant_result(spoken_number, "The correct number is one hundred and fifty, not one hundred and twenty-five.")["critical_pass"]

    correction = next(case for case in corpus["cases"] if case["id"] == "id-en-correction-001")
    for valid in ("I mean Thursday, not Tuesday.", "Not Tuesday, but Thursday."):
        assert evaluator.invariant_result(correction, valid)["critical_pass"], valid
    for reversed_meaning in ("I mean Tuesday, not Thursday.", "Not Thursday, but Tuesday."):
        assert not evaluator.invariant_result(correction, reversed_meaning)["critical_pass"], reversed_meaning

    disfluency = next(case for case in corpus["cases"] if case["id"] == "id-en-disfluency-001")
    for prohibition in ("don't", "do not", "not", "don’t"):
        valid = f"Maybe hold this one for now; {prohibition} send it now."
        assert evaluator.invariant_result(disfluency, valid)["critical_pass"], valid
        for mixed in (f"{valid} Actually, send it now.", f"Send it now. {valid}"):
            assert evaluator.invariant_result(disfluency, mixed)["forbidden_hits"] == ["send it now"], mixed
    assert evaluator.invariant_result(disfluency, "Maybe hold this one for now; send it now.")["forbidden_hits"] == ["send it now"]
    hallucination = {"preserve": [], "required_any": [], "forbidden": ["cloud"]}
    assert evaluator.invariant_result(hallucination, "Do not upload it to the cloud.")["forbidden_hits"] == ["cloud"]
    with TemporaryDirectory() as folder:
        invalid_path = Path(folder) / "invalid.json"
        for exceptions in ({"unknown cue": ["not unknown cue"]}, {"send it now": ["send it now"]}, {"send it now": "don't send it now"}):
            invalid = {**corpus, "cases": [{**disfluency, "forbidden_exceptions": exceptions}]}
            invalid_path.write_text(json.dumps(invalid), encoding="utf-8")
            try:
                evaluator.load_corpus(invalid_path)
            except ValueError as error:
                assert "invalid forbidden phrase exceptions" in str(error), error
            else:
                raise AssertionError(f"accepted unsafe exception contract: {exceptions}")
        invalid = {**corpus, "cases": [{**cases_by_id["en-id-entity-001"], "strict_preserve": ["undeclared literal"]}]}
        invalid_path.write_text(json.dumps(invalid), encoding="utf-8")
        try:
            evaluator.load_corpus(invalid_path)
        except ValueError as error:
            assert "strict literals must be declared" in str(error), error
        else:
            raise AssertionError("accepted an undeclared strict literal")

    candidate = dict(perfect)
    candidate["id-en-negation-001"] = "I will attend the meeting tomorrow."
    comparison = evaluator.compare_reports(
        corpus,
        perfect,
        candidate,
        baseline_corpus_fingerprint=fingerprint,
        candidate_corpus_fingerprint=fingerprint,
        baseline_source_identity="baseline-sha",
        candidate_source_identity="candidate-sha",
    )
    assert comparison["complete_result_sets"] is True
    assert comparison["critical_regressions"] == ["id-en-negation-001"]
    assert comparison["critical_regressions_by_risk_tag"]["negation"] == ["id-en-negation-001"]
    assert comparison["critical_recoveries"] == []
    assert comparison["group_critical_pass_rate_deltas"]["negation"] < 0
    assert comparison["risk_tag_critical_pass_rate_deltas"]["negation"] < 0
    assert comparison["promotion_provenance_complete"] is True
    assert comparison["promotion_safe_on_declared_critical_invariants"] is False

    missing_provenance = evaluator.compare_reports(corpus, perfect, perfect)
    assert missing_provenance["promotion_provenance_complete"] is False
    assert missing_provenance["promotion_safe_on_declared_critical_invariants"] is False

    # Unbound or mismatched source bundles may be graded, but not promoted,
    # even if both translations are perfect and both runtime IDs are present.
    for baseline_fp, candidate_fp in (
        ("", fingerprint),
        (fingerprint, ""),
        ("0" * 64, fingerprint),
        (fingerprint, "0" * 64),
    ):
        unbound = evaluator.compare_reports(
            corpus, perfect, perfect,
            baseline_corpus_fingerprint=baseline_fp,
            candidate_corpus_fingerprint=candidate_fp,
            baseline_source_identity="baseline-sha",
            candidate_source_identity="candidate-sha",
        )
        assert unbound["critical_regressions"] == []
        assert unbound["promotion_provenance_complete"] is False
        assert unbound["promotion_safe_on_declared_critical_invariants"] is False
        assert unbound["complete_result_sets"] is (
            baseline_fp != "0" * 64 and candidate_fp != "0" * 64
        )

    recovered = evaluator.compare_reports(
        corpus,
        candidate,
        perfect,
        baseline_corpus_fingerprint=fingerprint,
        candidate_corpus_fingerprint=fingerprint,
        baseline_source_identity="candidate-sha",
        candidate_source_identity="baseline-sha",
    )
    assert recovered["critical_regressions"] == []
    assert recovered["critical_recoveries"] == ["id-en-negation-001"]
    assert recovered["group_critical_pass_rate_deltas"]["negation"] > 0
    assert recovered["promotion_provenance_complete"] is True
    assert recovered["promotion_safe_on_declared_critical_invariants"] is True

    # Lock down the actual native failure without rewriting model output.
    budget_case = next(case for case in corpus["cases"] if case["id"] == "en-id-budget-purpose-001")
    invented_budget = "Jangan kirim laporan hari ini. Kirimlah besok pukul tiga sore setelah saya menyetujuinya. Anggaran pembangunannya adalah lima belas juta rupiah."
    budget_failure = evaluator.invariant_result(budget_case, invented_budget)
    assert budget_failure["critical_pass"] is False
    assert budget_failure["forbidden_hits"] == ["pembangun"]
    assert evaluator.invariant_result(budget_case, budget_case["references"][0])["critical_pass"] is True

    # A new meaning loss must not hide behind an already-failed approval cue.
    previous_case = "id-en-informal-workplace-005"
    already_failed = dict(perfect)
    already_failed[previous_case] = "Here, only this version will be approved. Don't do the previous one."
    worse = dict(already_failed)
    worse[previous_case] = "Here, only this version is approved. Don't do that before."
    worsened = evaluator.compare_reports(
        corpus, already_failed, worse,
        baseline_corpus_fingerprint=fingerprint,
        candidate_corpus_fingerprint=fingerprint,
        baseline_source_identity="baseline-sha",
        candidate_source_identity="candidate-sha",
    )
    assert worsened["baseline_critical_failures"] == 1
    assert worsened["candidate_critical_failures"] == 1
    assert worsened["critical_regressions"] == [previous_case]
    assert worsened["critical_regression_details"][previous_case]["missing_concepts"] == [["previous"]]
    assert worsened["promotion_safe_on_declared_critical_invariants"] is False

    # Regrade retained source-identical outputs without promoting the old run.
    original_corpus = json.loads(json.dumps(corpus))
    old_cases = {case["id"]: case for case in original_corpus["cases"]}
    old_conversion = old_cases["id-en-number-002"]
    old_conversion["preserve"] = ["12", "15"]
    old_conversion["required_any"] = [["conversion"], ["not"]]
    old_conversion.pop("ordered_any")
    old_cases["en-id-number-002"].pop("ordered_any")
    old_fingerprint = evaluator.corpus_fingerprint(original_corpus)
    retained = {
        "results": dict(perfect),
        "corpus_fingerprint": old_fingerprint,
        "source_identity": "historic-model-receipt",
        "result_file_sha256": "a" * 64,  # Synthetic fixture, not inference proof.
    }
    retained["results"]["id-en-number-002"] = "The target is 12%, not 15% conversion."
    retained["results"]["en-id-number-002"] = "Ukuran filenya 24 GB, bukan 2,4 GB."
    report = evaluator.regrade_retained(original_corpus, corpus, retained)
    assert report["request_equivalence"] == "EXACT_DECLARED_REQUESTS"
    assert report["case_count"] == validation["case_count"]
    assert report["before_quality_counts"]["PASS"] > report["after_quality_counts"]["PASS"]
    assert {row["case_id"] for row in report["changed_cases"]} == {
        "id-en-number-002", "en-id-number-002",
    }
    assert report["new_model_inference_executed"] is False
    assert report["release_promotion_eligible"] is False
    assert "translated_text" not in json.dumps(report)
    assert evaluator.regrade_retained(corpus, corpus, {
        **retained, "results": dict(perfect), "corpus_fingerprint": fingerprint,
    })["changed_cases"] == []

    altered_requests = json.loads(json.dumps(original_corpus))
    altered_requests["cases"][0]["source"] += " Extra sentence."
    for tampered_original, tampered_bundle, expected_error in (
        (original_corpus, {**retained, "corpus_fingerprint": ""}, "fingerprint"),
        (original_corpus, {**retained, "source_identity": ""}, "source identity"),
        (original_corpus, {**retained, "result_file_sha256": ""}, "file digest"),
        (original_corpus, {**retained, "results": {"missing": "output"}}, "result set"),
        (altered_requests, {**retained,
            "corpus_fingerprint": evaluator.corpus_fingerprint(altered_requests)}, "worker requests"),
    ):
        try:
            evaluator.regrade_retained(tampered_original, corpus, tampered_bundle)
        except ValueError as error:
            assert expected_error in str(error), (expected_error, error)
        else:
            raise AssertionError(f"retained regrade accepted {expected_error}")

    # The file digest identifies exactly the bytes parsed by the result loader.
    with TemporaryDirectory() as folder:
        result_file = Path(folder) / "retained.json"
        result_bytes = json.dumps({
            "source_identity": retained["source_identity"],
            "corpus_fingerprint": old_fingerprint,
            "results": [
                {"case_id": ident, "translated_text": value}
                for ident, value in retained["results"].items()
            ],
        }, ensure_ascii=False).encode("utf-8")
        result_file.write_bytes(result_bytes)
        loaded = evaluator.load_result_bundle(result_file)
        assert loaded["result_file_sha256"] == hashlib.sha256(result_bytes).hexdigest()
        assert evaluator.regrade_retained(original_corpus, corpus, loaded)["changed_cases"]

        # Fail closed on malformed, oversized or shape-confused retained bundles.
        bad_result_contents = (
            (b"", "empty"),
            (b"null", "object or list"),
            (b"{}", "results must be a list"),
            (b"[null]", "result row must be an object"),
            (b'[{"case_id": 123, "translated_text": "hello"}]', "case_id must be"),
            (b'[{"case_id": "example", "translated_text": null}]', "translated_text must be"),
            (b'[{"case_id": "example", "translated_text": true}]', "translated_text must be"),
            (b'[{"case_id": "example", "translated_text": "x"}, {"case_id": "example", "translated_text": "y"}]', "duplicate result case_id"),
            (b'{"schema": "unknown", "results": []}', "unsupported result bundle schema"),
            (b'{"corpus_fingerprint": null, "results": []}', "corpus_fingerprint must be"),
            (b'{"source_identity": 123, "results": []}', "source_identity must be"),
            (b'{"schema": "translateit.translation_quality.worker_run.v1", "execution_complete": false, "results": []}', "incomplete worker execution"),
        )
        for contents, expected_error in bad_result_contents:
            result_file.write_bytes(contents)
            try:
                evaluator.load_result_bundle(result_file)
            except ValueError as error:
                assert expected_error in str(error), (expected_error, error)
            else:
                raise AssertionError(f"accepted invalid result bundle: {expected_error}")

        result_file.write_text(json.dumps([
            {"case_id": "example", "translated_text": "x" * (evaluator.MAX_RESULT_TEXT_CHARS + 1)}
        ]), encoding="utf-8")
        try:
            evaluator.load_result_bundle(result_file)
        except ValueError as error:
            assert "translated_text exceeds size limit" in str(error)
        else:
            raise AssertionError("accepted oversized translated_text")

        result_file.write_text(json.dumps([
            {"case_id": str(index), "translated_text": "ok"}
            for index in range(evaluator.MAX_RESULT_ROWS + 1)
        ]), encoding="utf-8")
        try:
            evaluator.load_result_bundle(result_file)
        except ValueError as error:
            assert "too many result rows" in str(error)
        else:
            raise AssertionError("accepted excessive result rows")

        with result_file.open("wb") as stream:
            stream.truncate(evaluator.MAX_RESULT_BUNDLE_BYTES + 1)
        try:
            evaluator.load_result_bundle(result_file)
        except ValueError as error:
            assert "result bundle exceeds size limit" in str(error)
        else:
            raise AssertionError("accepted oversized result file")

        # Empty output remains a legitimate completeness failure, not a stringified null.
        result_file.write_text('[{"case_id": "empty", "translated_text": ""}]', encoding="utf-8")
        assert evaluator.load_result_bundle(result_file)["results"] == {"empty": ""}

    stats = evaluator.corpus_stats(corpus)
    assert stats["case_count"] == validation["case_count"]
    assert stats["legacy_context_fixture_count"] == 3

    requests = evaluator.emit_requests(corpus)
    assert requests["corpus_fingerprint"] == fingerprint
    assert len(requests["requests"]) == validation["case_count"]
    contextual = [
        item
        for item in requests["requests"]
        if item["request"]["request_kind"] == "meeting_finalized_utterance"
    ]
    standalone = [
        item
        for item in requests["requests"]
        if item["request"]["request_kind"] == "standalone_text"
    ]
    assert len(contextual) == 3
    assert standalone
    assert all(item["request"]["meeting_lane"] == "you" for item in contextual)
    assert all(item["request"]["meeting_generation"] == 1 for item in contextual)
    assert all(
        "context_pairs" not in item["request"]
        and "terminology" not in item["request"]
        and "translation_style" not in item["request"]
        for item in requests["requests"]
    )
    print(json.dumps({
        "ok": True,
        "regression_case_count": validation["case_count"],
        "heldout_case_count": heldout_validation["case_count"],
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
