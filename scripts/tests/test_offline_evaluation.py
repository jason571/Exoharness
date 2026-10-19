"""Offline evaluator contracts and real HTTP adapter fixtures; no paid models."""
import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'modules/benchmark/src'))
from exoharness_benchmark.contracts import Experience, Suite, digest
from exoharness_benchmark.experiment import Experiment, learning_prompt
from exoharness_benchmark.harness import Harness, origin
from exoharness_benchmark.evidence import write_private
from exoharness_benchmark.metrics import aggregate, retain, usage
from exoharness_benchmark.scenarios import create, current_reports, text_smoke, workflows
from exoharness_benchmark.verifiers import inspect_artifact, verify
from exoharness_benchmark.selection import review
from exoharness_benchmark.comparison import RuntimeComparison
from exoharness_benchmark.acceptance import Acceptance


def candidate(suite):
    return {'version': 1, 'suite_sha256': suite.sha256, 'rules': [
        {'group': 'math', 'sources': ['train_math_0'], 'when': 'Only a units digit changed.',
         'action': 'Find the digit difference, divide the product difference by it, then use the original multiplier.',
         'unless': 'The changed position or difference is unknown; preserve the question and clarify.'}]}


class ContractsTests(unittest.TestCase):
    def setUp(self):
        self.suite = text_smoke()

    def test_current_report_suite_preserves_exact_incident_and_does_not_supply_sources(self):
        suite = current_reports()
        incident = next(t for t in suite.value['tasks'] if t['split'] == 'regression')
        self.assertEqual(incident['turns'], ['写一个当前中国经济现状的PPT'])
        self.assertNotIn('slides', incident['checks'][0])
        self.assertTrue(all('http' not in p for t in suite.value['tasks'] for p in t['turns']))
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(create(Path(directory) / 'reports', 'current-reports').sha256, suite.sha256)

    def test_current_report_requires_real_successful_retrieval_not_loaded_schema_or_refused_call(self):
        task=current_reports().value['tasks'][1]
        outcome={'state':'succeeded','text':'qualitative only','tool_names':['search_capabilities','mcp_tavily_search'],
                 'artifacts':[{'extension':'.pptx','nonempty':True,'text':'中国经济'}]}
        self.assertFalse(verify(task,outcome)['delivered'])
        outcome['successful_tool_names']=['mcp_tavily_search']
        self.assertTrue(verify(task,outcome)['delivered'])
        for minimum in [-1,True,9]:
            value=current_reports().value;value['tasks'][1]['checks'][1]['min']=minimum
            with self.assertRaises(ValueError):Suite.parse(value)

    def test_suite_identity_is_immutable_and_order_independent(self):
        original = self.suite.value
        frozen = Suite.parse(original)
        original['tasks'][0]['turns'][0] = 'modified'
        self.assertEqual(frozen.sha256, self.suite.sha256)
        self.assertEqual(Suite.parse(json.loads(frozen.canonical)).sha256, frozen.sha256)

    def test_learner_never_receives_held_out_prompt_or_answer(self):
        value = self.suite.value
        value['tasks'][-1]['turns'][0] += ' HELD_OUT_SECRET_SENTINEL'
        value['tasks'][-1]['checks'][0]['fields']['secret'] = 'UNSEEN_REFERENCE'
        prompt = learning_prompt(Suite.parse(value))
        self.assertNotIn('HELD_OUT_SECRET_SENTINEL', prompt)
        self.assertNotIn('UNSEEN_REFERENCE', prompt)
        self.assertIn('train_math_0', prompt)

    def test_reviewed_feedback_is_scoped_to_the_exact_train_input(self):
        value = self.suite.value
        task = value['tasks'][0]
        task['evidence'] = {'input_sha256': digest(task['turns']), 'source_trace_sha256': 'a' * 64,
            'state': 'failed', 'verified_delivery': False, 'failure_classes': ['input_invalid']}
        prompt = learning_prompt(Suite.parse(value))
        self.assertIn('input_invalid', prompt)
        task['evidence']['input_sha256'] = 'b' * 64
        with self.assertRaisesRegex(ValueError, 'input-matched'):
            Suite.parse(value)

    def test_benchmark_logic_is_in_an_independent_module(self):
        root = Path(__file__).resolve().parents[2]
        module = root / 'modules/benchmark/src/exoharness_benchmark'
        for name in ('contracts', 'ports', 'evidence', 'experiment', 'harness', 'metrics', 'verifiers', 'scenarios'):
            self.assertTrue((module / (name + '.py')).is_file())
        self.assertNotIn('from .harness', (module / 'experiment.py').read_text())
        for name in ('evaluate-offline.py', 'create-evaluation-suite.py'):
            self.assertNotIn('class Experiment', (root / 'scripts' / name).read_text())

    def test_duplicate_inputs_and_identities_are_rejected(self):
        for field in ('id', 'turns'):
            value = self.suite.value
            value['tasks'][-1][field] = value['tasks'][0][field]
            with self.assertRaises(ValueError):
                Suite.parse(value)

    def test_artifact_checks_do_not_shadow_task_identity(self):
        value = self.suite.value
        value['tasks'][0]['checks'] = [{'kind': 'artifact', 'extension': '.mp4', 'min_duration': 1}]
        value['tasks'][-1]['id'] = value['tasks'][0]['id']
        with self.assertRaisesRegex(ValueError, 'duplicate task identity'):
            Suite.parse(value)

    def test_missing_split_unsafe_id_empty_checks_and_excess_turns(self):
        variants = [lambda v: v['tasks'].pop(),
                    lambda v: v['tasks'][0].update(id='../private'),
                    lambda v: v['tasks'][0].update(checks=[]),
                    lambda v: v['tasks'][0].update(turns=['turn'] * 9)]
        for alter in variants:
            with self.subTest(alter=alter):
                value = self.suite.value
                alter(value)
                with self.assertRaises(ValueError):
                    Suite.parse(value)

    def test_experience_requires_matching_training_provenance(self):
        for field, replacement in [('sources', ['test_math_composition']), ('group', 'web'), ('unless', '')]:
            value = candidate(self.suite)
            value['rules'][0][field] = replacement
            with self.assertRaises(ValueError):
                Experience.parse(value, self.suite)

    def test_experience_is_scoped_and_bounded(self):
        value = candidate(self.suite)
        experience = Experience.parse(value, self.suite)
        self.assertEqual(experience.instructions('web'), '')
        self.assertIn('does not grant', experience.instructions('math'))
        value['rules'][0]['action'] = 'x' * 601
        with self.assertRaises(ValueError):
            Experience.parse(value, self.suite)

    def test_pinned_attachment_paths(self):
        for path in ('../auth.json', '/tmp/input.wav', 'inputs/../../auth.json'):
            value = self.suite.value
            value['tasks'][0]['attachments'] = [{'type': 'audio', 'path': path, 'sha256': '0' * 64}]
            with self.assertRaises(ValueError):
                Suite.parse(value)

    def test_workflow_matrix_has_six_groups_and_unseen_rule_combinations(self):
        suite = workflows('a' * 64)
        self.assertEqual(len(suite.value['tasks']), 62)
        video = next(t for t in suite.value['tasks'] if t['id'] == 'regression_video_repo_endcard')
        self.assertEqual(video['turns'], ['为开源的ExoHarness生成15S宣传视频，开源地址，https://github.com/jason571/Exoharness\n视频最后必须出现地址链接'])
        self.assertTrue(video['review_required'])
        self.assertEqual(len(suite.training_material()), 30)
        self.assertEqual(len({t['group'] for t in suite.value['tasks']}), 6)
        for group in suite.value['rules']:
            tasks = [t for t in suite.value['tasks'] if t['group'] == group]
            train = {tuple(t['rule_ids']) for t in tasks if t['split'] == 'train'}
            test = {tuple(t['rule_ids']) for t in tasks if t['split'] == 'test'}
            self.assertTrue(train.isdisjoint(test))

    def test_repeated_or_unseen_rule_in_holdout_is_rejected(self):
        value = workflows('a' * 64).value
        train = next(t for t in value['tasks'] if t['split'] == 'train')
        test = next(t for t in value['tasks'] if t['split'] == 'test')
        test['rule_ids'] = train['rule_ids']
        with self.assertRaisesRegex(ValueError, 'recombine'):
            Suite.parse(value)

    def test_misspelled_check_fields_and_invalid_ranges_are_rejected(self):
        checks = [dict(kind='artifact', extension='.mp4', min_duraton=15),
                  dict(kind='artifact', extension='.mp4', min_duration=16, max_duration=10),
                  dict(kind='artifact', extension='.pptx', slides=True),
                  dict(kind='artifact', extension='.mp4', audio='yes')]
        for check in checks:
            with self.subTest(check=check):
                value = self.suite.value
                value['tasks'][-1]['checks'] = [check]
                with self.assertRaises(ValueError):
                    Suite.parse(value)

    def test_workflow_audio_is_required_and_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaises(ValueError):
                create(root / 'missing', 'workflows')
            audio = root / 'owned.wav'
            with wave.open(str(audio), 'wb') as stream:
                stream.setnchannels(1)
                stream.setsampwidth(2)
                stream.setframerate(8000)
                stream.writeframes(b'\0\0' * 800)
            before = audio.read_bytes()
            suite = create(root / 'suite', 'workflows', audio)
            self.assertEqual((root / 'suite/inputs/existing.wav').read_bytes(), before)
            self.assertEqual(audio.read_bytes(), before)
            self.assertEqual(suite.sha256, Suite.parse(json.loads((root / 'suite/suite.json').read_text())).sha256)


