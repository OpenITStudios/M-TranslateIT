"""Protect direct translation execution integrity without model/device inference."""
from __future__ import annotations

import importlib.util
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools/audio_validation"))
import worker_runner

spec = importlib.util.spec_from_file_location("translation_worker_test_grader", ROOT / "tools/translation_quality/evaluate_translation_quality.py")
quality = importlib.util.module_from_spec(spec)
spec.loader.exec_module(quality)
CORPUS = quality.load_corpus(ROOT / "tools/translation_quality/corpus/translation_quality_v1.json")
HELDOUT = quality.load_corpus(ROOT / "tools/translation_quality/heldout/translation_quality_heldout_v1.json")
SHA = "a" * 40


class TranslationWorkerTests(unittest.TestCase):
    def test_real_transport_preserves_unicode_under_legacy_host_encoding(self):
        with tempfile.TemporaryDirectory() as folder:
            entry = Path(folder) / "echo.py"
            entry.write_text(
                "import sys,json\nfor raw in sys.stdin:\n"
                " v=json.loads(raw)\n"
                " print(json.dumps(dict(ok=True,stage=v['command'],request_id=v['request_id'],text=v['text'],points=list(map(ord,v['text']))),ensure_ascii=False),flush=True)\n",
                encoding="utf-8",
            )
            with patch.object(worker_runner, "WORKER", entry), patch.dict(os.environ, {"PYTHONIOENCODING": "cp1252", "PYTHONUTF8": "0"}):
                worker = worker_runner.Worker(Path(sys.executable), 5)
                try:
                    text = "Rencana—Final_v2.1 © 2026 日本語"
                    response = worker.invoke("ping", text=text)
                    self.assertEqual(response["points"], list(map(ord, text)))
                    self.assertEqual(response["text"], text)
                finally:
                    worker.close()
                self.assertIsNotNone(worker.child.poll())

    def replay(self, suite="fast", change=None, outer=None):
        corpus = quality.select_suite(HELDOUT if suite == "heldout" else CORPUS, suite)
        def execute(requests):
            self.assertEqual(requests, worker_runner.native_translation_requests(corpus, SHA, 120, quality))
            rows = []
            for i, case in enumerate(corpus["cases"], 1):
                src, dst = case["direction"].split("-")
                response = {"ok": True, "stage": "translate", "request_id": f"helper-{i}",
                            "device": "cuda", "cuda_pci_id": "0000:01:00.0", "precision": "q6_k",
                            "source_language": src, "target_language": dst, "direction_pair": f"{src}->{dst}",
                            "complete": True, "finished_with_eos": True, "hit_token_ceiling": False,
                            "translation_contract": "canonical_bidirectional_id_en", "translation_degraded": False,
                            "translation_fallback_reason": "", "blocker": "",
                            "model_id": worker_runner.NATIVE_MT_MODEL, "model_revision": worker_runner.NATIVE_MT_REVISION,
                            "translated_text": case["references"][0], **(change or {})}
                rows.append({"case_id": case["id"], "response": response})
            return {"proof": worker_runner.NATIVE_TRANSLATION_PROOF, "source_sha": SHA,
                    "corpus_fingerprint": requests["corpus_fingerprint"], "cuda_pci_id": "0000:01:00.0",
                    "dll_pins_sha256": "b"*64, "request_sha256": "c"*64, "worker_pid": 123,
                    "worker_terminated": True, "execution_error": None, "results": rows, **(outer or {})}
        return execute

    def run_fixture(self, replay, suite="fast"):
        with patch.object(worker_runner, "native_translation_replay", side_effect=replay) as call, \
                patch.object(worker_runner, "pinned_source") as pin, patch.object(worker_runner, "Worker") as old:
            report = worker_runner.run_translation_quality(SHA, suite, 120)
        call.assert_called_once()
        old.assert_not_called()
        self.assertEqual(pin.call_count, 2)
        return report

    def test_both_suites_use_native_and_exact_current_text(self):
        for suite in ("fast", "full"):
            with self.subTest(suite=suite):
                report = self.run_fixture(self.replay(suite), suite)
                self.assertTrue(report["execution_complete"])
                self.assertEqual(report["verdict"], "PASS")
                self.assertEqual(len(report["results"]), len(quality.select_suite(CORPUS, suite)["cases"]))
                self.assertEqual(quality.declared_worker_receipt_suite(report), suite)

    def test_heldout_replay_is_distinct_from_full_regression(self):
        corpus = quality.select_suite(HELDOUT, "heldout")
        self.assertEqual(corpus, quality.select_suite(HELDOUT, "full"))
        with self.assertRaises(ValueError):
            quality.select_suite(CORPUS, "heldout")
        report = self.run_fixture(self.replay("heldout"), "heldout")
        self.assertTrue(report["execution_complete"])
        self.assertEqual(report["execution_profile"]["name"], "translation-heldout")
        self.assertEqual(len(report["results"]), len(HELDOUT["cases"]))
        self.assertEqual(report["selection"], {"name": "heldout"})
        self.assertEqual(quality.declared_worker_receipt_suite(report), "heldout")
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "heldout.json"
            path.write_text(json.dumps(report), encoding="utf-8")
            bundle = quality.load_result_bundle(path)
            self.assertTrue(quality.declared_worker_report_consistent(corpus, bundle))
            self.assertFalse(quality.declared_worker_report_consistent(CORPUS, bundle))
        values = {row["case_id"]: row["translated_text"] for row in report["results"]}
        comparison = quality.compare_reports(corpus, values, values,
            baseline_corpus_fingerprint=report["corpus_fingerprint"],
            candidate_corpus_fingerprint=report["corpus_fingerprint"],
            baseline_source_identity=report["source_identity"],
            candidate_source_identity=report["source_identity"])
        self.assertFalse(comparison["full_regression_evaluated"])
        self.assertFalse(comparison["promotion_safe_on_declared_critical_invariants"])

    def test_incomplete_or_unbound_output_cannot_pass(self):
        for change in ({"ok": False}, {"complete": False}, {"finished_with_eos": False},
                       {"translated_text": " "}, {"direction_pair": "invalid"}, {"device": "cpu"},
                       {"cuda_pci_id": "0000:02:00.0"}, {"model_id": "other"}, {"model_revision": "wrong"},
                       {"translation_contract": "other"}, {"hit_token_ceiling": True}):
            with self.subTest(change=change):
                report = self.run_fixture(self.replay(change=change))
                self.assertFalse(report["execution_complete"])
                self.assertEqual(report["verdict"], "FAIL")
                self.assertEqual(report["results"], [])
                self.assertEqual(report["checks"][0]["first_wrong_stage"], "translation_output_contract")

    def test_cleanup_missing_rows_and_transport_failure_never_pass(self):
        for replay in (self.replay(outer={"worker_terminated":False}),
                       self.replay(outer={"results":[]}),
                       Mock(side_effect=worker_runner.WorkerTransportError("controlled disconnect"))):
            report = self.run_fixture(replay)
            self.assertFalse(report["execution_complete"])
            self.assertEqual(report["verdict"], "FAIL")
            with tempfile.TemporaryDirectory() as folder:
                path = Path(folder) / "incomplete.json"
                path.write_text(json.dumps(report), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "incomplete worker execution"):
                    quality.load_result_bundle(path)

    def test_linguistic_failure_remains_distinct(self):
        report = self.run_fixture(self.replay(change={"translated_text": "An invented construction budget."}))
        self.assertTrue(report["execution_complete"])
        self.assertEqual(report["verdict"], "FAIL")
        self.assertEqual(report["checks"][0]["first_wrong_stage"], "translation_quality_check")
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "semantic-fail.json"
            path.write_text(json.dumps(report), encoding="utf-8")
            self.assertEqual(len(quality.load_result_bundle(path)["results"]), len(report["results"]))

    def test_review_is_not_pass_and_native_receipt_tampering_is_rejected(self):
        def replay(requests):
            receipt = self.replay()(requests)
            receipt["results"][0]["response"]["translated_text"] = "I meant Thursday instead of Tuesday."
            return receipt
        report = self.run_fixture(replay)
        self.assertTrue(report["execution_complete"])
        self.assertNotEqual(report["verdict"], "PASS")
        self.assertEqual(quality.declared_worker_receipt_suite(report), "fast")
        report["results"][0]["response"]["cuda_pci_id"] = "0000:02:00.0"
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "tampered.json"
            path.write_text(json.dumps(report), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "invalid native worker receipt"):
                quality.load_result_bundle(path)

    def test_stale_source_prevents_native_launch(self):
        with patch.object(worker_runner, "pinned_source", side_effect=ValueError("stale source")), \
                patch.object(worker_runner, "native_translation_replay") as call, self.assertRaisesRegex(ValueError, "stale source"):
            worker_runner.run_translation_quality(SHA, "fast", 120)
        call.assert_not_called()

    def test_cli_exit_and_immutable_output_follow_actual_verdict(self):
        with tempfile.TemporaryDirectory() as folder:
            for change, expected in ((None, 0), ({"translated_text": "Invented output."}, 2)):
                path = Path(folder) / f"result-{expected}.json"
                argv = ["worker_runner.py", "--translation-suite", "fast", "--source-sha", SHA, "--output", str(path)]
                with patch.object(sys, "argv", argv), patch.object(worker_runner, "pinned_source"), \
                        patch.object(worker_runner, "native_translation_replay", side_effect=self.replay(change=change)), patch("builtins.print"):
                    self.assertEqual(worker_runner.main(), expected)
                self.assertTrue(json.loads(path.read_text(encoding="utf-8"))["execution_complete"])
                with patch.object(sys, "argv", argv), patch.object(worker_runner, "native_translation_replay") as call, patch.object(sys, "stderr"):
                    with self.assertRaises(SystemExit):
                        worker_runner.main()
                call.assert_not_called()


if __name__ == "__main__":
    unittest.main()
