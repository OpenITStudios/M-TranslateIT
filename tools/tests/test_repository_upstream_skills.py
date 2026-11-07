"""Safety and integrity tests for the inert Matt Pocock snapshot."""
from __future__ import annotations

import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
from repository_upstream_skills import verify_upstream, git_blob_sha, score_method_runs  # noqa: E402


class UpstreamSkillsTests(unittest.TestCase):
    def test_pinned_snapshot_and_complete_method_catalog(self):
        self.assertEqual(verify_upstream(ROOT), [])
        manifest = json.loads((ROOT / ".agents/vendor/mattpocock/snapshot.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["upstreamSkillCount"], 39)
        self.assertEqual(len(manifest["files"]), 119)

    def test_git_blob_digest_is_content_sensitive(self):
        self.assertEqual(git_blob_sha(b"test"), "30d74d258442c7c65512eafab474568dd706c430")
        self.assertNotEqual(git_blob_sha(b"test"), git_blob_sha(b"tEst"))

    def _fixture(self, temp: Path):
        shutil.copytree(ROOT / ".agents/vendor/mattpocock", temp / ".agents/vendor/mattpocock")
        (temp / ".agents/methods").mkdir(parents=True)
        (temp / ".agents/evals").mkdir(parents=True)
        shutil.copy2(ROOT / ".agents/methods/catalog.json", temp / ".agents/methods/catalog.json")
        shutil.copy2(ROOT / ".agents/evals/method-overlay-cases.json",
                     temp / ".agents/evals/method-overlay-cases.json")

    def test_modified_upstream_bytes_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._fixture(root)
            upstream = root / ".agents/vendor/mattpocock/skills/engineering/tdd/SKILL.md"
            upstream.write_text(upstream.read_text(encoding="utf-8") + "\nnoise\n", encoding="utf-8")
            self.assertTrue(any("upstream content drift" in x for x in verify_upstream(root)))

    def test_missing_method_catalog_entry_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._fixture(root)
            path = root / ".agents/methods/catalog.json"
            catalog = json.loads(path.read_text(encoding="utf-8"))
            catalog["methods"].pop()
            path.write_text(json.dumps(catalog), encoding="utf-8")
            self.assertTrue(any("catalog must account" in x for x in verify_upstream(root)))

    def test_human_only_skill_cannot_activate_implicitly(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._fixture(root)
            path = root / ".agents/methods/catalog.json"
            catalog = json.loads(path.read_text(encoding="utf-8"))
            next(x for x in catalog["methods"] if x["id"] == "implement")["activation"] = "conditional"
            path.write_text(json.dumps(catalog), encoding="utf-8")
            self.assertTrue(any("human-invoked" in x for x in verify_upstream(root)))

    def test_beta_method_cannot_activate_implicitly(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._fixture(root)
            path = root / ".agents/methods/catalog.json"
            catalog = json.loads(path.read_text(encoding="utf-8"))
            next(x for x in catalog["methods"] if x["id"] == "software-factory")["activation"] = "conditional"
            path.write_text(json.dumps(catalog), encoding="utf-8")
            self.assertTrue(any("beta method" in x for x in verify_upstream(root)))

    def test_observed_receipt_scorer_distinguishes_wrong_route(self):
        cases = json.loads((ROOT / ".agents/evals/method-overlay-cases.json").read_text(encoding="utf-8"))["cases"]
        runs = [{"caseId": x["id"], "selectedMethod": x["expectedMethod"]} for x in cases]
        matched = score_method_runs(runs)
        self.assertEqual(matched["correct"], matched["total"])
        runs[0]["selectedMethod"] = "ask-matt"
        mismatch = score_method_runs(runs)
        self.assertEqual(mismatch["correct"], matched["total"] - 1)
        self.assertEqual(mismatch["mismatches"][0]["caseId"], cases[0]["id"])

    def test_unknown_vendor_file_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self._fixture(root)
            (root / ".agents/vendor/mattpocock/skills/extra.md").write_text("untracked", encoding="utf-8")
            self.assertTrue(any("inventory drift" in x for x in verify_upstream(root)))


if __name__ == "__main__":
    unittest.main()