class MetricsTests(unittest.TestCase):
    def call(self, incoming=100, outgoing=20, **extra):
        return {'usage_reported': True, 'usage': {'input_tokens': incoming, 'output_tokens': outgoing, **extra}}

    def test_cache_is_part_of_input_not_added_again(self):
        result = usage([self.call(cache_read_tokens=80)])
        self.assertEqual(result['total_tokens'], 120)
        self.assertEqual(result['cached_input_tokens'], 80)

    def test_unknown_failed_usage_is_not_zero(self):
        for call in ({'usage_reported': False, 'usage': {'input_tokens': 0, 'output_tokens': 0}},
                     self.call(incoming=None), self.call(incoming=True), self.call(total_tokens=999)):
            self.assertEqual(usage([call])['unknown_model_calls'], 1)

    def test_failed_requests_with_reported_usage_are_counted(self):
        call = self.call()
        call['status'] = 'failed'
        self.assertEqual(usage([call])['total_tokens'], 120)

    def rows(self):
        return [{'mode': mode, 'task_id': 'task', 'attempt': attempt, 'tokens': 100 if mode == 'base' else 60,
                 'delivered': True, 'state': 'succeeded', 'run_ids': [f'{mode}-{attempt}'],
                 'runtime_fingerprint': 'fixed'} for attempt in range(3) for mode in ('base', 'candidate')]

    def decision(self, rows=None, learner=10, repeats=3):
        return retain(rows or self.rows(), expected_tasks=['task'], repeats=repeats, learner_tokens=learner)

    def test_measured_cost_gain_includes_learning(self):
        self.assertTrue(self.decision()['retain'])
        self.assertFalse(self.decision(learner=300)['retain'])

    def test_failure_tokens_remain_in_denominator(self):
        result = aggregate([{'tokens': 100, 'delivered': False}, {'tokens': 20, 'delivered': True}])
        self.assertEqual(result['tokens_per_delivery'], 120)
        self.assertIsNone(aggregate([{'tokens': None, 'delivered': True}])['tokens_per_delivery'])

    def test_incomplete_duplicate_unknown_drift_and_review_block_retention(self):
        variants = [('incomplete_or_unexpected_matrix', lambda rows: rows.pop()),
                    ('duplicate_attempt', lambda rows: rows.append(copy.deepcopy(rows[0]))),
                    ('unknown_usage', lambda rows: rows[0].update(tokens=None)),
                    ('runtime_drift', lambda rows: rows[0].update(runtime_fingerprint='new')),
                    ('delivery_review_pending', lambda rows: rows[0].update(review_pending=True)),
                    ('reused_run', lambda rows: rows[0].update(run_ids=rows[1]['run_ids'])),
                    ('task_regression', lambda rows: rows[1].update(delivered=False))]
        for reason, alter in variants:
            with self.subTest(reason=reason):
                rows = self.rows()
                alter(rows)
                decision = self.decision(rows)
                self.assertFalse(decision['retain'])
                self.assertIn(reason, decision['reasons'])

    def test_one_repeat_cannot_be_presented_as_validated_improvement(self):
        self.assertIn('insufficient_repeats', self.decision(repeats=1)['reasons'])

    def test_declared_runtime_pair_still_rejects_drift(self):
        rows = self.rows()
        for row in rows:
            row['runtime_fingerprint'] = row['mode']
        args = dict(expected_tasks=['task'], repeats=3, learner_tokens=0,
                    expected_runtimes={'base': 'base', 'candidate': 'candidate'})
        self.assertTrue(retain(rows, **args)['retain'])
        self.assertIn('runtime_drift', self.decision(rows)['reasons'])
        rows[0]['runtime_fingerprint'] = 'unexpected'
        self.assertIn('runtime_drift', retain(rows, **args)['reasons'])

    def test_latency_requires_complete_observations_and_includes_failures(self):
        rows = [{'tokens': 5, 'delivered': True, 'elapsed_ms': 100},
                {'tokens': 30, 'delivered': False, 'elapsed_ms': 900}]
        result = aggregate(rows)
        self.assertEqual(result['mean_elapsed_ms'], 500)
        self.assertEqual(result['p95_elapsed_ms'], 900)
        rows[1]['elapsed_ms'] = None
        self.assertIsNone(aggregate(rows)['mean_elapsed_ms'])


