# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Compare actual persisted Skills; only that dedicated package may differ."""
import copy
import json
from pathlib import Path
from exoharness_benchmark.contracts import digest, Suite
from exoharness_benchmark.comparison import RuntimeComparison
from exoharness_benchmark.evidence import write_private
from exoharness_benchmark.metrics import retain
from exoharness_benchmark.selection import review


def stable_configuration(port, skill_id):
    value = copy.deepcopy(port.configuration())
    # Portal profiles intentionally omit private access policy. Freeze the full
    # management profile as well, so an identity projection is never a grant.
    if hasattr(port, 'auth'):
        import urllib.parse
        value['profile'] = port.api('/v1/agent-profiles/' + urllib.parse.quote(port.auth['assistant_id'], safe=''), admin=True)
    return normalize_configuration(value, skill_id)


def normalize_configuration(value, skill_id):
    value = copy.deepcopy(value)
    for key in ('capabilities', 'capability_evidence'):
        value[key] = [r for r in value[key] if r['id'] != skill_id]
    return value


class ScopedPort:
    def __init__(self, port, skill_id):
        self.port, self.skill_id, self.run_ceiling = port, skill_id, port.run_ceiling

    def fingerprint(self):
        return self.port.fingerprint()

    def configuration_fingerprint(self):
        return digest(stable_configuration(self.port, self.skill_id))

    def execute(self, *args):
        return self.port.execute(*args)


def check_environment(policy, baseline, candidate, production):
    scope = policy.value['scope']
    if scope['kind'] == 'user' and any(getattr(p, 'user_id', None) != scope['user_id'] for p in (baseline, candidate)):
        raise ValueError('User-scoped evaluation must use the same owned principal identity')
    expected = policy.value['baseline_binary_sha256']
    if production.runtime() != expected or any(p.auth['runtime_version'] != expected for p in (baseline, candidate)):
        raise ValueError('Baseline/canary must run the pinned actual production executable')
    endpoints = [baseline.portal, candidate.portal, production.origin]
    if len(set(endpoints)) != 3 or baseline.management == candidate.management:
        raise ValueError('Evaluation must use distinct isolated baseline, canary and publication services')
    a, b = stable_configuration(baseline, policy.skill_id), stable_configuration(candidate, policy.skill_id)
    actual = normalize_configuration(production.configuration(baseline.auth['assistant_id']), policy.skill_id)
    if digest(a) != digest(b) or digest(a) != digest(actual):
        raise ValueError('Model, assistant, grants or unrelated capability differs')
    profile = b['profile']
    access = profile.get('access', {}).get('skills', {})
    if access.get('mode') != 'all' and policy.skill_id not in access.get('ids', []):
        raise ValueError('The canary assistant is not authorized for the experience Skill')
    # Trigger executable/account validation before spending learning Tokens.
    baseline.fingerprint(); candidate.fingerprint()
    return digest(actual)


def phase_suite(suite, group, split):
    tasks = [copy.deepcopy(t) for t in suite.value['tasks'] if t['group'] == group and t['split'] in ('train', split)]
    if not any(t['split'] == split for t in tasks):
        raise ValueError('Evolution requires disjoint validation and held-out test tasks')
    for t in tasks:
        if t['split'] == split:
            t['split'] = 'test'
    return Suite.parse({'version': 1, 'id': suite.value['id'][:55] + '_' + split,
                        'tasks': tasks, **({'rules': suite.value['rules']} if 'rules' in suite.value else {})})


