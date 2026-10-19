# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Paired comparison of an intentional Harness implementation change."""
import json
from pathlib import Path

from .contracts import digest, integer
from .evidence import write_private
from .experiment import Experiment
from .metrics import retain
from .verifiers import verify


class RuntimeComparison:
    def __init__(self, suite, baseline, candidate, output, *, repeats=3,
                 max_tokens=2000000, run_reservation=300000, input_root=None):
        if not integer(repeats, 1) or repeats > 10 or not integer(max_tokens, run_reservation):
            raise ValueError('Invalid comparison repetitions or Token budget')
        if not integer(run_reservation, 1) or any(run_reservation < p.run_ceiling for p in (baseline, candidate)):
            raise ValueError('Comparison reservation does not cover the server ceilings')
        self.suite, self.ports, self.root = suite, {'base': baseline, 'candidate': candidate}, Path(output)
        self.repeats, self.limit, self.reservation = repeats, max_tokens, run_reservation
        self.inputs = Experiment(suite, baseline, output, repeats=repeats, max_tokens=max_tokens,
                                 run_reservation=run_reservation, input_root=input_root)

    def execute(self):
        self.root.mkdir(parents=True, mode=0o700, exist_ok=True)
        configurations = {mode: port.configuration_fingerprint() for mode, port in self.ports.items()}
        if len(set(configurations.values())) != 1:
            raise ValueError('Baseline and candidate must use identical model, assistant and capability configurations')
        fingerprints = {mode: port.fingerprint() for mode, port in self.ports.items()}
        identity = {'protocol': 'runtime-comparison-v1', 'suite_sha256': self.suite.sha256,
            'runtime_fingerprints': fingerprints, 'configuration_fingerprint': configurations['base'],
            'repeats': self.repeats, 'max_tokens': self.limit, 'run_reservation': self.reservation}
        path = self.root / 'comparison.json'
        state = json.loads(path.read_text()) if path.exists() else {'identity': identity, 'charges': {}, 'rows': []}
        if state['identity'] != identity:
            raise ValueError('Frozen comparison identity changed')
        write_private(path, state)
        tasks = [task for task in self.suite.value['tasks'] if task['split'] != 'train']
        for attempt in range(self.repeats):
            for task in tasks:
                modes = ('base', 'candidate') if attempt % 2 == 0 else ('candidate', 'base')
                for mode in modes:
                    key = f"{mode}-{task['id']}-{attempt}"
                    if any(row['key'] == key for row in state['rows']):
                        continue
                    port = self.ports[mode]
                    if port.fingerprint() != fingerprints[mode] or port.configuration_fingerprint() != configurations[mode]:
                        raise RuntimeError('Runtime/configuration drift before submission')
                    reservation = self.reservation * len(task['turns'])
                    if key not in state['charges']:
                        if sum(state['charges'].values()) + reservation > self.limit:
                            raise RuntimeError('Comparison Token admission budget exhausted; no new submission')
                        state['charges'][key] = reservation
                        write_private(path, state)
                    outcome = port.execute(self.root / key, list(task['turns']), fingerprints[mode], self.inputs.attachments(task))
                    if outcome.get('tokens') is not None:
                        state['charges'][key] = outcome['tokens']
                        write_private(path, state)
                        if outcome['tokens'] > reservation:
                            raise RuntimeError('Observed usage exceeded its reservation; actual cost retained, no further submission')
                    if outcome.get('runtime_drift') or outcome.get('tokens') is None:
                        raise RuntimeError('Uncertain usage or runtime drift; reservation retained, no further submission')
                    checked = verify(task, outcome)
                    row = {k: outcome[k] for k in ('tokens', 'run_ids', 'state', 'runtime_fingerprint', 'runtime_drift', 'model_bindings')}
                    row.update(key=key, task_id=task['id'], group=task['group'], split=task['split'], mode=mode,
                        attempt=attempt, **checked, review_pending=bool(task.get('review_required')),
                        first_turn_delivered=checked['delivered'] and len(task['turns']) == 1,
                        elapsed_ms=outcome.get('elapsed_ms'), model_duration_ms=outcome.get('model_duration_ms'),
                        model_requests=(outcome.get('usage') or {}).get('model_requests'))
                    state['rows'].append(row)
                    write_private(path, state)
                    print(json.dumps({k: row[k] for k in ('key', 'state', 'delivered', 'tokens', 'elapsed_ms')}), flush=True)
        decision = retain(state['rows'], expected_tasks=[t['id'] for t in tasks], repeats=self.repeats,
                          learner_tokens=0, expected_runtimes=fingerprints)
        if len({digest(row['model_bindings']) for row in state['rows']}) != 1:
            decision['retain'] = False
            decision['reasons'].append('model_binding_drift')
        report = {**identity, 'expected_tasks': [task['id'] for task in tasks],
            'rows': state['rows'], 'learning_tokens': 0,
            'total_charged_tokens': sum(state['charges'].values()), 'decision': decision,
            'production_configuration_modified': False,
            'measurement': 'Identical user inputs; all provider Tokens and failed attempts included; cache counted once'}
        write_private(self.root / 'report.json', report)
        return report