class VerifierTests(unittest.TestCase):
    def test_success_status_alone_is_not_delivery(self):
        task = {'checks': [{'kind': 'artifact', 'extension': '.mp4'}]}
        self.assertFalse(verify(task, {'state': 'succeeded', 'artifacts': []})['delivered'])

    def test_json_preserves_integer_and_boolean_types(self):
        task = {'checks': [{'kind': 'json_equals', 'fields': {'product': 312}}]}
        for text, expected in [('```json\n{"product":312}\n```', True), ('{"product":"312"}', False), ('{"product":312.0}', False), ('invalid', False)]:
            self.assertEqual(verify(task, {'state': 'succeeded', 'text': text})['delivered'], expected)

    def test_failed_state_keeps_useful_answer_but_is_not_completed(self):
        task = {'checks': [{'kind': 'text_contains', 'values': ['312']}]}
        self.assertFalse(verify(task, {'state': 'failed', 'text': '312'})['delivered'])

    def test_mp4_requires_real_video_audio_and_duration(self):
        task = {'checks': [{'kind': 'artifact', 'extension': '.mp4', 'min_duration': 14, 'max_duration': 16, 'audio': True}]}
        item = {'extension': '.mp4', 'nonempty': True, 'video': True, 'audio': True, 'duration': 15}
        self.assertTrue(verify(task, {'state': 'succeeded', 'artifacts': [item]})['delivered'])
        for field, value in [('video', False), ('audio', False), ('duration', 5)]:
            self.assertFalse(verify(task, {'state': 'succeeded', 'artifacts': [{**item, field: value}]})['delivered'])

    def test_project_zip_counts_as_html_delivery(self):
        data = io.BytesIO()
        with zipfile.ZipFile(data, 'w') as archive:
            archive.writestr('index.html', '<html>ExoHarness Rust</html>')
        artifact = inspect_artifact('project.zip', data.getvalue())
        task = {'checks': [{'kind': 'artifact', 'extension': '.zip', 'contains': ['ExoHarness', 'Rust']}]}
        self.assertTrue(verify(task, {'state': 'succeeded', 'artifacts': [artifact]})['delivered'])

    def test_unsafe_archive_and_bad_html_are_not_success(self):
        data = io.BytesIO()
        with zipfile.ZipFile(data, 'w') as archive:
            archive.writestr('../auth.json', '{}')
        with self.assertRaises(ValueError):
            inspect_artifact('project.zip', data.getvalue())
        self.assertFalse(inspect_artifact('page.html', b'not HTML')['valid_html'])


