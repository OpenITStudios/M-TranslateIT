"""Prevent false VAD proof, swapped WAV identities and incomplete case receipts."""
from __future__ import annotations

import copy
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

HERE = Path(__file__).resolve().parents[1] / "audio_validation"
sys.path.insert(0, str(HERE))
import harness
import vad_runner
import worker_runner
from worker_runner import (
    finalized_asr_outcome,
    grade_finalized_asr,
    grade_pipeline,
    pipeline_contracts,
)


class AudioVadReceiptTests(unittest.TestCase):
    def test_semantic_contracts_allow_paraphrases_and_reject_reversed_corrections(self):
        cases = {r["id"]: r for r in harness.specs()[0]}
        contracts, evaluator, _ = pipeline_contracts(cases)
        for text in ('I mean 15, not 50.', 'I meant 15 rather than 50.', 'Not 50, but 15.',
                     'I mean fifteen, not 50.', 'I mean 15, not fifty.'):
            self.assertTrue(evaluator.invariant_result(contracts['A04'], text)['critical_pass'], text)
        for text in ('I mean 50, not 15.', 'I mean 15 and 50.', 'I mean 16, not 50.',
                     'I mean -15, not 50.', 'I mean 15.5, not 50.', 'I mean 115, not 50.'):
            self.assertFalse(evaluator.invariant_result(contracts['A04'], text)['critical_pass'], text)
        self.assertFalse(evaluator.invariant_result(contracts['A34'], 'This version is ready for release.')['critical_pass'])

    def test_finalized_pipeline_reuses_one_worker_and_fails_closed(self):
        cases, suite_id = harness.specs()
        by_id = {x['id']: x for x in cases}
        selection = {'A03', 'A04', 'A11', 'A14'}
        for reversed_number in (False, True):
            with self.subTest(reversed_number=reversed_number), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                segments = root / 'segments'
                segments.mkdir()
                observed = {}
                asr_text = {}
                mt_text = {}
                for case in cases:
                    name = case['id']
                    texts = (['Angka yang benar 125.', 'Bukan 150.'] if name == 'A11'
                             else [case['reference']]) if name in selection - {'A14'} else []
                    rows = []
                    for index, text in enumerate(texts, 1):
                        file = f'{name}_{index}.wav'
                        harness.write_wave(segments / file, [1000, -1000] * 1600)
                        rows.append({'file': file, 'sha256': harness.fingerprint(segments / file), 'rate_hz': 16000, 'frames': 3200})
                        asr_text[file] = text
                        mt_text[text] = ({'A03': 'I disagree with that change.',
                            'A04': 'I mean 50, not 15.' if reversed_number else 'I meant 15 rather than 50.',
                            'A11': 'The correct number is 125.' if index == 1 else 'Not 150.'})[name]
                    observed[name] = {'claim': vad_runner.CLAIM, 'test_only': True, 'ok': True,
                        'segments': rows, 'writer_attempts': len(rows), 'writer_valid': len(rows),
                        'finalized_count': len(rows), 'overflow_count': 0}
                manifest = {'audio_fingerprint': 'a' * 64,
                            'fixtures': [{'id': x['id'], 'sha256': 'b' * 64} for x in cases]}
                prior = {'schema': harness.RESULT_SCHEMA, 'source_identity': {'local_sha': '1' * 40},
                    'scenario_fingerprint': suite_id, 'audio_fingerprint': 'a' * 64,
                    'segment_fingerprint': vad_runner.verify_segments(segments, observed),
                    'results': [{'scenario_id': name, 'audio_sha256': 'b' * 64, 'stages': {'vad': stage}}
                                for name, stage in observed.items()]}
                import json
                vad = root / 'vad.json'
                vad.write_text(json.dumps(prior), encoding='utf-8')
                workers = []
                commands = []

                class FakeWorker:
                    def __init__(self, *_args, instances=workers, calls=commands,
                                 asr_inputs=asr_text, translations=mt_text):
                        instances.append(self)
                        self.calls = calls
                        self.asr_inputs = asr_inputs
                        self.translations = translations
                        self.child = self
                        self.pid = 123
                        self.stopped = False
                        self.outputs = {}

                    def poll(self):
                        return 0 if self.stopped else None

                    def close(self):
                        self.stopped = True

                    def invoke(self, command, **fields):
                        self.calls.append((command, fields))
                        response = {'ok': True}
                        if command == 'voice_actor_preflight':
                            response['actor_token'] = 'selected-actor'
                        if command == 'transcribe':
                            path = Path(fields['audio_path'])
                            response['transcript_text'] = self.outputs[str(path)] if str(path) in self.outputs else self.asr_inputs[path.name]
                        if command == 'translate':
                            response.update(complete=True, finished_with_eos=True, direction_pair='id->en',
                                            translated_text=self.translations[fields['text']])
                        if command == 'voice_actor_synthesize':
                            path = Path(fields['output_path'])
                            harness.write_wave(path, [1000, -1000] * 1600)
                            self.outputs[str(path)] = fields['text']
                            response.update(actor_token='selected-actor', output_path=str(path))
                        return response

                with patch.object(worker_runner, 'pinned_source'), patch.object(worker_runner, 'inspect', return_value=manifest), patch.object(worker_runner, 'Worker', FakeWorker):
                    with self.assertRaises(ValueError):
                        worker_runner.run_finalized(root, root, '2' * 40, selection,
                            root / 'rejected.json', 120, vad, segments, pipeline=True)
                    self.assertEqual(workers, [])
                    result = worker_runner.run_finalized(root, root, '2' * 40, selection,
                        root / 'result.json', 120, vad, segments, pipeline=True, rounds=2, vad_source_sha='1' * 40)
                self.assertEqual(len(workers), 1)
                self.assertTrue(workers[0].stopped)
                self.assertEqual(result['input_source_identity']['local_sha'], '1' * 40)
                self.assertEqual(result['source_identity']['local_sha'], '2' * 40)
                self.assertEqual(len(result['checks']), 8)
                self.assertEqual(result['verdict'], 'FAIL' if reversed_number else 'PASS')
                self.assertFalse(any('A14' in str(fields) for _, fields in commands))
                if reversed_number:
                    self.assertEqual([c['first_wrong_stage'] for c in result['checks'] if c['status'] == 'FAIL'],
                                     ['translation_critical', 'translation_critical'])
                else:
                    contracts, evaluator, _ = pipeline_contracts(by_id)
                    missing_round = copy.deepcopy(result)
                    missing_round['results'] = [r for r in result['results'] if r['round'] == 1]
                    self.assertEqual(grade_pipeline(missing_round, by_id, selection, 2, contracts, evaluator)['verdict'], 'UNVERIFIED')
                    unsupported = dict(contracts)
                    del unsupported['A03']
                    self.assertEqual(grade_pipeline(result, by_id, selection, 2, unsupported, evaluator)['verdict'], 'UNVERIFIED')
                    for stage, mutation, expected in (
                        ('translation', {'finished_with_eos': False}, 'translation_envelope'),
                        ('tts', {'actor_identity_match': False}, 'tts_identity_or_audio'),
                        ('roundtrip_asr', {'transcript_text': 'I agree with that change.'}, 'tts_critical_content'),
                    ):
                        bad = copy.deepcopy(result)
                        row = next(r for r in bad['results'] if r['scenario_id'] == 'A03')
                        row[stage].update(mutation)
                        grade = grade_pipeline(bad, by_id, selection, 2, contracts, evaluator)
                        self.assertEqual(grade['verdict'], 'FAIL')
                        self.assertIn(expected, [c['first_wrong_stage'] for c in grade['checks']])
                    bad = copy.deepcopy(result)
                    next(r for r in bad['results'] if r['scenario_id'] == 'A14')['translation'] = {'ok': True}
                    self.assertEqual(grade_pipeline(bad, by_id, selection, 2, contracts, evaluator)['verdict'], 'FAIL')

    def test_pipeline_cli_returns_failure_for_unverified_acceptance(self):
        args = ['worker_runner', '--fixtures', 'fixture', '--python', 'python', '--source-sha', '2' * 40,
                '--ids', 'A04', '--output', 'new.json', '--vad-observations', 'vad.json',
                '--segments-dir', 'segments', '--finalized-pipeline']
        for verdict in ('FAIL', 'UNVERIFIED'):
            with self.subTest(verdict=verdict), patch.object(sys, 'argv', args), patch.object(
                worker_runner, 'run_finalized', return_value={'results': [], 'proof': 'SIMULATED_CLI_CONTRACT', 'verdict': verdict}
            ):
                self.assertEqual(worker_runner.main(), 2)

    def test_zero_segments_are_safe_only_for_proven_non_speech_rejection(self):
        cases = {r["id"]: r for r in harness.specs()[0]}
        report = {"results": [{"scenario_id": "A14", "outcome": "VAD_REJECTED_NON_SPEECH",
            "vad": {"ok": True, "finalized_count": 0, "overflow_count": 0}, "asr_executed": False}]}
        self.assertEqual(grade_finalized_asr(report, cases, {"A14"})["verdict"], "PASS")
        report["results"][0]["asr_executed"] = True
        self.assertEqual(grade_finalized_asr(report, cases, {"A14"})["verdict"], "UNVERIFIED")
        report["results"][0]["asr_executed"] = False
        report["results"][0]["vad"]["ok"] = False
        self.assertEqual(grade_finalized_asr(report, cases, {"A14"})["verdict"], "UNVERIFIED")
        report["results"][0]["scenario_id"] = "A34"
        report["results"][0]["vad"]["ok"] = True
        self.assertEqual(grade_finalized_asr(report, cases, {"A34"})["verdict"], "UNVERIFIED")

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
