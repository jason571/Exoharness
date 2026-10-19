# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Train -> immutable canary -> validation -> held-out test -> scoped publication."""
import json
from pathlib import Path
from exoharness_benchmark.contracts import digest
from exoharness_benchmark.evidence import write_private
from exoharness_benchmark.verifiers import verify
from . import evaluation, learning
from .ledger import Ledger, consume_holdout, exclusive
from .package import compile_package, package_digest, package_files, EXPERIENCE
from .publisher import Publisher


class Controller:
    def __init__(self, policy, suite, baseline, canary, production, canary_management, output, *, input_root=None, training_evidence=None):
        if suite.sha256 != policy.value['suite_sha256']:
            raise ValueError('Frozen suite does not match the evolution policy')
        for split in ('validation', 'test'):
            evaluation.phase_suite(suite, policy.value['group'], split)
        self.policy, self.suite, self.baseline, self.canary = policy, suite, baseline, canary
        self.production = Publisher(production, policy)
        self.staging = Publisher(canary_management, policy)
        self.root, self.input_root = Path(output), input_root
        from .trajectories import Trajectories
        scope = policy.value['scope']
        self.trajectories = Trajectories(training_evidence, suite, policy.value['group'],
                                         scope.get('user_id') if scope['kind'] == 'user' else None)
        if policy.value.get('method', 'rules') == 'workflow' and not self.trajectories.rows:
            raise ValueError('Workflow induction requires reviewed training evidence')

    def _open(self):
        configuration = evaluation.check_environment(self.policy, self.baseline, self.canary, self.production.client)
        identity = {'policy_sha256': self.policy.sha256, 'suite_sha256': self.suite.sha256,
            'baseline_binary_sha256': self.policy.value['baseline_binary_sha256'],
            'production_origin': self.production.client.origin, 'baseline_origin': self.baseline.portal,
            'canary_origin': self.canary.portal, 'configuration_sha256': configuration}
        if self.policy.value.get('method', 'rules') == 'workflow':
            from .skill import identity as skill_identity
            identity.update(training_evidence_sha256=self.trajectories.identity, learning_skill_sha256=skill_identity())
        return Ledger(self.root, identity)

    def _evaluate(self, ledger, round_, split, learning_tokens):
        key = round_['id'] + '-' + split
        limits = ledger.state.setdefault('phase_limits', {})
        if key not in limits:
            limits[key] = self.policy.value['max_tokens'] - ledger.charged
            if limits[key] < self.policy.value['run_reservation']:
                raise RuntimeError('Evolution Token budget cannot admit evaluation')
            ledger.state['charges'][key] = limits[key]; ledger.save()
        output = self.root / key
        files = package_files(json.loads((self.root / round_['id'] / 'package.json').read_text()))
        index = json.loads(files['resource-index.json']) if 'resource-index.json' in files else None
        try:
            return evaluation.compare(self.policy, self.suite, split, self.baseline, self.canary, output,
                learning_tokens=learning_tokens, max_tokens=limits[key], input_root=self.input_root,
                skill_revision=round_['canary_revision'], resource_index=index)
        finally:
            # Includes failed requests and unresolved per-Run reservations on restart.
            evidence = output / 'comparison.json'
            if evidence.exists():
                ledger.state['charges'][key] = sum(json.loads(evidence.read_text())['charges'].values())
                ledger.save()

    def run(self):
        with exclusive(self.root):
            ledger = self._open()
            if ledger.state['phase'] == 'validated' and self.policy.value['publish']:
                return self._publish(ledger)
            if ledger.state['phase'] in ('published', 'validated', 'validation_rejected', 'test_rejected', 'unchanged', 'exhausted'):
                return ledger.state
            if 'initial_revision' not in ledger.state:
                current = self.production.existing()
                baseline_record = next((r for r in self.baseline.configuration()['capabilities'] if r['id'] == self.policy.skill_id), None)
                if ((baseline_record or {}).get('source_digest'), (baseline_record or {}).get('enabled')) != ((current or {}).get('source_digest'), (current or {}).get('enabled')):
                    raise ValueError('Baseline learned Skill is not the actual production incumbent')
                ledger.state['initial_revision'] = current['revision'] if current else None
                body = self.production.snapshot() if current else None
                if body:
                    write_private(self.root / 'incumbent-package.json', body)
                    ledger.state['incumbent_digest'] = package_digest(body)
                ledger.save()
            # A source Skill may not be replaced by unrelated administrator work.
            current = self.production.existing()
            if (current['revision'] if current else None) != ledger.state['initial_revision']:
                raise ValueError('Production experience changed during the campaign')
            start = len(ledger.state['rounds']) - (1 if ledger.state['rounds'] and ledger.state['rounds'][-1]['phase'] != 'rejected' else 0)
            for number in range(max(0, start), self.policy.value['max_rounds']):
                if number == len(ledger.state['rounds']):
                    ledger.state['rounds'].append({'id': 'round-' + str(number), 'phase': 'learning'})
                    ledger.save()
                round_ = ledger.state['rounds'][number]; rid = round_['id']
                body_path = self.root / rid / 'package.json'
                if not body_path.exists():
                    incumbent = None; feedback = []
                    if number:
                        previous = self.root / ledger.state['rounds'][number - 1]['id']
                        if (previous / 'experience.json').exists():
                            incumbent = json.loads((previous / 'experience.json').read_text())
                        if (previous / 'train-feedback.json').exists():
                            feedback = json.loads((previous / 'train-feedback.json').read_text())
                    elif (self.root / 'incumbent-package.json').exists():
                        incumbent = json.loads(package_files(json.loads((self.root / 'incumbent-package.json').read_text()))[EXPERIENCE])
                    def submit(suffix, question):
                        return ledger.execute(rid + '/learner' + suffix, self.baseline, [question], self.baseline.fingerprint(),
                            ceiling=self.policy.value['run_reservation'], limit=self.policy.value['max_tokens'])
                    candidate, learner_runs, corrections = learning.propose(self.policy, self.suite, submit,
                        incumbent, feedback, self.trajectories)
                    write_private(self.root / rid / 'candidate-validation.json', {'corrections': corrections, 'run_ids': learner_runs})
                    write_private(self.root / rid / 'experience.json', candidate.value)
                    if candidate.sha256 in ledger.state['tested_candidates']:
                        round_.update(phase='rejected', reason='unchanged_candidate')
                        ledger.state['phase'] = 'unchanged'; ledger.save(); return ledger.state
                    ledger.state['tested_candidates'].append(candidate.sha256)
                    body = compile_package(self.policy, self.suite, candidate, round_id=rid,
                                           learner_run_ids=learner_runs, trajectories=self.trajectories)
                    write_private(body_path, body)
                    round_['candidate_sha256'] = candidate.sha256
                    record = self.staging.existing()
                    round_['canary_expected_revision'] = record['revision'] if record else None
                    ledger.save()
                body = json.loads(body_path.read_text())
                if 'canary_expected_revision' not in round_:
                    # Package write may have committed immediately before a
                    # crash. Publication starts only after this intent is saved.
                    candidate = json.loads((self.root / rid / 'experience.json').read_text())
                    round_['candidate_sha256'] = digest(candidate)
                    record = self.staging.existing()
                    round_['canary_expected_revision'] = record['revision'] if record else None
                    ledger.save()
                receipt = self.staging.publish(body, self.root / rid / 'canary-publication', expected_revision=round_['canary_expected_revision'])
                round_['canary_revision'] = receipt['revision']; ledger.save()
                # Candidate selection sees only train feedback. Validation and test answers never return to the learner.
                if self.policy.value['supervision'] == 'reflect':
                    feedback = self._train_feedback(ledger, rid)
                    if not all(item['passed'] for item in feedback):
                        round_.update(phase='rejected', reason='train_feedback'); ledger.save(); continue
                learning_tokens = sum(v for k, v in ledger.state['charges'].items() if '/learner' in k or '/train-' in k)
                report = self._evaluate(ledger, round_, 'validation', learning_tokens)
                if 'delivery_review_pending' in report['decision']['reasons']:
                    round_['phase'] = 'awaiting_validation_review'; ledger.save(); return ledger.state
                if not report['decision']['retain']:
                    round_.update(phase='rejected', reason=report['decision']['reasons'])
                    ledger.state['phase'] = 'validation_rejected'; ledger.save(); return ledger.state
                # Shared across sibling campaigns, not reset when one campaign ends.
                with exclusive(self.root.parent / '.evolution-holdouts'):
                    consume_holdout(self.root.parent / '.evolution-holdouts', self.suite, self.policy.value['group'], round_['candidate_sha256'])
                # Cost of candidate search belongs to the final decision, not just its last learner.
                search_cost = learning_tokens + sum(r['tokens'] for r in report['rows'] if r['mode'] == 'candidate')
                final = self._evaluate(ledger, round_, 'test', search_cost)
                if 'delivery_review_pending' in final['decision']['reasons']:
                    round_['phase'] = 'awaiting_test_review'; ledger.save(); return ledger.state
                if not final['decision']['retain']:
                    round_.update(phase='test_rejected', reason=final['decision']['reasons'])
                    ledger.state['phase'] = 'test_rejected'; ledger.save(); return ledger.state
                round_['phase'] = 'validated'; ledger.state['phase'] = 'validated'
                write_private(self.root / 'release.json', {'policy_sha256': self.policy.sha256,
                    'candidate_sha256': round_['candidate_sha256'], 'package_sha256': package_digest(body),
                    'validation_report_sha256': digest(report), 'test_report_sha256': digest(final),
                    'expected_revision': ledger.state['initial_revision'], 'round_id': rid})
                ledger.save()
                if self.policy.value['publish']:
                    return self._publish(ledger)
                return ledger.state
            ledger.state['phase'] = 'exhausted'; ledger.save(); return ledger.state

    def _train_feedback(self, ledger, rid):
        feedback = []
        from exoharness_benchmark.experiment import Experiment
        inputs = Experiment(self.suite, self.canary, self.root, input_root=self.input_root)
        for task in self.suite.value['tasks']:
            if task['split'] != 'train' or task['group'] != self.policy.value['group']:
                continue
            outcome = ledger.execute(rid + '/train-' + task['id'], self.canary, task['turns'], self.canary.fingerprint(),
                ceiling=self.policy.value['run_reservation'], limit=self.policy.value['max_tokens'], attachments=inputs.attachments(task))
            result = verify(task, outcome)
            feedback.append({'train_id': task['id'], 'passed': result['delivered'],
                             'checks': result['checks'], 'answer': outcome.get('text', '')[:2000]})
        write_private(self.root / rid / 'train-feedback.json', feedback)
        return feedback

    def publish(self):
        with exclusive(self.root):
            return self._publish(self._open())

    def monitor(self):
        from .monitor import observe
        with exclusive(self.root):
            ledger = Ledger(self.root, json.loads((self.root / 'evolution.json').read_text())['identity'])
            if (ledger.state['identity']['policy_sha256'] != self.policy.sha256
                    or ledger.state['identity']['production_origin'] != self.production.client.origin):
                raise ValueError('Monitoring policy does not match this campaign')
            self.production.client.runtime()
            return observe(self.production, self.policy, ledger)

    def _publish(self, ledger):
        if ledger.state['phase'] == 'published':
            return ledger.state
        if ledger.state['phase'] != 'validated':
            raise ValueError('Only a fully validated candidate can be published')
        release = json.loads((self.root / 'release.json').read_text())
        body = json.loads((self.root / release['round_id'] / 'package.json').read_text())
        for split in ('validation', 'test'):
            report = json.loads((self.root / (release['round_id'] + '-' + split) / 'evolution-report.json').read_text())
            if digest(report) != release[split + '_report_sha256'] or not report['decision']['retain']:
                raise ValueError('Release evidence was changed or rejected')
        if release['policy_sha256'] != self.policy.sha256 or package_digest(body) != release['package_sha256']:
            raise ValueError('Release package or policy was changed')
        if self.production.client.runtime() != self.policy.value['baseline_binary_sha256']:
            raise ValueError('Production executable changed before publication')
        receipt = self.production.publish(body, self.root / 'production-publication', expected_revision=release['expected_revision'])
        ledger.state['publications'].append({**receipt, 'candidate_sha256': release['candidate_sha256']})
        ledger.state['phase'] = 'published'; ledger.save()
        return ledger.state