class DeliveryReviewTests(unittest.TestCase):
    def report(self):
        rows = MetricsTests().rows()
        for row in rows:
            row.update(key=f"{row['mode']}-{row['attempt']}", review_pending=True)
        return {'repeats': 3, 'learning_tokens': 10, 'rows': rows,
                'decision': retain(rows, expected_tasks=['task'], repeats=3, learner_tokens=10)}

    def test_report_bound_review_can_complete_local_selection_without_mutating_source(self):
        report = self.report()
        evidence = {'report_sha256': digest(report), 'reviewer': 'QA reviewer',
            'decisions': [{'key': row['key'], 'passed': True} for row in report['rows']]}
        result = review(report, evidence)
        self.assertTrue(result['decision']['retain'])
        self.assertFalse(report['decision']['retain'])
        self.assertFalse(result['delivery_review']['production_imported'])

    def test_missing_or_foreign_review_does_not_approve(self):
        report = self.report()
        evidence = {'report_sha256': digest(report), 'reviewer': 'QA reviewer', 'decisions': []}
        self.assertFalse(review(report, evidence)['decision']['retain'])
        evidence['report_sha256'] = 'wrong'
        with self.assertRaises(ValueError):
            review(report, evidence)

    def test_review_cannot_override_structural_failure_or_runtime_drift(self):
        report = self.report()
        report['rows'][1]['delivered'] = False
        report['decision']['reasons'].append('model_binding_drift')
        evidence = {'report_sha256': digest(report), 'reviewer': 'QA reviewer',
            'decisions': [{'key': row['key'], 'passed': True} for row in report['rows']]}
        result = review(report, evidence)
        self.assertFalse(result['decision']['retain'])
        self.assertFalse(result['rows'][1]['delivered'])
        self.assertIn('model_binding_drift', result['decision']['reasons'])

    def test_review_keeps_declared_runtime_pair_and_missing_task_rejection(self):
        report = self.report()
        report['runtime_fingerprints'] = {'base': 'old', 'candidate': 'new'}
        for row in report['rows']:
            row['runtime_fingerprint'] = report['runtime_fingerprints'][row['mode']]
        evidence = lambda: {'report_sha256': digest(report), 'reviewer': 'QA reviewer',
            'decisions': [{'key': row['key'], 'passed': True} for row in report['rows']]}
        self.assertTrue(review(report, evidence())['decision']['retain'])
        report['expected_tasks'] = ['task', 'missing_entire_task']
        self.assertIn('incomplete_or_unexpected_matrix', review(report, evidence())['decision']['reasons'])
        del report['expected_tasks']
        report['decision']['reasons'].append('incomplete_or_unexpected_matrix')
        self.assertFalse(review(report, evidence())['decision']['retain'])


