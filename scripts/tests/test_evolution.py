"""Experience publication contracts, recovery and end-to-end fixture regression."""
import base64
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
from urllib.parse import urlsplit, unquote
for module in ('benchmark', 'evolution'):
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'modules' / module / 'src'))
from exoharness_benchmark.contracts import Suite, Experience, digest
from exoharness_benchmark.evidence import write_private
from exoharness_evolution.contracts import Policy
from exoharness_evolution.controller import Controller
from exoharness_evolution.ledger import Ledger, exclusive, consume_holdout
from exoharness_evolution.package import compile_package, package_digest, package_files, PROVENANCE
from exoharness_evolution.publisher import Publisher
from exoharness_evolution.learning import prompt
from exoharness_evolution.monitor import observe
from exoharness_evolution.queue import tick

SHA = 'a' * 64

def suite():
    return Suite.parse({'version': 1, 'id': 'evolution_fixture', 'tasks': [
        {'id': split + '_task', 'group': 'pricing', 'split': split,
         'turns': ['Compute a different business case: ' + split],
         'checks': [{'kind': 'json_equals', 'fields': {'amount': 42}}]}
        for split in ('train', 'validation', 'test')]})


def policy(s, **changes):
    return Policy.parse({'version': 1, 'id': 'pricing_v1', 'group': 'pricing',
        'skill_name': 'experience-pricing', 'description': 'Internal business pricing procedures',
        'scope': {'kind': 'global'}, 'baseline_binary_sha256': SHA, 'suite_sha256': s.sha256,
        'run_reservation': 100, 'max_tokens': 10000, 'supervision': 'fewshot', **changes})


def candidate(s):
    return Experience.parse({'version': 1, 'suite_sha256': s.sha256, 'rules': [
        {'group': 'pricing', 'sources': ['train_task'], 'when': 'Internal business pricing requested.',
         'action': 'Compute the applicable business amount using the confirmed multiplier.',
         'unless': 'Required values are missing; ask for them.'}]}, s)


class Management:
    def __init__(self, origin, profile=None):
        self.origin, self.record, self.files, self.calls = origin, None, [], []
        self.profile = profile or {'id': 'fixture', 'access': {'skills': {'mode': 'all'}}}
        self.lose_import = False; self.ready = True; self.sessions = []; self.runs = []; self.traces = {}

    def runtime(self):
        return SHA

    def configuration(self, assistant_id=None):
        return {'profile': copy.deepcopy(self.profile), 'models': {'models': []},
                'capabilities': [copy.deepcopy(self.record)] if self.record else [], 'capability_evidence': []}

    def api(self, path, payload=None, *, method=None, optional=False):
        self.calls.append((path, payload, method)); url = urlsplit(path); path = unquote(url.path)
        if path == '/v1/trace/sessions': return copy.deepcopy(self.sessions)
        if path == '/v1/trace/runs': return copy.deepcopy(self.runs)
        if path.startswith('/v1/trace/runs/'): return copy.deepcopy(self.traces[path.rsplit('/', 1)[1]])
        if path == '/v1/imports/skill':
            old = self.record
            assert payload.get('expected_revision') == (old['revision'] if old else None)
            name = package_files(payload)['SKILL.md'].decode().split('name: ')[1].split('\n')[0]
            self.record = {'id': 'skill:global:_:' + name, 'revision': old['revision'] + 1 if old else 1,
                'enabled': False, 'source_digest': package_digest(payload), 'manifest': {
                    'name': name, 'version': payload['version'], 'scope': payload['scope'], 'permissions': [],
                    'secret_refs': [], 'spec': {'kind': 'skill'}}}
            self.files = payload['files']
            if self.lose_import:
                self.lose_import = False
                raise ConnectionError('lost import response')
            return {'capabilities': [copy.deepcopy(self.record)]}
        if path.endswith('/skill-instructions'):
            return {'content': package_files({'files': self.files})['SKILL.md'].decode(), 'revision': self.record['revision']}
        if path.endswith('/skill-package'):
            return {'capability': copy.deepcopy(self.record), 'files': copy.deepcopy(self.files)}
        if path.endswith('/preparation'):
            if payload and self.ready:
                self.record['enabled'] = True; self.record['revision'] += 1
            return {'runtime_ready': self.ready and self.record['enabled']}
        if path.endswith('/enabled'):
            assert payload['expected_revision'] == self.record['revision']
            self.record['enabled'] = payload['enabled']; self.record['revision'] += 1
            return copy.deepcopy(self.record)
        if path.startswith('/v1/capabilities/'):
            return copy.deepcopy(self.record)
        raise AssertionError(path)


