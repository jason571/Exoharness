# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Explicit evolution scope and finite resource limits; never execution grants."""
from dataclasses import dataclass
import re
from exoharness_benchmark.contracts import digest, identifier, integer


@dataclass(frozen=True)
class Policy:
    canonical: str

    @classmethod
    def parse(cls, value):
        import json
        allowed = {'version', 'id', 'group', 'skill_name', 'description', 'scope',
                   'baseline_binary_sha256', 'suite_sha256', 'max_rounds', 'max_tokens',
                   'run_reservation', 'repeats', 'interval_seconds', 'min_savings',
                   'max_tokens_per_delivery', 'max_rule_characters', 'publish',
                   'monitor_user_ids', 'monitor_min_samples', 'monitor_failure_rate',
                   'monitor_max_tokens', 'auto_suspend', 'supervision', 'max_training_characters', 'method'}
        if not isinstance(value, dict) or set(value) - allowed or value.get('version') != 1:
            raise ValueError('Unknown policy field or version')
        for key in ('id', 'group', 'skill_name'):
            if not identifier(value.get(key)):
                raise ValueError('Policy identities must be bounded stable names')
        if not value['skill_name'].startswith('experience-'):
            raise ValueError('Evolution may manage only a dedicated experience-* Skill')
        description = value.get('description')
        if not isinstance(description, str) or not description.strip() or len(description) > 320 or any(c in description for c in '\r\n\0'):
            raise ValueError('A bounded single-line Skill description is required')
        scope = value.get('scope', {'kind': 'global'})
        if scope != {'kind': 'global'} and not (isinstance(scope, dict) and set(scope) == {'kind', 'user_id'}
                and scope['kind'] == 'user' and isinstance(scope['user_id'], str)
                and re.fullmatch(r'[A-Za-z0-9_-]{1,128}', scope['user_id'])):
            raise ValueError('Invalid publication scope')
        for key in ('baseline_binary_sha256', 'suite_sha256'):
            if not isinstance(value.get(key), str) or not re.fullmatch('[a-f0-9]{64}', value[key]):
                raise ValueError('Pin the actual production binary and complete suite')
        defaults = {'scope': scope, 'max_rounds': 3, 'max_tokens': 1000000,
                    'run_reservation': 300000, 'repeats': 3, 'interval_seconds': 3600,
                    'min_savings': .05, 'max_tokens_per_delivery': 100000,
                    'max_rule_characters': 2400, 'publish': False, 'monitor_user_ids': [],
                    'monitor_min_samples': 5, 'monitor_failure_rate': .2,
                    'monitor_max_tokens': 100000, 'auto_suspend': False, 'supervision': 'reflect', 'max_training_characters': 32000}
        value = {**defaults, **value}
        for key, low, high in [('max_rounds', 1, 20), ('max_tokens', 1, 100000000),
                               ('run_reservation', 1, 1000000), ('repeats', 3, 5),
                               ('interval_seconds', 60, 604800), ('max_tokens_per_delivery', 1, 1000000),
                               ('max_rule_characters', 200, 4800), ('monitor_min_samples', 2, 1000),
                               ('monitor_max_tokens', 1, 1000000), ('max_training_characters', 1000, 64000)]:
            if not integer(value[key], low) or value[key] > high:
                raise ValueError('Invalid bounded policy limit: ' + key)
        if value['max_tokens'] < value['run_reservation']:
            raise ValueError('Budget must admit at least one reserved Run')
        for key in ('min_savings', 'monitor_failure_rate'):
            if type(value[key]) not in (int, float) or not 0 <= value[key] <= 1:
                raise ValueError('Invalid policy fraction')
        if type(value['publish']) is not bool or type(value['auto_suspend']) is not bool:
            raise ValueError('Publication and suspension must be explicit booleans')
        owners = value['monitor_user_ids']
        if not isinstance(owners, list) or len(owners) > 100 or not all(isinstance(v, str) and v for v in owners) or len(set(owners)) != len(owners):
            raise ValueError('Monitoring requires a bounded explicit owner scope')
        if value['auto_suspend'] and not owners:
            raise ValueError('Automatic suspension requires scoped observation')
        if value['supervision'] not in ('self', 'fewshot', 'reflect'):
            raise ValueError('Unknown supervision mode')
        if value.get('method', 'rules') not in ('rules', 'workflow'):
            raise ValueError('Unknown evolution method')
        if value.get('method', 'rules') == 'workflow' and value['supervision'] == 'self':
            raise ValueError('Workflow induction requires reviewed supervision')
        return cls(json.dumps(value, sort_keys=True, ensure_ascii=False, allow_nan=False))

    @property
    def value(self):
        import json
        return json.loads(self.canonical)

    @property
    def sha256(self):
        return digest(self.value)

    @property
    def skill_id(self):
        scope = self.value['scope']
        return ('skill:global:_:' if scope['kind'] == 'global' else 'skill:user:' + scope['user_id'] + ':') + self.value['skill_name']