class FakePort:
    def __init__(self, suite, *, unknown=False, drift=False):
        self.suite, self.calls, self.unknown, self.drift = suite, [], unknown, drift

    def fingerprint(self):
        return 'fixed'

    def execute(self, directory, prompts, fingerprint, attachments=()):
        target = Path(directory) / 'outcome.json'
        if target.exists():
            return json.loads(target.read_text())
        self.calls.append((str(directory), prompts))
        learner = Path(directory).name == 'learner'
        result = {'state': 'succeeded', 'text': json.dumps(candidate(self.suite)) if learner else '{"product":312}',
                  'tokens': None if self.unknown else (10 if learner else 20), 'run_ids': [Path(directory).name],
                  'tool_names': [], 'runtime_fingerprint': fingerprint, 'runtime_drift': self.drift,
                  'model_bindings': [['fixture', 'fixture-model']], 'artifacts': []}
        write_private(target, result)
        return result


class ExperimentTests(unittest.TestCase):
    def test_full_matrix_persisted_and_restart_does_not_resubmit(self):
        suite = text_smoke()
        port = FakePort(suite)
        with tempfile.TemporaryDirectory() as tmp:
            experiment = Experiment(suite, port, tmp, max_tokens=10000, run_reservation=100)
            first = experiment.execute()
            self.assertEqual(len(port.calls), 7)
            self.assertEqual(len(first['rows']), 6)
            self.assertEqual(first['total_charged_tokens'], 130)
            self.assertFalse(first['decision']['retain'])
            self.assertEqual(experiment.execute(), first)
            self.assertEqual(len(port.calls), 7)
            self.assertEqual((Path(tmp) / 'candidate.json').stat().st_mode & 0o077, 0)

    def test_budget_blocks_before_first_submission(self):
        suite, port = text_smoke(), FakePort(text_smoke())
        with tempfile.TemporaryDirectory() as tmp:
            experiment = Experiment(suite, port, tmp, max_tokens=100, run_reservation=100)
            with self.assertRaisesRegex(RuntimeError, 'budget exhausted'):
                experiment.execute()
            self.assertEqual(len(port.calls), 1)

    def test_unknown_usage_keeps_reservation_and_stops(self):
        suite = text_smoke()
        port = FakePort(suite, unknown=True)
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(RuntimeError, 'usage is unknown'):
                Experiment(suite, port, tmp, max_tokens=1000, run_reservation=100).execute()
            self.assertEqual(len(port.calls), 1)
            state = json.loads((Path(tmp) / 'experiment.json').read_text())
            self.assertEqual(state['charges']['learner'], 100)

    def test_runtime_drift_stops_without_next_submission(self):
        suite = text_smoke()
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(RuntimeError, 'Runtime changed'):
                Experiment(suite, FakePort(suite, drift=True), tmp, max_tokens=1000, run_reservation=100).execute()

    def test_changed_suite_or_candidate_refuses_existing_experiment(self):
        suite = text_smoke()
        with tempfile.TemporaryDirectory() as tmp:
            Experiment(suite, FakePort(suite), tmp, max_tokens=10000, run_reservation=100).execute()
            path = Path(tmp) / 'candidate.json'
            changed = json.loads(path.read_text())
            changed['rules'][0]['action'] = 'A new procedure'
            write_private(path, changed)
            with self.assertRaisesRegex(ValueError, 'frozen learner output'):
                Experiment(suite, FakePort(suite), tmp, max_tokens=10000, run_reservation=100).execute()

    def test_attachment_replacement_and_escape_are_rejected(self):
        suite = text_smoke()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'inputs').mkdir()
            (root / 'inputs/owned.wav').write_bytes(b'changed')
            task = {'attachments': [{'type': 'audio', 'path': 'inputs/owned.wav', 'sha256': hashlib.sha256(b'original').hexdigest()}]}
            engine = Experiment(suite, FakePort(suite), root / 'out', input_root=root)
            with self.assertRaisesRegex(ValueError, 'content changed'):
                engine.attachments(task)
            (root / 'inputs/owned.wav').unlink()
            (root / 'inputs/owned.wav').symlink_to('/etc/passwd')
            with self.assertRaisesRegex(ValueError, 'outside'):
                engine.attachments(task)


class ComparisonPort(FakePort):
    run_ceiling = 100

    def __init__(self, suite, mode, order, **kwargs):
        super().__init__(suite, **kwargs)
        self.mode, self.order = mode, order
        self.configuration_id = 'identical-configuration'

    def fingerprint(self):
        return self.mode

    def configuration_fingerprint(self):
        return self.configuration_id

    def execute(self, directory, prompts, fingerprint, attachments=()):
        self.order.append(self.mode)
        return super().execute(directory, prompts, fingerprint, attachments)