class Port:
    def __init__(self, management, s, *, fail=False, tokens=10, wrong_read=False):
        self.manager, self.suite, self.fail, self.tokens, self.wrong_read = management, s, fail, tokens, wrong_read
        self.auth = {'runtime_version': SHA, 'assistant_id': 'fixture'}
        self.portal = management.origin + '/portal'; self.management = management.origin
        self.run_ceiling = 100; self.calls = []

    def api(self, path, admin=False):
        return self.manager.profile

    def configuration(self):
        return self.manager.configuration()

    def fingerprint(self): return digest(self.configuration())
    def configuration_fingerprint(self): return self.fingerprint()

    def execute(self, directory, prompts, fingerprint, attachments=()):
        directory = Path(directory); file = directory / 'outcome.json'
        if file.exists(): return json.loads(file.read_text())
        self.calls.append(prompts)
        learner = prompts[0].startswith('Produce ONLY')
        text = candidate(self.suite).canonical if learner else json.dumps({'amount': 0 if self.fail or not self.manager.record else 42})
        record = self.manager.record
        value = {'state': 'failed' if self.fail and record else 'succeeded', 'text': text,
                 'tool_names': [], 'tokens': self.tokens, 'run_ids': [self.manager.origin + '-' + directory.name],
                 'runtime_fingerprint': fingerprint, 'runtime_drift': False, 'artifacts': [],
                 'model_bindings': [['fixture', 'model']], 'skill_reads': ([{'id': record['id'],
                 'revision': -1 if self.wrong_read else record['revision']}] if record and not learner else [])}
        write_private(file, value); return value


class EvolutionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.root = Path(self.temp.name)
        self.s = suite(); self.p = policy(self.s)
        self.prod = Management('https://publication.invalid')
        self.base = Management('https://baseline.invalid'); self.can = Management('https://canary.invalid')
        self.bp, self.cp = Port(self.base, self.s), Port(self.can, self.s)

    def tearDown(self): self.temp.cleanup()

    def controller(self, **kwargs):
        return Controller(kwargs.get('policy', self.p), self.s, self.bp, self.cp,
                          self.prod, self.can, self.root / 'campaign')

    def body(self, p=None, c=None):
        return compile_package(p or self.p, self.s, c or candidate(self.s), round_id='round-0', learner_run_ids=['learner'])

    def test_full_pipeline_three_pairs_per_partition_and_persistent_reuse(self):
        controller = self.controller(policy=policy(self.s, publish=True))
        state = controller.run()
        self.assertEqual(state['phase'], 'published')
        self.assertEqual(len(self.bp.calls), 7)  # learner + 3 validation + 3 held-out
        self.assertEqual(len(self.cp.calls), 6)
        self.assertEqual(sum(state['charges'].values()), 130)
        self.assertTrue(self.prod.record['enabled'])
        self.assertEqual(state['publications'][0]['revision'], self.prod.record['revision'])
        new_user = Port(self.prod, self.s)
        answer = new_user.execute(self.root / 'fresh', ['Compute a later business request'], new_user.fingerprint())
        self.assertEqual(answer['skill_reads'][0]['id'], self.p.skill_id)
        controller.run(); self.assertEqual(len(self.bp.calls), 7)

    def test_explicit_publish_and_crash_resume_are_idempotent(self):
        c = self.controller(); self.assertEqual(c.run()['phase'], 'validated')
        self.assertIsNone(self.prod.record)
        self.prod.lose_import = True
        with self.assertRaises(ConnectionError): c.publish()
        self.assertEqual(c.publish()['phase'], 'published')
        c.publish()
        self.assertEqual(sum(path == '/v1/imports/skill' for path, _, _ in self.prod.calls), 1)

    def test_candidate_failure_is_rejected_without_production_mutation(self):
        self.cp.fail = True
        result = self.controller(policy=policy(self.s, publish=True)).run()
        self.assertEqual(result['phase'], 'validation_rejected')
        self.assertIsNone(self.prod.record)
        self.assertEqual(len(self.cp.calls), 3)

    def test_wrong_read_revision_fails_the_gate(self):
        self.cp.wrong_read = True
        result = self.controller().run()
        self.assertEqual(result['phase'], 'validation_rejected')
        self.assertIn('experience_not_read_in_every_candidate_attempt', result['rounds'][0]['reason'])

    def test_budget_overrun_retains_actual_cost_and_stops(self):
        self.bp.tokens = 101
        with self.assertRaisesRegex(RuntimeError, 'overrun'): self.controller().run()
        self.assertEqual(len(self.bp.calls), 1)
        state = json.loads((self.root / 'campaign/evolution.json').read_text())
        self.assertEqual(sum(state['charges'].values()), 101)

    def test_unknown_usage_stops_with_reservation(self):
        self.bp.tokens = None
        with self.assertRaisesRegex(RuntimeError, 'Unknown usage'): self.controller().run()
        state = json.loads((self.root / 'campaign/evolution.json').read_text())
        self.assertEqual(sum(state['charges'].values()), 100)

    def test_cost_spike_rejected_even_when_accuracy_improves(self):
        self.cp.tokens = 90
        p = policy(self.s, max_tokens_per_delivery=20)
        result = self.controller(policy=p).run()
        self.assertEqual(result['phase'], 'validation_rejected')
        self.assertIn('delivery_token_ceiling', result['rounds'][0]['reason'])

    def test_actual_production_configuration_must_match(self):
        self.prod.profile['access']['skills']['mode'] = 'selected'
        with self.assertRaisesRegex(ValueError, 'differs'): self.controller().run()
        self.assertFalse(self.bp.calls)

    def test_unauthorized_target_cannot_be_granted_by_evolution(self):
        for manager in (self.prod, self.base, self.can): manager.profile['access']['skills'] = {'mode': 'selected', 'ids': []}
        with self.assertRaisesRegex(ValueError, 'not authorized'): self.controller().run()
        self.assertFalse(self.bp.calls); self.assertIsNone(self.prod.record)

    def test_unrelated_target_is_not_overwritten(self):
        self.can.record = {'id': self.p.skill_id, 'manifest': {'permissions': ['network'], 'secret_refs': []}}
        self.can.files = self.body()['files']; self.can.record['revision'] = 1
        with self.assertRaisesRegex(ValueError, 'unrelated'): self.controller().run()

    def test_external_revision_change_blocks_publication(self):
        c = self.controller(); c.run()
        self.prod.record = copy.deepcopy(self.can.record); self.prod.files = self.can.files
        self.prod.record['source_digest'] = 'c' * 64
        with self.assertRaisesRegex(ValueError, 'revision conflict'): c.publish()

    def test_import_reply_lost_reconciles_exact_digest(self):
        pub = Publisher(self.can, self.p); self.can.lose_import = True
        with self.assertRaises(ConnectionError): pub.publish(self.body(), self.root / 'pub', expected_revision=None)
        receipt = pub.publish(self.body(), self.root / 'pub', expected_revision=None)
        self.assertEqual(receipt['phase'], 'ready')
        self.assertEqual(sum(path == '/v1/imports/skill' for path, _, _ in self.can.calls), 1)

    def test_unknown_import_is_not_reposted(self):
        pub = Publisher(self.can, self.p)
        body = self.body()
        write_private(self.root / 'pub/publication.json', {'phase': 'importing', 'identity': {
            'skill_id': self.p.skill_id, 'source_digest': package_digest(body),
            'expected_revision': None, 'management_origin': self.can.origin}})
        with self.assertRaisesRegex(RuntimeError, 'unconfirmed'): pub.publish(body, self.root / 'pub', expected_revision=None)
        self.assertFalse(any(path == '/v1/imports/skill' for path, _, _ in self.can.calls))

    def test_preparation_failure_preserves_import_and_never_publishes(self):
        self.can.ready = False
        with self.assertRaisesRegex(ValueError, 'not ready'): self.controller().run()
        self.assertIsNone(self.prod.record)
        self.can.ready = True
        self.assertEqual(self.controller().run()['phase'], 'validated')

    def test_heldout_reuse_is_rejected_across_campaigns(self):
        root = self.root / 'shared'
        consume_holdout(root, self.s, 'pricing', 'a')
        consume_holdout(root, self.s, 'pricing', 'a')
        with self.assertRaisesRegex(ValueError, 'fresh test'): consume_holdout(root, self.s, 'pricing', 'b')

    def test_train_only_learning_never_receives_test_or_validation(self):
        text = prompt(self.p, self.s)
        self.assertNotIn('case: validation', text); self.assertNotIn('case: test', text)
        with self.assertRaises(ValueError): prompt(self.p, self.s, feedback=[{'train_id': 'test_task'}])
        text = prompt(policy(self.s, supervision='self'), self.s)
        self.assertNotIn('expected_delivery', text)

    def test_package_has_no_executable_or_permissions_and_bound_provenance(self):
        body = self.body(); files = package_files(body)
        self.assertEqual(set(files), {'SKILL.md', 'evolution/experience.json', PROVENANCE})
        self.assertEqual(body['permissions'], [])
        self.assertEqual(json.loads(files[PROVENANCE])['policy_sha256'], self.p.sha256)
        self.assertIn('grant no tools', files['SKILL.md'].decode())

    def test_private_handles_and_oversized_rule_are_rejected(self):
        for action in ('Download https://private.example/file', 'password=supersecret', 'Bearer value', 'session-123456789'):
            value = candidate(self.s).value; value['rules'][0]['action'] = action
            with self.subTest(action=action), self.assertRaises(ValueError): self.body(c=Experience.parse(value, self.s))
        value = candidate(self.s).value; value['rules'][0]['action'] = 'A' * 300
        with self.assertRaisesRegex(ValueError, 'context exceeds'):
            self.body(p=policy(self.s, max_rule_characters=200), c=Experience.parse(value, self.s))

    def test_malformed_policies_are_rejected(self):
        for change in ({'publish': 'yes'}, {'max_rounds': 0}, {'monitor_user_ids': [[]]}, {'run_reservation': True},
                       {'skill_name': 'original-workflow'}, {'baseline_binary_sha256': 'not-pinned'}, {'auto_suspend': True}):
            with self.subTest(change=change), self.assertRaises(ValueError): policy(self.s, **change)

    def test_lock_prevents_parallel_campaign_processes(self):
        with exclusive(self.root):
            with self.assertRaises(BlockingIOError):
                with exclusive(self.root): pass

    def test_symlink_workspace_is_refused(self):
        target = self.root / 'link'; target.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            with exclusive(target): pass

    def test_manual_review_is_required_when_declared(self):
        value = self.s.value
        for t in value['tasks']: t['review_required'] = True
        self.s = Suite.parse(value); self.p = policy(self.s); self.bp.suite = self.cp.suite = self.s
        result = self.controller().run()
        self.assertEqual(result['rounds'][0]['phase'], 'awaiting_validation_review')
        self.assertIsNone(self.prod.record)

    def test_monitor_uses_exact_revision_and_explicit_owners_only(self):
        c = self.controller(policy=policy(self.s, publish=True, monitor_user_ids=['test_owner'], monitor_min_samples=2, auto_suspend=True))
        c.run()
        rev = self.prod.record['revision']
        self.prod.sessions = [{'id': 'session_own', 'owner_user_id': 'test_owner'}, {'id': 'other', 'owner_user_id': 'other'}]
        self.prod.runs = [{'id': 'r1', 'status': 'failed'}, {'id': 'r2', 'status': 'failed'}, {'id': 'r3', 'status': 'failed'}]
        for index in range(1, 4):
            self.prod.traces['r' + str(index)] = {'model_calls': [{'usage_reported': True,
                'usage': {'input_tokens': 30, 'output_tokens': 10}}], 'invocations': [{'tool_name': 'read_skill',
                'arguments': {'id': self.p.skill_id}, 'result': {'revision': rev if index < 3 else rev - 1}}]}
        result = c.monitor(); self.assertEqual(result['samples'], 2)
        self.assertEqual(result['mean_tokens'], 40); self.assertTrue(result['suspended'])
        self.assertFalse(self.prod.record['enabled'])
        self.assertTrue(c.monitor()['suspended'])
        private = (c.root / 'evolution.json').read_text()
        self.assertNotIn('session_own', private)

    def test_no_observation_without_opt_in(self):
        c = self.controller(policy=policy(self.s, publish=True)); c.run()
        self.assertEqual(c.monitor()['status'], 'not_configured')

    def test_reflection_runs_training_verification_before_selection(self):
        result = self.controller(policy=policy(self.s, supervision='reflect')).run()
        self.assertEqual(result['phase'], 'validated')
        feedback = json.loads((self.root / 'campaign/round-0/train-feedback.json').read_text())
        self.assertTrue(feedback[0]['passed'])
        self.assertEqual(feedback[0]['train_id'], 'train_task')
        self.assertEqual(len(self.cp.calls), 7)

    def test_reflection_stops_when_candidate_does_not_change(self):
        self.cp.fail = True
        result = self.controller(policy=policy(self.s, supervision='reflect')).run()
        self.assertEqual(result['phase'], 'unchanged')
        self.assertEqual(len(self.bp.calls), 2)
        self.assertEqual(len(self.cp.calls), 1)
        self.assertIsNone(self.prod.record)

    def test_training_context_has_a_hard_limit(self):
        s = self.s.value; s['tasks'][0]['turns'][0] = 'TRAIN ' * 300
        s = Suite.parse(s)
        with self.assertRaisesRegex(ValueError, 'context exceeds'):
            prompt(policy(s, max_training_characters=1000), s)

    def test_tampered_report_cannot_be_published(self):
        c = self.controller(); c.run()
        path = c.root / 'round-0-test/evolution-report.json'
        report = json.loads(path.read_text()); report['rows'][0]['tokens'] += 1
        write_private(path, report)
        with self.assertRaisesRegex(ValueError, 'evidence'): c.publish()
        self.assertIsNone(self.prod.record)

    def test_second_campaign_reuses_exact_incumbent_package(self):
        c = self.controller(policy=policy(self.s, publish=True)); c.run()
        body = Publisher(self.prod, self.p).snapshot()
        self.assertEqual(package_digest(body), self.prod.record['source_digest'])
        changed = copy.deepcopy(self.prod.record); changed['manifest']['permissions'] = ['network']
        self.prod.record = changed
        with self.assertRaisesRegex(ValueError, 'unrelated'): Publisher(self.prod, self.p).snapshot()

    def test_queue_admission_and_failed_jobs_are_not_retried(self):
        queue = self.root / 'queue'; queue.mkdir(mode=0o700)
        state = self.root / 'queue-state'
        job = {'version': 1, 'id': 'job-one', 'policy': 'p', 'suite': 's', 'baseline_auth': 'b',
               'canary_auth': 'c', 'publication_auth': 'a', 'input_root': None}
        write_private(queue / 'one.json', job)
        calls = []
        class Broken:
            policy = self.p
            root = state / 'job-one'
            def run(inner): calls.append(1); raise ConnectionError('private API token not retained')
        def factory(job, root): return Broken()
        tick(queue, state, 9999, factory)
        self.assertFalse(calls)
        result = tick(queue, state, 9999, factory)
        self.assertFalse(result['jobs'])
        # An immutable queue limit cannot silently be increased after admission.
        with self.assertRaises(ValueError): tick(queue, state, 10000, factory)
        state2 = self.root / 'second-queue'
        tick(queue, state2, 10000, factory); tick(queue, state2, 10000, factory)
        self.assertEqual(len(calls), 1)
        self.assertNotIn('private API token', (state2 / 'queue.json').read_text())

    def test_review_rejection_never_turns_into_a_published_delivery(self):
        value = self.s.value
        for t in value['tasks']: t['review_required'] = True
        self.s = Suite.parse(value); self.p = policy(self.s); self.bp.suite = self.cp.suite = self.s
        c = self.controller(); c.run()
        path = c.root / 'round-0-validation/evolution-report.json'
        report = json.loads(path.read_text())
        write_private(path.parent / 'delivery-review.json', {'report_sha256': digest(report), 'reviewer': 'fixture',
            'decisions': [{'key': r['key'], 'passed': r['mode'] != 'candidate'} for r in report['rows']]})
        state = c.run()
        self.assertEqual(state['phase'], 'validation_rejected')
        self.assertIsNone(self.prod.record)

    def test_suspension_refuses_recreated_package_with_same_revision(self):
        c = self.controller(policy=policy(self.s, publish=True, monitor_user_ids=['test_owner'], monitor_min_samples=2, auto_suspend=True))
        c.run()
        ledger_path = c.root / 'evolution.json'; state = json.loads(ledger_path.read_text())
        revision = str(self.prod.record['revision'])
        state['monitor'][revision] = {'r1': {'succeeded':False,'tokens':10}, 'r2': {'succeeded':False,'tokens':10}}
        write_private(ledger_path, state)
        self.prod.record['source_digest'] = 'd' * 64
        with self.assertRaisesRegex(ValueError, 'unrelated package'): c.monitor()
        self.assertTrue(self.prod.record['enabled'])

    def test_queue_monitor_does_not_restart_published_evaluation(self):
        queue = self.root / 'queue'; queue.mkdir(mode=0o700)
        state = self.root / 'queue-state'
        job = {'version':1,'id':'completed','policy':'p','suite':'s','baseline_auth':'b',
               'canary_auth':'c','publication_auth':'a','input_root':None}
        write_private(queue / 'completed.json', job)
        calls = []
        class Completed:
            policy = self.p
            root = state / 'completed'
            def run(inner):
                calls.append('run'); write_private(inner.root / 'evolution.json', {'charges':{'one':100}})
                return {'phase':'published'}
            def monitor(inner): calls.append('monitor'); return {'status':'observing'}
        def factory(job, output): return Completed()
        with patch('exoharness_evolution.queue.time.time', return_value=10000):
            tick(queue, state, 10000, factory); tick(queue, state, 10000, factory)
        with patch('exoharness_evolution.queue.time.time', return_value=13601):
            tick(queue, state, 10000, factory)
        self.assertEqual(calls, ['run','monitor','monitor'])

    def test_user_cancellation_is_not_a_learning_regression(self):
        c = self.controller(policy=policy(self.s, publish=True, monitor_user_ids=['test_owner']))
        c.run()
        self.prod.sessions = [{'id':'own','owner_user_id':'test_owner'}]
        self.prod.runs = [{'id':'cancelled-by-user','status':'cancelled'}]
        # No Trace exists: cancelled Runs must be excluded before detail reads.
        self.assertEqual(c.monitor()['samples'], 0)

    def test_monitor_retains_newest_observations_by_run_time(self):
        c = self.controller(policy=policy(self.s, publish=True, monitor_user_ids=['test_owner']))
        c.run()
        path = c.root / 'evolution.json'; state = json.loads(path.read_text())
        revision = str(self.prod.record['revision'])
        # API order is newest first; insertion-order trimming would keep older
        # evidence, hiding precisely the regression the guard should observe.
        state['monitor'][revision] = {'r'+str(i):{'created_at':i,'succeeded':True,'tokens':1}
                                      for i in range(1001, 0, -1)}
        write_private(path, state); self.assertEqual(c.monitor()['samples'], 1000)
        kept = json.loads(path.read_text())['monitor'][revision]
        self.assertIn('r1001', kept); self.assertNotIn('r1', kept)


if __name__ == '__main__': unittest.main()
