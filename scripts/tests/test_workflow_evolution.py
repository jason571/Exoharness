"""Reviewed evidence and bounded workflow generation; fixtures are not live-model proof."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from test_evolution import suite, policy, Management, Port, candidate
from exoharness_benchmark.contracts import digest
from exoharness_benchmark.evidence import write_private
from exoharness_evolution.trajectories import Trajectories
from exoharness_evolution.workflows import WorkflowExperience
from exoharness_evolution.package import compile_package, package_files, validate_inventory
from exoharness_evolution.learning import prompt, extract, propose
from exoharness_evolution.controller import Controller
from exoharness_evolution.skill import package as skill_package


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.root = Path(self.temp.name)
        self.s = suite(); self.p = policy(self.s, method='workflow')
        self.evidence = self.root / 'evidence'; self.evidence.mkdir(mode=0o700)
        self.trace = {'run': {'id': 'fixture-run', 'session_id': 'owned-session', 'status': 'succeeded'},
            'prompt': self.s.value['tasks'][0]['turns'][0],
            'model_calls': [{'usage_reported': True, 'usage': {'input_tokens': 20, 'output_tokens': 5}}],
            'invocations': [{'tool_name': 'sandbox_run', 'outcome_status': 'success',
                'arguments': {'source': 'PRIVATE SOURCE password=PRIVATE'},
                'result': {'_host_execution': True, 'text': 'PRIVATE SOURCE'}}]}
        self.outcome = {'run_ids': ['fixture-run'], 'tokens': 25, 'state': 'succeeded',
            'turn_states': ['succeeded'], 'runtime_drift': False, 'text': '{"amount":42}', 'artifacts': []}
        self.sample = {'task_id': 'train_task', 'directory': 'sample', 'reviewer': 'qa',
            'owner_user_id': 'owner', 'delivery_passed': True}
        self.session = {'id': 'owned-session', 'owner_user_id': 'owner'}
        self.save()

    def tearDown(self):
        self.temp.cleanup()

    def save(self):
        write_private(self.evidence / 'sample/trace-0.json', self.trace)
        write_private(self.evidence / 'sample/outcome.json', self.outcome)
        write_private(self.evidence / 'sample/session.json', self.session)
        write_private(self.evidence / 'sample/execution.json', {'session_id': 'owned-session',
            'turns': [{'run_id': 'fixture-run', 'prompt_sha256': digest(self.s.value['tasks'][0]['turns'][0])}]})
        self.sample.update(outcome_sha256=digest(self.outcome), trace_sha256=[digest(self.trace)], session_sha256=digest(self.session))
        write_private(self.evidence / 'manifest.json', {'version': 1, 'suite_sha256': self.s.sha256, 'samples': [self.sample]})

    def corpus(self):
        return Trajectories(self.evidence, self.s, 'pricing')

    def value(self):
        return {'version': 2, 'suite_sha256': self.s.sha256, 'rules': [], 'workflows': [{
            'id': 'price-report', 'group': 'pricing', 'sources': ['train_task'],
            'when': 'A pricing report is requested.', 'unless': 'Inputs are missing.',
            'terms': ['pricing', '报价'], 'steps': [{'tool': 'sandbox_run', 'action': 'Compute the confirmed amount.',
                                                 'expect': 'A successful computation receipt.'}]}]}

    def test_projection_omits_raw_payloads_and_is_train_only(self):
        corpus = self.corpus(); text = prompt(self.p, self.s, trajectories=corpus)
        self.assertNotIn('PRIVATE SOURCE', text)
        self.assertNotIn('owned-session', text)
        self.assertNotIn('fixture-run', text)
        self.assertNotIn('test_task', text); self.assertNotIn('validation_task', text)
        self.assertIn('sandbox_run', text); self.assertIn('workflow-evolution', text)
        self.sample['task_id'] = 'test_task'; self.save()
        with self.assertRaisesRegex(ValueError, 'training'): self.corpus()

    def test_changed_trace_is_not_the_reviewed_evidence(self):
        self.trace['invocations'][0]['result']['text'] = 'changed'
        write_private(self.evidence / 'sample/trace-0.json', self.trace)
        with self.assertRaisesRegex(ValueError, 'review'): self.corpus()

    def test_owner_prompt_usage_and_terminal_binding(self):
        original_trace, original_outcome, original_session = copy.deepcopy(self.trace), copy.deepcopy(self.outcome), copy.deepcopy(self.session)
        for mutation in ('owner', 'prompt', 'usage', 'state', 'unknown'):
            self.trace, self.outcome, self.session = copy.deepcopy(original_trace), copy.deepcopy(original_outcome), copy.deepcopy(original_session)
            if mutation == 'owner': self.session['owner_user_id'] = 'other'
            if mutation == 'prompt': self.trace['prompt'] = 'different task'
            if mutation == 'usage': self.outcome['tokens'] = 1
            if mutation == 'state': self.outcome['state'] = 'failed'
            if mutation == 'unknown': self.trace['run']['status'] = 'running'
            self.save()
            with self.subTest(mutation=mutation), self.assertRaises(ValueError): self.corpus()

    def test_failed_execution_is_not_promoted_by_a_review(self):
        self.trace['run']['status'] = 'failed'; self.outcome.update(state='failed', turn_states=['failed'])
        self.save()
        with self.assertRaisesRegex(ValueError, 'failed execution'): self.corpus()
        self.sample['delivery_passed'] = False; self.save()
        corpus = self.corpus()
        with self.assertRaisesRegex(ValueError, 'successful deliveries'):
            WorkflowExperience.parse(self.value(), self.p, self.s, corpus)
        value = self.value(); value['workflows'] = []
        value['rules'] = [{**candidate(self.s).value['rules'][0], 'terms': ['pricing']}]
        self.assertEqual(WorkflowExperience.parse(value, self.p, self.s, corpus).value['version'], 2)

    def test_failed_tool_cannot_be_a_successful_step(self):
        self.trace['invocations'].insert(0, {'tool_name': 'remote_generate', 'outcome_status': 'failed',
            'result': {'_host_input_rejection': 1}})
        self.save(); corpus = self.corpus()
        self.assertEqual(corpus.rows[0]['steps'][0]['outcome'], 'input_invalid')
        value = self.value(); value['workflows'][0]['steps'][0]['tool'] = 'remote_generate'
        with self.assertRaisesRegex(ValueError, 'successful tool'):
            WorkflowExperience.parse(value, self.p, self.s, corpus)

    def test_unobserved_order_is_rejected(self):
        self.trace['invocations'].append({'tool_name': 'workspace_publish', 'outcome_status': 'success', 'result': {}})
        self.save(); corpus = self.corpus(); value = self.value()
        value['workflows'][0]['steps'].insert(0, {'tool': 'workspace_publish', 'action': 'Publish.', 'expect': 'Receipt.'})
        with self.assertRaisesRegex(ValueError, 'order'):
            WorkflowExperience.parse(value, self.p, self.s, corpus)

    def test_package_is_progressive_and_inventory_checked(self):
        corpus = self.corpus(); experience = WorkflowExperience.parse(self.value(), self.p, self.s, corpus)
        body = compile_package(self.p, self.s, experience, round_id='round-0', learner_run_ids=['learner'], trajectories=corpus)
        files = package_files(body); validate_inventory(files)
        self.assertNotIn(b'Compute the confirmed amount', files['SKILL.md'])
        self.assertIn(b'Compute the confirmed amount', files['references/experience-1.md'])
        self.assertEqual(json.loads(files['evolution/provenance.json'])['training_evidence_sha256'], corpus.identity)
        files['unrelated.py'] = b'code'
        with self.assertRaisesRegex(ValueError, 'unexpected'): validate_inventory(files)

    def test_candidate_limits_and_private_data(self):
        for mutation in ('private', 'too_large', 'wrong_group', 'unknown', 'empty_terms'):
            value = self.value(); item = value['workflows'][0]
            if mutation == 'private': item['steps'][0]['action'] = 'Use https://private.example'
            if mutation == 'too_large': item['steps'][0]['action'] = 'x' * 601
            if mutation == 'wrong_group': item['group'] = 'other'
            if mutation == 'unknown': item['sources'] = ['test_task']
            if mutation == 'empty_terms': item['terms'] = []
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                WorkflowExperience.parse(value, self.p, self.s, self.corpus())

    def test_evidence_symlinks_and_oversized_sequences_rejected(self):
        path = self.evidence / 'sample/session.json'; path.unlink(); path.symlink_to('/etc/passwd')
        with self.assertRaisesRegex(ValueError, 'symlink'): self.corpus()
        path.unlink(); self.trace['invocations'] *= 1000; self.save()
        with self.assertRaisesRegex(ValueError, 'window'): self.corpus()

    def test_no_evidence_or_unknown_method_is_refused(self):
        with self.assertRaises(ValueError): policy(self.s, method='arbitrary')
        with self.assertRaises(ValueError): policy(self.s, method='workflow', supervision='self')
        with self.assertRaises(ValueError): prompt(self.p, self.s)
        with self.assertRaises(ValueError): Controller(self.p, self.s, None, None, None, None, self.root / 'missing')

    def test_old_policy_identity_and_package_are_preserved(self):
        old = policy(self.s)
        self.assertNotIn('method', old.value)
        body = compile_package(old, self.s, candidate(self.s), round_id='round-0', learner_run_ids=['learner'])
        validate_inventory(package_files(body))
        self.assertEqual(len(body['files']), 3)
        self.assertEqual(set(skill_package()), {'version', 'scope', 'permissions', 'files'})

    def test_learner_cannot_execute_business_tools(self):
        outcome = {'state': 'succeeded', 'tokens': 25, 'text': json.dumps(self.value()), 'tool_names': ['sandbox_run']}
        with self.assertRaises(ValueError): extract(self.p, self.s, outcome, self.corpus())

    def test_candidate_correction_is_diagnosed_and_bounded(self):
        calls = []
        invalid = self.value(); invalid['workflows'][0]['terms'] = []
        def submit(suffix, question):
            calls.append((suffix, question))
            return {'state':'succeeded','tokens':25,'tool_names':[], 'run_ids':['attempt'+suffix],
                    'text':json.dumps(invalid if not suffix else self.value())}
        value, run_ids, errors = propose(self.p, self.s, submit, trajectories=self.corpus())
        self.assertEqual(len(calls), 2); self.assertEqual(len(run_ids), 2)
        self.assertIn('search terms', calls[1][1]); self.assertEqual(len(errors), 1)
        self.assertEqual(value.value, self.value())
        calls.clear()
        def always_invalid(suffix, question):
            calls.append(suffix)
            return {'state':'succeeded','tokens':25,'tool_names':[], 'run_ids':['attempt'+suffix], 'text':json.dumps(invalid)}
        with self.assertRaises(ValueError): propose(self.p, self.s, always_invalid, trajectories=self.corpus())
        self.assertEqual(len(calls), 2)

    def test_complex_workflow_keeps_twelve_steps_when_it_fits_context(self):
        self.trace['invocations'] *= 12; self.save()
        value = self.value()
        value['workflows'][0]['steps'] = [{'tool':'sandbox_run','action':'Compute the next value.',
                                         'expect':'A verified receipt.'} for _ in range(12)]
        experience = WorkflowExperience.parse(value, self.p, self.s, self.corpus())
        self.assertEqual(len(experience.value['workflows'][0]['steps']),12)
        limited = policy(self.s, method='workflow', max_rule_characters=300)
        with self.assertRaisesRegex(ValueError, 'context policy'):
            WorkflowExperience.parse(value, limited, self.s, self.corpus())

    def test_execution_failure_or_unknown_usage_never_triggers_candidate_repair(self):
        for state, tokens, tools in [('failed',25,[]),('succeeded',None,[]),('succeeded',25,['sandbox_run'])]:
            calls = []
            def submit(suffix, question):
                calls.append(suffix)
                return {'state':state,'tokens':tokens,'tool_names':tools,'text':'invalid','run_ids':['run']}
            with self.subTest(state=state,tokens=tokens,tools=tools), self.assertRaises(ValueError):
                propose(self.p,self.s,submit,trajectories=self.corpus())
            self.assertEqual(len(calls),1)

    def test_workflow_pipeline_requires_resource_receipts_and_resumes(self):
        managers = [Management('https://' + n + '.invalid') for n in ('base', 'canary', 'production')]
        base, canary, prod = managers
        bp, cp = Port(base, self.s), Port(canary, self.s)
        original = bp.execute
        def proposer(directory, prompts, fingerprint, attachments=()):
            value = original(directory, prompts, fingerprint, attachments)
            if prompts[0].startswith('Produce ONLY'):
                value['text'] = json.dumps(self.value()); write_private(Path(directory) / 'outcome.json', value)
            return value
        bp.execute = proposer
        controller = Controller(self.p, self.s, bp, cp, prod, canary, self.root / 'campaign', training_evidence=self.evidence)
        result = controller.run()
        self.assertEqual(result['phase'], 'validation_rejected')
        self.assertIn('matching_experience_resources_not_read', result['rounds'][0]['reason'])
        self.assertIsNone(prod.record)
        controller.run(); self.assertEqual(len(cp.calls), 3)

    def test_full_workflow_candidate_publication_and_inventory_reuse(self):
        from exoharness_evolution.publisher import Publisher
        base, canary, prod = [Management('https://' + n + '.invalid') for n in ('base', 'canary', 'production')]
        bp, cp = Port(base, self.s), Port(canary, self.s)
        original_base, original_canary = bp.execute, cp.execute
        def propose(directory, prompts, fingerprint, attachments=()):
            value = original_base(directory, prompts, fingerprint, attachments)
            if prompts[0].startswith('Produce ONLY'):
                value['text'] = json.dumps(self.value()); write_private(Path(directory) / 'outcome.json', value)
            return value
        def consume(directory, prompts, fingerprint, attachments=()):
            value = original_canary(directory, prompts, fingerprint, attachments)
            index = json.loads(package_files({'files': canary.files})['resource-index.json'])
            value['skill_resource_reads'] = [{'id': self.p.skill_id, 'revision': canary.record['revision'],
                'run_id': value['run_ids'][0], 'resources': [{'path': r['path'], 'sha256': r['sha256']} for r in index['resources']]}]
            write_private(Path(directory) / 'outcome.json', value)
            return value
        bp.execute, cp.execute = propose, consume
        p = policy(self.s, method='workflow', publish=True)
        c = Controller(p, self.s, bp, cp, prod, canary, self.root / 'campaign', training_evidence=self.evidence)
        self.assertEqual(c.run()['phase'], 'published')
        self.assertIn('resource-index.json', package_files(Publisher(prod, p).snapshot()))
        self.assertEqual(c.run()['phase'], 'published'); self.assertEqual(len(cp.calls), 6)
        self.trace['invocations'][0]['result']['changed'] = True; self.save()
        changed = Controller(p, self.s, bp, cp, prod, canary, self.root / 'campaign', training_evidence=self.evidence)
        with self.assertRaisesRegex(ValueError, 'changed'): changed.run()


if __name__ == '__main__':
    unittest.main()