class RuntimeComparisonTests(unittest.TestCase):
    def ports(self, **kwargs):
        suite, order = text_smoke(), []
        return suite, ComparisonPort(suite, 'base', order), ComparisonPort(suite, 'candidate', order, **kwargs), order

    def test_paired_inputs_order_accounting_and_restart(self):
        suite, base, candidate_port, order = self.ports()
        with tempfile.TemporaryDirectory() as tmp:
            runner = RuntimeComparison(suite, base, candidate_port, tmp, max_tokens=10000, run_reservation=100)
            report = runner.execute()
            self.assertEqual(order, ['base', 'candidate', 'candidate', 'base', 'base', 'candidate'])
            self.assertEqual([p for _, p in base.calls], [p for _, p in candidate_port.calls])
            self.assertEqual(base.calls[0][1], suite.value['tasks'][-1]['turns'])
            self.assertEqual(report['learning_tokens'], 0)
            self.assertEqual(report['total_charged_tokens'], 120)
            self.assertNotIn('runtime_drift', report['decision']['reasons'])
            self.assertEqual(runner.execute(), report)
            self.assertEqual(len(order), 6)

    def test_different_configuration_stops_before_any_call(self):
        suite, base, candidate_port, order = self.ports()
        candidate_port.configuration_id = 'changed-permissions'
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(ValueError, 'identical'):
                RuntimeComparison(suite, base, candidate_port, tmp, run_reservation=100).execute()
        self.assertEqual(order, [])

    def test_unknown_usage_or_drift_stops_and_keeps_evidence(self):
        for options in ({'unknown': True}, {'drift': True}):
            suite, base, candidate_port, order = self.ports(**options)
            with tempfile.TemporaryDirectory() as tmp:
                with self.assertRaisesRegex(RuntimeError, 'Uncertain'):
                    RuntimeComparison(suite, base, candidate_port, tmp, run_reservation=100).execute()
                state = json.loads((Path(tmp) / 'comparison.json').read_text())
                self.assertEqual(order, ['base', 'candidate'])
                self.assertEqual(len(state['rows']), 1)
                self.assertEqual(state['charges']['candidate-test_math_composition-0'], 100 if options.get('unknown') else 20)

    def test_insufficient_reservation_prevents_next_submission(self):
        suite, base, candidate_port, order = self.ports()
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(RuntimeError, 'budget exhausted'):
                RuntimeComparison(suite, base, candidate_port, tmp, max_tokens=100, run_reservation=100).execute()
            self.assertEqual(order, ['base'])


class AcceptanceTests(unittest.TestCase):
    def test_known_failure_is_preserved_and_not_repeated_until_diagnosed(self):
        suite=text_smoke();port=FakePort(suite);original=port.execute
        def bad(*args,**kwargs):
            result=original(*args,**kwargs);result['text']='incorrect answer'
            write_private(Path(args[0])/'outcome.json',result)
            return result
        port.execute=bad
        with tempfile.TemporaryDirectory() as tmp:
            report=Acceptance(suite,port,tmp,repeats=2,max_tokens=1000,run_reservation=100).execute()
            self.assertEqual(len(port.calls),1)
            self.assertEqual(report['expected_attempts'],2)
            self.assertFalse(report['all_structural_checks_passed'])
            self.assertEqual(report['total_charged_tokens'],20)
    def test_repeated_fresh_sessions_have_no_learner_or_hidden_retry_and_resume_is_readonly(self):
        suite=text_smoke(); port=FakePort(suite)
        with tempfile.TemporaryDirectory() as tmp:
            runner=Acceptance(suite,port,tmp,repeats=2,max_tokens=1000,run_reservation=100)
            report=runner.execute()
            self.assertEqual(len(report['rows']),2)
            self.assertEqual(len(port.calls),2)
            self.assertTrue(report['all_structural_checks_passed'])
            self.assertEqual(report['total_charged_tokens'],40)
            self.assertEqual(report['metrics']['first_turn_deliveries'],2)
            self.assertEqual(runner.execute(),report)
            self.assertEqual(len(port.calls),2)

    def test_acceptance_unknown_usage_stops_before_next_scenario(self):
        suite=text_smoke();port=FakePort(suite,unknown=True)
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(RuntimeError,'unknown usage'):
                Acceptance(suite,port,tmp,repeats=2,max_tokens=1000,run_reservation=100).execute()
            self.assertEqual(len(port.calls),1)
            self.assertEqual(sum(json.loads((Path(tmp)/'campaign.json').read_text())['charges'].values()),100)

    def test_observed_overrun_is_fully_charged_before_stopping(self):
        suite, base, candidate_port, order = RuntimeComparisonTests().ports()
        original = base.execute
        def overrun(*args, **kwargs):
            result = original(*args, **kwargs)
            result['tokens'] = 150
            return result
        base.execute = overrun
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(RuntimeError, 'actual cost retained'):
                RuntimeComparison(suite, base, candidate_port, tmp, max_tokens=1000, run_reservation=100).execute()
            state = json.loads((Path(tmp) / 'comparison.json').read_text())
            self.assertEqual(state['charges']['base-test_math_composition-0'], 150)
            self.assertEqual(order, ['base'])


