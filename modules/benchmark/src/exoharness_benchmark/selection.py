# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Local, report-bound delivery review; never modifies the source report."""
import copy
from .contracts import digest
from .metrics import retain


def review(report, evidence):
    if evidence.get('report_sha256') != digest(report):
        raise ValueError('Delivery review belongs to another report')
    reviewer = evidence.get('reviewer')
    if not isinstance(reviewer, str) or not reviewer.strip() or len(reviewer) > 120:
        raise ValueError('Delivery review requires an identified reviewer')
    decisions = evidence.get('decisions')
    if not isinstance(decisions, list):
        raise ValueError('Delivery review decisions must be a list')
    selected = {}
    known = {r['key'] for r in report['rows']}
    for item in decisions:
        key = item.get('key')
        if key not in known or key in selected or type(item.get('passed')) is not bool:
            raise ValueError('Invalid, duplicate or unknown delivery review')
        selected[key] = item['passed']
    result = copy.deepcopy(report)
    for row in result['rows']:
        if row['key'] in selected:
            row['review_pending'] = False
            # A review can reject a structurally valid artifact, but cannot turn
            # a failed execution or failed deterministic check into a delivery.
            row['delivered'] = row['delivered'] and selected[row['key']]
            row['delivery_review_passed'] = selected[row['key']]
    decision = retain(result['rows'], expected_tasks=result.get('expected_tasks', sorted({r['task_id'] for r in result['rows']})),
        repeats=result['repeats'], learner_tokens=result['learning_tokens'],
        expected_runtimes=result.get('runtime_fingerprints'))
    for reason in report['decision']['reasons']:
        if reason in ('runtime_drift', 'model_binding_drift', 'incomplete_or_unexpected_matrix'):
            decision['retain'] = False
            if reason not in decision['reasons']:
                decision['reasons'].append(reason)
    result['decision'] = decision
    result['delivery_review'] = {'source_report_sha256': digest(report),
        'evidence_sha256': digest(evidence), 'reviewer': reviewer,
        'reviewed_attempts': len(selected), 'production_imported': False}
    return result
