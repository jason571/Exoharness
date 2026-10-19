# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Bounded workflow candidates grounded in reviewed successful tool sequences."""
from dataclasses import dataclass
import json
import re
from exoharness_benchmark.contracts import Experience, digest, identifier


def safe_text(value, maximum):
    if (not isinstance(value, str) or not value.strip() or len(value) > maximum
            or any(ord(c) < 32 and c not in '\n\t' for c in value)
            or re.search(r'https?://|[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}|Bearer\s|(?:session-|run-|artifact-|vid_)\w{8,}|(?:api[_-]?key|password|access_token)\s*[:=]', value, re.I)):
        raise ValueError('Candidate requires bounded reusable text without private handles')
    return value


def search_terms(terms):
    if (not isinstance(terms, list) or not 1 <= len(terms) <= 8
            or not all(isinstance(t, str) and 2 <= len(t) <= 40 and t == t.strip().lower() for t in terms)
            or len(set(terms)) != len(terms)):
        raise ValueError('Experience needs bounded unique normalized search terms')
    for term in terms:
        safe_text(term, 40)


@dataclass(frozen=True)
class WorkflowExperience:
    canonical: str

    @classmethod
    def parse(cls, value, policy, suite, trajectories):
        if (not isinstance(value, dict) or set(value) != {'version', 'suite_sha256', 'rules', 'workflows'}
                or value['version'] != 2 or value['suite_sha256'] != suite.sha256):
            raise ValueError('Workflow candidate does not match the frozen contract')
        rules, workflows = value['rules'], value['workflows']
        if not isinstance(rules, list) or not isinstance(workflows, list) or not 1 <= len(rules) + len(workflows) <= 8 or len(workflows) > 4:
            raise ValueError('At most eight experiences and four workflows may be proposed')
        if rules:
            Experience.parse({'version': 1, 'suite_sha256': suite.sha256,
                              'rules': [{k: v for k, v in r.items() if k != 'terms'} for r in rules]}, suite)
        known = {row['train_id'] for row in trajectories.rows}
        successful = {row['train_id'] for row in trajectories.rows if row['verified_delivery']}
        identities = set()
        for rule in rules:
            if set(rule) != {'group', 'sources', 'when', 'action', 'unless', 'terms'} or rule['group'] != policy.value['group'] or not set(rule['sources']) <= known:
                raise ValueError('Lessons require matching reviewed training sources')
            search_terms(rule['terms'])
            for key in ('when', 'action', 'unless'):
                safe_text(rule[key], 600)
        for item in workflows:
            if not isinstance(item, dict) or set(item) != {'id', 'group', 'sources', 'when', 'unless', 'terms', 'steps'}:
                raise ValueError('Unknown workflow field')
            if not identifier(item['id']) or item['id'] in identities or item['group'] != policy.value['group']:
                raise ValueError('Invalid workflow identity or scope')
            identities.add(item['id'])
            sources = item['sources']
            if not isinstance(sources, list) or not sources or not all(isinstance(s, str) and s in successful for s in sources) or len(sources) != len(set(sources)):
                raise ValueError('Workflow sources must have verified successful deliveries')
            for key in ('when', 'unless'):
                safe_text(item[key], 300)
            terms = item['terms']
            search_terms(terms)
            steps = item['steps']
            if not isinstance(steps, list) or not steps:
                raise ValueError('Workflow requires observed steps')
            for step in steps:
                if not isinstance(step, dict) or set(step) != {'tool', 'action', 'expect'} or step['tool'] not in trajectories.tools(sources):
                    raise ValueError('A workflow step must reference an observed successful tool')
                safe_text(step['action'], 300); safe_text(step['expect'], 240)
            # An actual successful source must contain the proposed sequence in order.
            sequence = [step['tool'] for step in steps]
            def observed(row):
                it = iter(s['tool'] for s in row['steps'] if s['outcome'] == 'success')
                return all(any(tool == name for tool in it) for name in sequence)
            if not any(row['verified_delivery'] and row['train_id'] in sources and observed(row) for row in trajectories.rows):
                raise ValueError('Proposed tool order was not observed in a successful trajectory')
        text = json.dumps({'rules': rules, 'workflows': workflows}, ensure_ascii=False)
        if len(text) > policy.value['max_rule_characters']:
            raise ValueError('Workflow experience exceeds its context policy')
        return cls(json.dumps(value, sort_keys=True, ensure_ascii=False, allow_nan=False))

    @property
    def value(self):
        return json.loads(self.canonical)

    @property
    def sha256(self):
        return digest(self.value)