def compare(policy, suite, split, baseline, candidate, output, *, learning_tokens, max_tokens, input_root, skill_revision, resource_index=None):
    current = phase_suite(suite, policy.value['group'], split)
    engine = RuntimeComparison(current, ScopedPort(baseline, policy.skill_id), ScopedPort(candidate, policy.skill_id),
        output, repeats=policy.value['repeats'], max_tokens=max_tokens,
        run_reservation=policy.value['run_reservation'], input_root=input_root)
    original = engine.execute()
    report = copy.deepcopy(original)
    report['learning_tokens'] = learning_tokens
    report['decision'] = retain(report['rows'], expected_tasks=report['expected_tasks'], repeats=report['repeats'],
        learner_tokens=learning_tokens, min_savings=policy.value['min_savings'],
        expected_runtimes=report['runtime_fingerprints'])
    # A cost reduction never compensates for a failed candidate task.
    if not all(r['delivered'] for r in report['rows'] if r['mode'] == 'candidate'):
        report['decision']['reasons'].append('candidate_delivery_failed')
    candidate_mean = report['decision']['candidate_tokens_per_delivery_including_learning']
    if candidate_mean is None or candidate_mean > policy.value['max_tokens_per_delivery']:
        report['decision']['reasons'].append('delivery_token_ceiling')
    if any(r.get('tokens') is not None and r['tokens'] > policy.value['max_tokens_per_delivery']
           for r in report['rows'] if r['mode'] == 'candidate'):
        report['decision']['reasons'].append('attempt_token_ceiling')
    if any(reason in original['decision']['reasons'] for reason in ('model_binding_drift', 'runtime_drift')):
        report['decision']['reasons'].append('model_or_runtime_drift')
    # Inspect persisted proof of reuse, rather than inferring it from an outcome.
    reused = []
    for row in report['rows']:
        if row['mode'] != 'candidate':
            continue
        outcome = json.loads((Path(output) / row['key'] / 'outcome.json').read_text())
        reused.append(any(s['id'] == policy.skill_id and s.get('revision') == skill_revision for s in outcome.get('skill_reads', [])))
    if not reused or not all(reused):
        report['decision']['reasons'].append('experience_not_read_in_every_candidate_attempt')
    if resource_index is not None:
        expected = {(r['path'], r['sha256']) for r in resource_index['resources']}
        for row in report['rows']:
            if row['mode'] != 'candidate':
                continue
            outcome = json.loads((Path(output) / row['key'] / 'outcome.json').read_text())
            receipts = [r for r in outcome.get('skill_resource_reads', [])
                        if r['id'] == policy.skill_id and r['revision'] == skill_revision]
            selected = [(r.get('path'), r.get('sha256')) for receipt in receipts for r in receipt['resources']]
            if not selected or not all(item in expected for item in selected):
                report['decision']['reasons'].append('matching_experience_resources_not_read')
    report.update(original_suite_sha256=suite.sha256, evaluation_split=split,
                  policy_sha256=policy.sha256, actual_persisted_skill=True, skill_revision=skill_revision)
    custom = set(report['decision']['reasons']) - {'delivery_review_pending', 'no_measured_gain_including_learning'}
    evidence = Path(output) / 'delivery-review.json'
    if evidence.exists():
        report = review(report, json.loads(evidence.read_text()))
        report['decision'] = retain(report['rows'], expected_tasks=report['expected_tasks'], repeats=report['repeats'],
            learner_tokens=learning_tokens, min_savings=policy.value['min_savings'],
            expected_runtimes=report['runtime_fingerprints'])
        report['decision']['reasons'].extend(custom)
        # Review may reject a structurally valid artifact; recompute the hard
        # all-delivered and amortized-cost gates on the reviewed rows.
        if not all(r['delivered'] for r in report['rows'] if r['mode'] == 'candidate'):
            report['decision']['reasons'].append('candidate_delivery_failed')
        amount = report['decision']['candidate_tokens_per_delivery_including_learning']
        if amount is None or amount > policy.value['max_tokens_per_delivery']:
            report['decision']['reasons'].append('delivery_token_ceiling')
    report['decision']['reasons'] = sorted(set(report['decision']['reasons']))
    report['decision']['retain'] = not report['decision']['reasons']
    write_private(Path(output) / 'evolution-report.json', report)
    return report
