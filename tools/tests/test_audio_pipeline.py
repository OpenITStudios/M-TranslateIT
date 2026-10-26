"""Prevent false VAD proof, swapped WAV identities and incomplete case receipts."""
from __future__ import annotations

import copy
import json
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
from pipeline_checks import (
    grade_pipeline,
    pipeline_contracts,
    score_pipeline,
    source_admission_passed,
)


class AudioPipelineTests(unittest.TestCase):
    def test_offline_score_cli_uses_the_same_failure_exit_contract(self):
        for verdict, expected in (('PASS', 0), ('FAIL', 2), ('UNVERIFIED', 2)):
            with patch.object(harness, 'score', return_value={'verdict': verdict}):
                self.assertEqual(harness.main(['score', '--fixtures', 'fixture', '--observations', 'report.json']), expected)
        with patch.object(harness, 'score', return_value={'counts': {'PASS': 0, 'FAIL': 0, 'UNVERIFIED': 1}}):
            self.assertEqual(harness.main(['score', '--fixtures', 'fixture', '--observations', 'report.json']), 2)

    def test_named_profiles_keep_audio_identity_independent(self):
        cases, audio_id = harness.specs()
        core = harness.regression_profile('core')
        endurance = harness.regression_profile('endurance')
        self.assertEqual(len(core['scenario_ids']), 13)
        self.assertEqual(core['scenario_ids'], endurance['scenario_ids'])
        self.assertEqual((core['rounds'], endurance['rounds']), (2, 20))
        contracts, _, _ = pipeline_contracts({c['id']: c for c in cases})
        self.assertTrue(all(c['kind'] != 'speech' or c['id'] in contracts for c in cases if c['id'] in core['scenario_ids']))
        with tempfile.TemporaryDirectory() as tmp:
            suite = json.loads(harness.SUITE.read_text(encoding='utf-8'))
            suite['regression']['profiles']['core']['rounds'] = 3
            path = Path(tmp) / 'scenarios.json'
            path.write_text(json.dumps(suite), encoding='utf-8')
            self.assertEqual(harness.specs(path)[1], audio_id)
            self.assertNotEqual(harness.regression_profile('core', path)['fingerprint'], core['fingerprint'])

    def test_source_admission_requires_all_validators_and_same_revision(self):
        proof = {'source_identity': {'local_sha': '2' * 40}, 'verdict': 'PASS',
                 'checks': [{'name': name, 'status': 'PASS', 'exit_code': 0}
                            for name in ('audio_contracts', 'asr_evaluator', 'translation_evaluator', 'tts_evaluator')]}
        self.assertTrue(source_admission_passed(proof, '2' * 40))
        self.assertFalse(source_admission_passed(proof, '3' * 40))
        self.assertFalse(source_admission_passed({**proof, 'checks': proof['checks'][:1]}, '2' * 40))
        bad = copy.deepcopy(proof)
        bad['checks'][1]['exit_code'] = 2
        self.assertFalse(source_admission_passed(bad, '2' * 40))

    def test_core_cli_admission_prevents_model_execution(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / 'blocked.json'
            args = ['worker_runner', '--profile', 'core', '--fixtures', 'fixtures', '--python', 'python',
                    '--source-sha', '2' * 40,
                    '--output', str(output)]
            with patch.object(sys, 'argv', args), patch.object(worker_runner, 'run_source_checks', return_value={
                'verdict': 'FAIL', 'checks': [{'name': 'audio_contracts', 'status': 'FAIL'}]
            }), patch.object(worker_runner, 'run_finalized') as model, patch.object(vad_runner, 'run') as vad:
                self.assertEqual(worker_runner.main(), 2)
                model.assert_not_called()
                vad.assert_not_called()
            report = json.loads(output.read_text(encoding='utf-8'))
            self.assertEqual(report['counts']['UNVERIFIED'], 26)
            self.assertEqual(report['proof'], 'ADMISSION_FAILED_PIPELINE_NOT_EXECUTED')

    def test_named_cli_owns_coverage_and_current_vad(self):
        source = {'source_identity': {'local_sha': '2' * 40}, 'verdict': 'PASS',
                  'checks': [{'name': name, 'status': 'PASS', 'exit_code': 0}
                             for name in ('audio_contracts', 'asr_evaluator', 'translation_evaluator', 'tts_evaluator')]}
        for profile, rounds in (('core', 2), ('endurance', 20)):
            with tempfile.TemporaryDirectory() as tmp:
                output = Path(tmp) / 'new.json'
                args = ['worker_runner', '--profile', profile, '--fixtures', 'fixtures', '--python', 'python',
                        '--source-sha', '2' * 40, '--output', str(output)]
                with patch.object(sys, 'argv', args), patch.object(worker_runner, 'run_source_checks', return_value=source), patch.object(
                    vad_runner, 'run'
                ) as vad, patch.object(worker_runner, 'run_finalized', return_value={
                    'proof': 'SIMULATED_DISPATCH_ONLY', 'verdict': 'PASS', 'checks': [], 'counts': {}}
                ) as model:
                    self.assertEqual(worker_runner.main(), 0)
                vad.assert_called_once()
                model.assert_called_once()
                positional, keywords = model.call_args
                self.assertEqual(positional[3], set(harness.regression_profile('core')['scenario_ids']))
                self.assertEqual(keywords['rounds'], rounds)
                self.assertEqual(keywords['profile'], profile)
                self.assertIsNone(keywords['vad_source_sha'])
                self.assertEqual(vad.call_args.args[1], '2' * 40)

    def test_vad_execution_failure_is_reported_without_worker(self):
        source = {'source_identity': {'local_sha': '2' * 40}, 'verdict': 'PASS',
                  'checks': [{'name': name, 'status': 'PASS', 'exit_code': 0}
                             for name in ('audio_contracts', 'asr_evaluator', 'translation_evaluator', 'tts_evaluator')]}
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / 'new.json'
            args = ['worker_runner', '--profile', 'core', '--fixtures', 'fixtures', '--python', 'python',
                    '--source-sha', '2' * 40, '--output', str(output)]
            with patch.object(sys, 'argv', args), patch.object(worker_runner, 'run_source_checks', return_value=source), patch.object(
                vad_runner, 'run', side_effect=ValueError('controlled VAD execution failure')
            ), patch.object(worker_runner, 'run_finalized') as model:
                self.assertEqual(worker_runner.main(), 2)
                model.assert_not_called()
            report = json.loads(output.read_text(encoding='utf-8'))
            self.assertEqual(report['counts']['UNVERIFIED'], 26)
            self.assertTrue(all(c['first_wrong_stage'] == 'vad_execution' for c in report['checks']))

    def test_worker_adapter_preserves_full_text_and_rejects_nontext(self):
        text = 'word ' * 1000
        self.assertEqual(worker_runner.stage_result({'ok': True, 'transcript_text': text}, 'asr')['transcript_text'], text)
        self.assertEqual(worker_runner.stage_result({'ok': True, 'translated_text': text}, 'translation')['translated_text'], text)
        with self.assertRaises(worker_runner.WorkerTransportError):
            worker_runner.stage_result({'ok': True, 'translated_text': None}, 'translation')

    def test_semantic_contracts_allow_paraphrases_and_reject_reversed_corrections(self):
        cases = {r["id"]: r for r in harness.specs()[0]}
        contracts, evaluator, _ = pipeline_contracts(cases)
        for text in ('I mean 15, not 50.', 'I meant 15 rather than 50.', 'Not 50, but 15.',
                     'I mean fifteen, not 50.', 'I mean 15, not fifty.'):
            self.assertTrue(evaluator.invariant_result(contracts['A04'], text)['critical_pass'], text)
        for text in ('I mean 50, not 15.', 'I mean 15 and 50.', 'I mean 16, not 50.',
                     'I mean -15, not 50.', 'I mean minus fifteen, not 50.', 'I mean −15, not 50.', 'I mean 15.5, not 50.', 'I mean 115, not 50.'):
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
                with patch.object(harness, 'inspect', return_value=manifest), patch.object(worker_runner, 'Worker', side_effect=AssertionError('offline scoring must not start a worker')):
                    rescored = harness.score(root, root / 'result.json')
                self.assertEqual(rescored['verdict'], result['verdict'])
                self.assertEqual(rescored['checks'], result['checks'])
                self.assertFalse(any('A14' in str(fields) for _, fields in commands))
                if reversed_number:
                    self.assertEqual([c['first_wrong_stage'] for c in result['checks'] if c['status'] == 'FAIL'],
                                     ['translation_critical', 'translation_critical'])
                    failed_audio = [r['tts']['retained_output_path'] for r in result['results'] if r['scenario_id'] == 'A04']
                    self.assertEqual(len(failed_audio), 2)
                    self.assertTrue(all(Path(path).is_file() for path in failed_audio))
                    self.assertTrue(all(harness.fingerprint(Path(row['tts']['retained_output_path'])) == row['tts']['wav_sha256']
                                        for row in result['results'] if row['scenario_id'] == 'A04'))
                else:
                    self.assertFalse((root / 'result.json.artifacts').exists())
                    contracts, evaluator, _ = pipeline_contracts(by_id)
                    missing_round = copy.deepcopy(result)
                    missing_round['results'] = [r for r in result['results'] if r['round'] == 1]
                    self.assertEqual(grade_pipeline(missing_round, by_id, selection, 2, contracts, evaluator)['verdict'], 'UNVERIFIED')
                    numeral_variant = copy.deepcopy(result)
                    row = next(r for r in numeral_variant['results'] if r['scenario_id'] == 'A04')
                    row['translation']['translated_text'] = 'I meant fifteen rather than 50.'
                    row['roundtrip_asr']['transcript_text'] = 'I meant 15 rather than fifty.'
                    self.assertEqual(grade_pipeline(numeral_variant, by_id, selection, 2, contracts, evaluator)['verdict'], 'PASS')
                    unsupported = dict(contracts)
                    del unsupported['A03']
                    self.assertEqual(grade_pipeline(result, by_id, selection, 2, unsupported, evaluator)['verdict'], 'UNVERIFIED')
                    for stage, mutation, expected in (
                        ('translation', {'finished_with_eos': False}, 'translation_envelope'),
                        ('tts', {'actor_identity_match': False}, 'tts_identity_or_audio'),
                        ('tts', {'actor_token': 'other-actor'}, 'tts_identity_or_audio'),
                        ('tts', {'output_path': 'unrelated.wav'}, 'tts_identity_or_audio'),
                        ('roundtrip_asr', {'transcript_text': 'I agree with that change.'}, 'tts_critical_content'),
                    ):
                        bad = copy.deepcopy(result)
                        row = next(r for r in bad['results'] if r['scenario_id'] == 'A03')
                        row[stage].update(mutation)
                        grade = grade_pipeline(bad, by_id, selection, 2, contracts, evaluator)
                        self.assertEqual(grade['verdict'], 'FAIL')
                        self.assertIn(expected, [c['first_wrong_stage'] for c in grade['checks']])
                        failure = next(c for c in grade['checks'] if c['status'] == 'FAIL')
                        self.assertTrue(failure['expected'])
                        self.assertTrue(failure['actual'])
                        self.assertTrue(failure['evidence'])
                    for defect in ('missing', 'duplicate', 'reordered', 'wrong_hash'):
                        bad = copy.deepcopy(result)
                        positions = [i for i, r in enumerate(bad['results']) if r['scenario_id'] == 'A11' and r['round'] == 1]
                        if defect == 'missing':
                            del bad['results'][positions[0]]
                        elif defect == 'duplicate':
                            bad['results'].append(copy.deepcopy(bad['results'][positions[0]]))
                        elif defect == 'reordered':
                            a, b = positions
                            bad['results'][a], bad['results'][b] = bad['results'][b], bad['results'][a]
                        else:
                            bad['results'][positions[0]]['input_sha256'] = '0' * 64
                        grade = grade_pipeline(bad, by_id, selection, 2, contracts, evaluator)
                        failure = next(c for c in grade['checks'] if c['scenario_id'] == 'A11' and c['round'] == 1)
                        self.assertEqual(failure['first_wrong_stage'], 'input_coverage', defect)
                        self.assertEqual(failure['status'], 'UNVERIFIED' if defect == 'missing' else 'FAIL', defect)
                    incompatible = copy.deepcopy(result)
                    incompatible['translation_contract_fingerprint'] = '0' * 64
                    with self.assertRaises(ValueError):
                        score_pipeline(incompatible, by_id, suite_id, manifest['audio_fingerprint'])
                    bad = copy.deepcopy(result)
                    next(r for r in bad['results'] if r['scenario_id'] == 'A14')['translation'] = {'ok': True}
                    self.assertEqual(grade_pipeline(bad, by_id, selection, 2, contracts, evaluator)['verdict'], 'FAIL')
                    bad = copy.deepcopy(result)
                    bad['current_vad_required'] = True
                    bad['vad_stage_results']['A04']['ok'] = False
                    grade = grade_pipeline(bad, by_id, selection, 2, contracts, evaluator)
                    self.assertIn('vad', [c['first_wrong_stage'] for c in grade['checks'] if c['status'] == 'FAIL'])

    def test_pipeline_cli_returns_failure_for_unverified_acceptance(self):
        args = ['worker_runner', '--fixtures', 'fixture', '--python', 'python', '--source-sha', '2' * 40,
                '--ids', 'A04', '--output', 'new.json', '--vad-observations', 'vad.json',
                '--segments-dir', 'segments', '--finalized-pipeline']
        for verdict in ('FAIL', 'UNVERIFIED'):
            with self.subTest(verdict=verdict), patch.object(sys, 'argv', args), patch.object(
                worker_runner, 'run_finalized', return_value={'results': [], 'proof': 'SIMULATED_CLI_CONTRACT', 'verdict': verdict}
            ):
                self.assertEqual(worker_runner.main(), 2)

