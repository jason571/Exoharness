"""Native Rust evolution policy and instruction-package checks."""
import base64
import json
from rust_eval_test_support import NativeEvaluationTest


class NativeEvolutionTests(NativeEvaluationTest):
    def test_legacy_policy_identity_is_preserved(self):
        fixture = self.workflow()
        result = self.validate('evolution', 'policy', fixture['policy'])
        self.assertEqual(result['sha256'], fixture['expected_policy_sha256'])
        self.assertEqual(result['skill_id'], 'skill:global:_:experience-pricing')

    def test_policy_boundaries_and_authority(self):
        keys = ('max_rounds', 'max_tokens', 'run_reservation', 'repeats', 'interval_seconds',
                'max_tokens_per_delivery', 'max_rule_characters', 'monitor_min_samples',
                'monitor_max_tokens', 'max_training_characters')
        for key in keys:
            for value in (True, 0, -1, 100000001):
                with self.subTest(key=key, value=value):
                    policy = self.workflow()['policy']; policy[key] = value
                    self.validate('evolution', 'policy', policy, success=False)
        for key, value in (('method', True), ('method', 'unknown'), ('publish', 1),
                           ('scope', {'kind': 'global', 'user_id': 'other'}),
                           ('auto_suspend', True), ('skill_name', 'primary-workflow')):
            with self.subTest(key=key):
                policy = self.workflow()['policy']; policy[key] = value
                self.validate('evolution', 'policy', policy, success=False)

    def test_learning_skill_is_native_instruction_only(self):
        path = self.root / 'package.json'
        self.invoke('evolution', 'skill-package', '--output', path, no_path=True)
        package = json.loads(path.read_text())
        self.assertEqual(package['permissions'], [])
        self.assertEqual([f['path'] for f in package['files']], ['SKILL.md'])
        self.assertIn('workflow-evolution', base64.b64decode(package['files'][0]['content_base64']).decode())

    def test_learning_and_watch_require_explicit_model_opt_in(self):
        self.invoke('evolution', 'run', '--job', '/missing/job', '--state-root', self.root / 'state', success=False)
        self.invoke('evolution', 'watch', '--queue', '/missing/queue', '--state-root', self.root / 'state', '--max-tokens', '100', success=False)
        self.assertFalse((self.root / 'state').exists())
