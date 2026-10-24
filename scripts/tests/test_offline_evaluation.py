"""Rust benchmark binary acceptance; no paid model or media requests."""
import copy
import json
from rust_eval_test_support import NativeEvaluationTest


class NativeBenchmarkTests(NativeEvaluationTest):
    def test_suite_and_plan_need_no_python_on_path(self):
        root = self.root / 'suite'
        self.assertEqual(self.invoke('benchmark', 'suite', '--kind', 'text-smoke', '--output', root, no_path=True)['tasks'], 4)
        plan = self.invoke('benchmark', 'plan', '--suite', root / 'suite.json', no_path=True)
        self.assertEqual(plan['model_requests_submitted'], 0)
        self.assertEqual(plan['solver_attempts'], 6)

    def test_invalid_suite_identity_split_prompt_and_check_matrix(self):
        variants = {'id': ['', '../escape', 'A', True], 'split': ['unknown', True],
                    'turns': [[], [''], [True], ['x'] * 9], 'checks': [[], [{'kind': 'unknown'}]]}
        for key, values in variants.items():
            for value in values:
                with self.subTest(key=key, value=value):
                    suite = self.workflow()['suite']; suite['tasks'][0][key] = value
                    self.validate('benchmark', 'suite', suite, success=False)

    def test_check_types_ranges_unknown_fields_and_private_input_paths(self):
        checks = [{'kind': 'artifact', 'extension': '.exe'},
                  {'kind': 'artifact', 'extension': '.mp4', 'min_duration': True},
                  {'kind': 'artifact', 'extension': '.mp4', 'min_duration': 5, 'max_duration': 1},
                  {'kind': 'artifact', 'extension': '.pptx', 'slides': True},
                  {'kind': 'artifact', 'extension': '.mp4', 'audio': 1},
                  {'kind': 'tool_count', 'name_contains': 'lookup', 'min': 2, 'max': 1},
                  {'kind': 'tool_count', 'name_contains': 'lookup', 'max': True},
                  {'kind': 'text_contains', 'values': []}, {'kind': 'json_equals', 'fields': {}},
                  {'kind': 'artifact', 'extension': '.mp4', 'min_duraton': 1}]
        for check in checks:
            with self.subTest(check=check):
                suite = self.workflow()['suite']; suite['tasks'][0]['checks'] = [check]
                self.validate('benchmark', 'suite', suite, success=False)
        for path in ('/etc/passwd', 'inputs/../private', 'inputs/sub/file', 'inputs/'):
            with self.subTest(path=path):
                suite = self.workflow()['suite']
                suite['tasks'][0]['attachments'] = [{'type': 'file', 'path': path, 'sha256': 'a' * 64}]
                self.validate('benchmark', 'suite', suite, success=False)

    def test_learning_material_contains_only_train_tasks(self):
        result = self.validate('benchmark', 'suite', self.workflow()['suite'])
        self.assertEqual([t['id'] for t in result['training_material']], ['train_task'])
        for excluded in ('validation_task', 'test_task'):
            self.assertNotIn(excluded, json.dumps(result['training_material']))

    def test_cache_is_in_input_and_invalid_usage_is_unknown(self):
        call = {'usage_reported': True, 'usage': {'input_tokens': 100, 'output_tokens': 5, 'cache_read_tokens': 80}}
        self.assertEqual(self.validate('benchmark', 'usage', [call])['total_tokens'], 105)
        for key, values in {'input_tokens': [True, -1, None, '100'], 'output_tokens': [True, -1, None],
                            'cache_read_tokens': [True, '80', 101]}.items():
            for value in values:
                with self.subTest(key=key, value=value):
                    invalid = copy.deepcopy(call); invalid['usage'][key] = value
                    self.assertEqual(self.validate('benchmark', 'usage', [invalid])['unknown_model_calls'], 1)

    def test_json_integer_is_not_a_boolean_and_status_does_not_prove_delivery(self):
        task = {'checks': [{'kind': 'json_equals', 'fields': {'amount': 42}}]}
        for answer, passed in (('{"amount":42}', True), ('{"amount":true}', False), ('{"amount":42.0}', False)):
            result = self.validate('benchmark', 'verify', {'task': task, 'outcome': {'state': 'succeeded', 'text': answer}})
            self.assertEqual(result['delivered'], passed)
        task = {'checks': [{'kind': 'artifact', 'extension': '.mp4', 'audio': True, 'min_duration': 10}]}
        for artifact in ({}, {'extension': '.mp4', 'nonempty': True},
                         {'extension': '.mp4', 'nonempty': True, 'video': True, 'audio': False, 'duration': 15}):
            self.assertFalse(self.validate('benchmark', 'verify', {'task': task, 'outcome': {'state': 'succeeded', 'artifacts': [artifact]}})['delivered'])

    def test_model_opt_in_precedes_authentication_and_output(self):
        result = self.invoke('benchmark', 'run', '--suite', '/missing/suite', '--auth-file',
                             '/missing/secret', '--output', self.root / 'out', success=False)
        self.assertIn('requires --allow-model-requests', result.stderr)
        self.assertFalse((self.root / 'out').exists())