class AdapterContractTests(unittest.TestCase):
    def port(self):
        port = object.__new__(Harness)
        port.auth = {'assistant_id': 'test'}
        port.timeout, port.run_ceiling = 5, 100
        return port

    def test_origin_contract_rejects_remote_plaintext_and_embedded_credentials(self):
        self.assertEqual(origin('https://agent.flyang.art/'), 'https://agent.flyang.art')
        for value in ('http://example.com', 'https://secret@example.com', 'https://example.com/path', 'file:///etc/passwd'):
            with self.assertRaises(ValueError):
                origin(value)

    def test_ambiguous_session_is_not_reposted(self):
        with tempfile.TemporaryDirectory() as tmp:
            prompts = ['original']
            write_private(Path(tmp) / 'execution.json', {'session_intent': True, 'turns': [],
                'input_sha256': digest({'prompts': prompts, 'attachments': ()})})
            port = self.port()
            port.api = lambda *args, **kwargs: self.fail('Must not resubmit an ambiguous Session')
            with self.assertRaisesRegex(RuntimeError, 'Session submission unknown'):
                port.execute(tmp, prompts, 'fixed')

    def test_ambiguous_turn_is_not_reposted(self):
        with tempfile.TemporaryDirectory() as tmp:
            prompts = ['original']
            write_private(Path(tmp) / 'execution.json', {'session_id': 'owned', 'turns': [{'intent': True, 'prompt_sha256': digest('original')}],
                'input_sha256': digest({'prompts': prompts, 'attachments': ()})})
            port = self.port()
            port.api = lambda *args, **kwargs: self.fail('Must not resubmit an ambiguous Turn')
            with self.assertRaisesRegex(RuntimeError, 'Turn submission unknown'):
                port.execute(tmp, prompts, 'fixed')

    def test_opt_in_precedes_auth_or_output_access(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / 'never-created'
            result = subprocess.run([sys.executable, str(Path(__file__).resolve().parents[1] / 'evaluate-offline.py'),
                'run', '--suite', '/missing/suite', '--auth-file', '/missing/secret', '--output', str(output)],
                capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 2)
            self.assertIn('requires --allow-model-requests', result.stderr)
            self.assertFalse(output.exists())

    def test_actual_http_adapter_and_experiment_preserve_role_and_resume_boundaries(self):
        suite = text_smoke()
        sessions, traces, requests = {}, {}, []

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def respond(self, value):
                body = json.dumps(value).encode()
                self.send_response(200)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def do_GET(self):
                requests.append(('GET', self.path, dict(self.headers)))
                if self.path == '/artifact.html':
                    body = b'<html>ExoHarness Rust</html>'
                    self.send_response(200)
                    self.end_headers()
                    self.wfile.write(body)
                elif self.path == '/redirect':
                    self.send_response(302)
                    self.send_header('Location', '/artifact.html')
                    self.end_headers()
                elif self.path == '/v1/account':
                    self.respond({'account': {'id': 'exo_eval_fixture'}})
                elif self.path == '/v1/agent-profile':
                    self.respond({'id': 'test', 'revision': 1})
                elif self.path in ('/v1/model-catalog', '/v1/capabilities'):
                    if self.headers.get('Authorization') != 'Bearer fixture-admin':
                        self.send_error(403)
                        return
                    self.respond([])
                elif self.path.startswith('/v1/trace/runs/'):
                    self.respond(traces[self.path.split('/')[-1].split('?')[0]])
                elif self.path.startswith('/v1/sessions/'):
                    self.respond(sessions[self.path.split('/')[-1].split('?')[0]])
                else:
                    self.send_error(404)

            def do_POST(self):
                requests.append(('POST', self.path, dict(self.headers)))
                body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                if self.headers.get('Cookie') != 'fixture-login' or self.headers.get('x-exoharness-csrf') != 'fixture-csrf':
                    self.send_error(403)
                    return
                if self.path == '/v1/sessions':
                    sid = 's' + str(len(sessions))
                    sessions[sid] = {'id': sid, 'agent_profile': {'id': 'test'}, 'turns': []}
                    self.respond(sessions[sid])
                elif self.path.endswith('/turns'):
                    sid = self.path.split('/')[-2]
                    rid = 'r' + str(len(traces))
                    learner = 'TRAIN MATERIAL:' in body['prompt']
                    candidate_mode = 'Task-scoped candidate guidance' in body['prompt']
                    tokens = 10 if learner else 60 if candidate_mode else 100
                    text = json.dumps(candidate(suite)) if learner else '{"product":312}'
                    traces[rid] = {'run': {'id': rid, 'status': 'succeeded'}, 'model_calls': [
                        {'usage_reported': True, 'usage': {'input_tokens': tokens, 'output_tokens': 1,
                         'cache_read_tokens': None}, 'provider': 'fixture', 'model': 'fixture-model'}],
                        'invocations': [], 'events': []}
                    sessions[sid]['turns'].append({'run_id': rid, 'answer': text, 'response': {'text': text, 'artifacts': []}})
                    self.respond({'run_id': rid})
                else:
                    self.send_error(404)

        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            endpoint = 'http://127.0.0.1:' + str(server.server_port)
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                auth = root / 'auth.json'
                write_private(auth, {'portal_origin': endpoint, 'management_origin': endpoint,
                    'artifact_origin': endpoint, 'portal_headers': {'Cookie': 'fixture-login', 'x-exoharness-csrf': 'fixture-csrf'},
                    'management_headers': {'Authorization': 'Bearer fixture-admin'},
                    'user_id': 'exo_eval_fixture', 'assistant_id': 'test', 'owned_test_account': True,
                    'runtime_version': hashlib.sha256(Path('/proc/self/exe').read_bytes()).hexdigest(),
                    'runtime_binary': '/proc/self/exe', 'run_token_ceiling': 500})
                port = Harness(auth, run_ceiling=500)
                engine = Experiment(suite, port, root / 'experiment', run_reservation=500, max_tokens=10000)
                report = engine.execute()
                self.assertTrue(report['decision']['retain'])
                self.assertEqual(len(traces), 7)
                self.assertEqual(report['total_charged_tokens'], 497)
                self.assertEqual(engine.execute(), report)
                self.assertEqual(len(traces), 7)
                delivered = port.artifact({'download_url': endpoint + '/artifact.html', 'filename': 'page.html'}, root, 0)
                self.assertTrue(delivered['valid_html'])
                with self.assertRaisesRegex(ValueError, 'Redirect refused'):
                    port.artifact({'download_url': endpoint + '/redirect', 'filename': 'page.html'}, root, 1)
                with self.assertRaisesRegex(ValueError, 'outside'):
                    port.artifact({'download_url': 'https://example.invalid/private', 'filename': 'page.html'}, root, 2)
                for method, path, headers in requests:
                    if method == 'POST':
                        self.assertNotIn('Authorization', headers)
                    if path.startswith('/v1/trace') or path in ('/v1/model-catalog', '/v1/capabilities'):
                        self.assertNotIn('Cookie', headers)
                    if path in ('/artifact.html', '/redirect'):
                        self.assertNotIn('Cookie', headers)
                        self.assertNotIn('Authorization', headers)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)

    def test_runtime_probe_rejects_a_changed_binary_before_network(self):
        port = self.port()
        port.auth.update(runtime_binary='/proc/self/exe', runtime_version='0' * 64)
        port.api = lambda *args, **kwargs: self.fail('Binary drift must stop before network')
        with self.assertRaisesRegex(ValueError, 'binary changed'):
            port.fingerprint()

    def test_fingerprint_tracks_schema_validation_and_grants_at_the_same_revision(self):
        port = self.port()
        port.user_id = 'exo_eval_fixture'
        with open('/proc/self/exe', 'rb') as stream:
            binary_sha = hashlib.file_digest(stream, 'sha256').hexdigest()
        port.auth.update(runtime_binary='/proc/self/exe', runtime_version=binary_sha, assistant_id='test')
        records = [{'id':'mcp:test', 'revision':1, 'enabled':True, 'manifest':{'spec':{'kind':'mcp_server'}}},
                   {'id':'skill:test', 'revision':1, 'enabled':True, 'manifest':{'spec':{'kind':'skill'}}}]
        tools = [{'name':'echo','inputSchema':{'type':'object'}}]
        grants = [{'permission':'filesystem:read','decision':'allow'}]
        calls = []
        def api(path, payload=None, admin=False):
            self.assertIsNone(payload, 'Fingerprinting must remain read-only')
            calls.append(path)
            if path == '/v1/account': return {'id':port.user_id}
            if path == '/v1/agent-profile': return {'id':'test','revision':1}
            self.assertTrue(admin)
            if path == '/v1/capabilities': return records
            if path == '/v1/model-catalog': return []
            if path.endswith('/permissions'): return grants
            if path.endswith('/tools'): return tools
            self.fail('Unexpected fingerprint endpoint: '+path)
        port.api = api
        original = port.fingerprint()
        tools[0]['inputSchema']['description'] = 'Upstream validation changed only the cached schema'
        changed_schema = port.fingerprint()
        self.assertNotEqual(original, changed_schema)
        grants[0]['decision'] = 'deny'
        self.assertNotEqual(changed_schema, port.fingerprint())
        self.assertTrue(any('skill%3Atest/permissions' in path for path in calls))
        self.assertFalse(any('skill%3Atest/tools' in path for path in calls))
        tools.clear()
        self.assertNotEqual(original, port.fingerprint(), 'Unavailable discovery must change the fingerprint')


if __name__ == '__main__':
    unittest.main()
