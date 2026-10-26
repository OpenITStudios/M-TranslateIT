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
MODEL = next(item for item in json.loads((worker_runner.WORKER.parent / "model_manifest.json").read_text(encoding="utf-8"))["models"]
             if item["stage"] == "translation_bidirectional_id_en")
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

    def worker(self, suite="fast", change=None):
        cases = iter(quality.select_suite(CORPUS, suite)["cases"])
        worker = Mock()
        worker.child.pid = 123
        worker.child.poll.return_value = 0

        def invoke(**request):
            case = next(cases)
            self.assertEqual(request, quality.emit_requests({**CORPUS, "cases": [case]})["requests"][0]["request"])
            return {"ok": True, "direction_pair": case["direction"].replace("-", "->"),
                    "complete": True, "finished_with_eos": True,
                    "translation_contract": "canonical_bidirectional_id_en",
                    "model_id": MODEL["model_id"], "model_revision": MODEL["revision"],
                    "translated_text": case["references"][0], **(change or {})}

        worker.invoke.side_effect = invoke
        return worker

    def run_fixture(self, worker, suite="fast"):
        with patch.object(worker_runner, "Worker", return_value=worker) as factory, patch.object(worker_runner, "pinned_source") as pin:
            report = worker_runner.run_translation_quality(Path("canonical-python.exe"), SHA, suite, 120)
        factory.assert_called_once()
        self.assertEqual(pin.call_count, 2)
        worker.close.assert_called_once()
        return report

    def test_both_suites_use_one_worker_and_the_exact_emitted_requests(self):
        for suite in ("fast", "full"):
            with self.subTest(suite=suite):
                worker = self.worker(suite)
                report = self.run_fixture(worker, suite)
                self.assertTrue(report["execution_complete"])
                self.assertEqual(report["verdict"], "PASS")
                self.assertEqual(worker.invoke.call_count, len(quality.select_suite(CORPUS, suite)["cases"]))

    def test_incomplete_or_unbound_output_stops_before_more_requests(self):
        for change in ({"ok": False}, {"complete": False}, {"finished_with_eos": False},
                       {"translated_text": " "}, {"direction_pair": "invalid"},
                       {"model_id": "other-model"}, {"model_revision": "wrong"},
                       {"translation_contract": "other-contract"}, {"hit_token_ceiling": True}):
            with self.subTest(change=change):
                worker = self.worker(change=change)
                report = self.run_fixture(worker)
                self.assertFalse(report["execution_complete"])
                self.assertEqual(report["verdict"], "FAIL")
                self.assertEqual(worker.invoke.call_count, 1)
                self.assertEqual(report["results"], [])
                self.assertEqual(report["checks"][0]["first_wrong_stage"], "translation_output_contract")
                self.assertGreater(report["counts"]["UNVERIFIED"], 0)

    def test_transport_and_cleanup_failures_never_pass(self):
        worker = self.worker()
        worker.invoke.side_effect = worker_runner.WorkerTransportError("controlled disconnect")
        report = self.run_fixture(worker)
        self.assertFalse(report["execution_complete"])
        self.assertEqual(report["checks"][0]["first_wrong_stage"], "worker_transport")
        worker = self.worker()
        worker.child.poll.return_value = None
        report = self.run_fixture(worker)
        self.assertEqual(report["verdict"], "FAIL")
        self.assertEqual(report["execution_error"], "worker_cleanup:not_terminated")
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "incomplete.json"
            path.write_text(json.dumps(report), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "incomplete worker execution"):
                quality.load_result_bundle(path)

    def test_linguistic_failure_is_distinct_from_execution_failure(self):
        report = self.run_fixture(self.worker(change={"translated_text": "An invented construction budget."}))
        self.assertTrue(report["execution_complete"])
        self.assertEqual(report["verdict"], "FAIL")
        self.assertEqual(report["checks"][0]["first_wrong_stage"], "translation_quality_check")
        self.assertTrue(report["checks"][0]["requires_linguistic_review"])
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "semantic-fail.json"
            path.write_text(json.dumps(report), encoding="utf-8")
            self.assertEqual(len(quality.load_result_bundle(path)["results"]), len(report["results"]))

    def test_stale_source_prevents_worker_start(self):
        with patch.object(worker_runner, "pinned_source", side_effect=ValueError("stale source")), patch.object(worker_runner, "Worker") as factory, self.assertRaisesRegex(ValueError, "stale source"):
            worker_runner.run_translation_quality(Path("canonical-python.exe"), SHA, "fast", 120)
        factory.assert_not_called()

    def test_cli_exit_and_immutable_output_follow_actual_verdict(self):
        with tempfile.TemporaryDirectory() as folder:
            for change, expected in ((None, 0), ({"translated_text": "Invented output."}, 2)):
                path = Path(folder) / f"result-{expected}.json"
                argv = ["worker_runner.py", "--translation-suite", "fast", "--python", "canonical-python.exe",
                        "--source-sha", SHA, "--output", str(path)]
                with patch.object(sys, "argv", argv), patch.object(worker_runner, "pinned_source"), patch.object(worker_runner, "Worker", return_value=self.worker(change=change)), patch("builtins.print"):
                    self.assertEqual(worker_runner.main(), expected)
                self.assertEqual(json.loads(path.read_text(encoding="utf-8"))["execution_complete"], True)
                with patch.object(sys, "argv", argv), patch.object(worker_runner, "Worker") as factory, patch.object(sys, "stderr"):
                    with self.assertRaises(SystemExit) as error:
                        worker_runner.main()
                    self.assertEqual(error.exception.code, 2)
                factory.assert_not_called()


if __name__ == "__main__":
    unittest.main()
