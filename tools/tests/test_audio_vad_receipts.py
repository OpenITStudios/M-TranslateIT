"""Prevent false VAD proof, swapped WAV identities and incomplete case receipts."""
from __future__ import annotations

import copy
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parents[1] / "audio_validation"
sys.path.insert(0, str(HERE))
import harness
import vad_runner
from worker_runner import finalized_asr_outcome, grade_finalized_asr


class AudioVadReceiptTests(unittest.TestCase):
    def test_probe_requires_real_control_and_preserves_negation_failure(self):
        cases = {r["id"]: r for r in harness.specs()[0]}
        report = {"results": [
            {"scenario_id": "A03", "outcome": "SPEECH_TRANSCRIBED",
             "asr": {"transcript_text": cases["A03"]["reference"]}},
            {"scenario_id": "A14", "outcome": "ASR_REJECTED_EMPTY"},
        ]}
        self.assertEqual(grade_finalized_asr(report, cases, {"A03", "A14"})["verdict"], "PASS")
        # Low aggregate WER cannot hide dropping the critical negation.
        report["results"][0]["asr"]["transcript_text"] = "Saya setuju dengan perubahan itu."
        self.assertEqual(grade_finalized_asr(report, cases, {"A03", "A14"})["verdict"], "FAIL")
        report["results"] = [{"scenario_id": "A14", "outcome": "UNVERIFIED_ASR_FAILURE"}]
        result = grade_finalized_asr(report, cases, {"A03", "A14"})
        self.assertEqual(result["verdict"], "UNVERIFIED")
        self.assertEqual(result["counts"]["PASS"], 0)

    def test_exported_writer_wav_is_bound_and_original_substitution_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            audio = root / "A14_1.wav"
            harness.write_wave(audio, [1000, -1000] * 16000)
            stage = {"writer_attempts": 1, "writer_valid": 1, "segments": [{
                "file": "A14_1.wav", "sha256": harness.fingerprint(audio),
                "frames": 32000, "rate_hz": 16000,
            }]}
            original = vad_runner.verify_segments(root, {"A14": stage})
            self.assertEqual(len(original), 64)
            # An independently valid WAV is still not the finalized segment.
            harness.write_wave(audio, [2000, -2000] * 16000)
            with self.assertRaisesRegex(ValueError, "identity differs"):
                vad_runner.verify_segments(root, {"A14": stage})

    def test_failed_asr_is_never_classified_as_safe_noise(self):
        self.assertEqual(finalized_asr_outcome({"ok": False, "transcript_text": "",
                         "blocker": "asr:empty_transcript"}, non_speech=True), "ASR_REJECTED_EMPTY")
        self.assertEqual(finalized_asr_outcome({"ok": False, "transcript_text": "",
                         "blocker": "asr:model_failed"}, non_speech=True), "UNVERIFIED_ASR_FAILURE")
        self.assertEqual(finalized_asr_outcome({"ok": True, "transcript_text": "halo"},
                         non_speech=True), "NON_SPEECH_TRANSCRIPT")
        self.assertEqual(finalized_asr_outcome({"ok": True, "transcript_text": ""},
                         non_speech=True), "UNVERIFIED_ASR_FAILURE")

    def setUp(self):
        self.scenarios, self.scenario_hash = harness.specs()
        self.record = {row["id"]: "a" * 64 for row in self.scenarios}
        self.manifest = {
            "schema": harness.FIXTURE_SCHEMA,
            "scenario_fingerprint": self.scenario_hash,
            "audio_fingerprint": "b" * 64,
            "fixtures": [{"id": key, "sha256": value} for key, value in self.record.items()],
        }

    def receipts(self, changed=None):
        lines = []
        for key in self.record:
            value = {
                "schema": vad_runner.RECEIPT_SCHEMA, "scenario_id": key,
                "audio_fingerprint": self.manifest["audio_fingerprint"],
                "scenario_fingerprint": self.scenario_hash,
                "audio_sha256": self.record[key],
                "stage": {
                    "ok": True, "finalized_count": 1, "overflow_count": 0,
                    "writer_ok": True, "claim": vad_runner.CLAIM, "test_only": True,
                },
            }
            if key == "A01" and changed:
                changed(value)
            lines.append(vad_runner.PREFIX + __import__("json").dumps(value))
        return "\n".join(lines)

    def test_complete_replay_receipts_are_bound_to_original_wavs(self):
        result = vad_runner.parse_receipts(
            self.receipts(), self.manifest, self.scenario_hash
        )
        self.assertEqual(len(result), len(self.scenarios))
        self.assertEqual(result["A01"]["finalized_count"], 1)

    def test_modified_audio_hash_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "unbound"):
            vad_runner.parse_receipts(
                self.receipts(lambda r: r.__setitem__("audio_sha256", "c"*64)),
                self.manifest, self.scenario_hash
            )

    def test_incomplete_receipt_set_is_not_a_pass(self):
        one = self.receipts().split("\n")[:-1]
        with self.assertRaisesRegex(ValueError, "incomplete"):
            vad_runner.parse_receipts("\n".join(one), self.manifest, self.scenario_hash)

    def test_false_proof_flag_is_not_trusted(self):
        with self.assertRaisesRegex(ValueError, "invalid Rust VAD"):
            vad_runner.parse_receipts(
                self.receipts(lambda r: r["stage"].__setitem__("test_only", False)),
                self.manifest, self.scenario_hash
            )

    def test_merge_requires_exact_worker_identity_and_no_duplicate_vad(self):
        source_sha = "1"*40
        stage = vad_runner.parse_receipts(
            self.receipts(), self.manifest, self.scenario_hash
        )
        output = vad_runner.make_observations(
            source_sha, self.manifest, self.scenarios, self.scenario_hash, stage, None
        )
        self.assertEqual(output["results"][0]["stages"]["vad"]["claim"], vad_runner.CLAIM)
        bad = copy.deepcopy(output)
        bad["source_identity"]["local_sha"] = "2"*40
        with self.assertRaisesRegex(ValueError, "different source"):
            vad_runner.make_observations(
                source_sha, self.manifest, self.scenarios, self.scenario_hash, stage, bad
            )
        good = copy.deepcopy(output)
        good["source_identity"]["local_sha"] = source_sha
        with self.assertRaisesRegex(ValueError, "duplicate VAD"):
            vad_runner.make_observations(
                source_sha, self.manifest, self.scenarios, self.scenario_hash, stage, good
            )


if __name__ == "__main__":
    unittest.main()
