# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Repeated end-to-end regression acceptance; no learning or hidden retries."""
import json
from pathlib import Path
from .contracts import integer
from .evidence import write_private
from .experiment import Experiment
from .metrics import aggregate
from .verifiers import verify


class Acceptance:
    def __init__(self, suite, port, output, *, repeats=2, max_tokens=4000000,
                 run_reservation=300000, input_root=None):
        if not integer(repeats, 2) or repeats > 3:
            raise ValueError('Acceptance requires two or three independent repetitions')
        self.inputs = Experiment(suite, port, output, repeats=repeats, max_tokens=max_tokens,
                                 run_reservation=run_reservation, input_root=input_root)
        self.suite, self.port, self.root = suite, port, Path(output)
        self.repeats, self.limit, self.reservation = repeats, max_tokens, run_reservation

    def execute(self):
        self.root.mkdir(mode=0o700, parents=True, exist_ok=True)
        fingerprint = self.port.fingerprint()
        identity = dict(protocol='repeated-acceptance-v1', suite_sha256=self.suite.sha256,
                        runtime_fingerprint=fingerprint, repeats=self.repeats,
                        max_tokens=self.limit, run_reservation=self.reservation)
        path = self.root / 'campaign.json'
        state = json.loads(path.read_text()) if path.exists() else dict(identity=identity, charges={}, rows=[])
        if state['identity'] != identity:
            raise ValueError('Acceptance inputs, runtime or budget changed; retain the old campaign')
        write_private(path, state)
        tasks = [t for t in self.suite.value['tasks'] if t['split'] != 'train']
        for attempt in range(self.repeats):
            for task in tasks:
                key = f"accept-{task['id']}-{attempt}"
                if any(r['key'] == key for r in state['rows']):
                    continue
                if any(r['task_id']==task['id'] and not r['delivered'] for r in state['rows']):
                    # Diagnose a known failure before spending another model or
                    # generation request on the same unchanged scenario.
                    continue
                if self.port.fingerprint() != fingerprint:
                    raise RuntimeError('Runtime drift before submission')
                reservation = self.reservation * len(task['turns'])
                if key not in state['charges']:
                    if sum(state['charges'].values()) + reservation > self.limit:
                        raise RuntimeError('Acceptance Token admission budget exhausted')
                    state['charges'][key] = reservation
                    write_private(path, state)
                outcome = self.port.execute(self.root / key, task['turns'], fingerprint, self.inputs.attachments(task))
                if outcome.get('tokens') is not None:
                    state['charges'][key] = outcome['tokens']
                    write_private(path, state)
                    if outcome['tokens'] > reservation:
                        raise RuntimeError('Observed reservation overrun; actual cost retained')
                if outcome.get('runtime_drift') or outcome.get('tokens') is None:
                    raise RuntimeError('Runtime drift or unknown usage; reservation retained, no new submission')
                row = {k:outcome[k] for k in ('tokens','run_ids','state','runtime_fingerprint','model_bindings')}
                row.update(key=key,task_id=task['id'],group=task['group'],attempt=attempt,
                           **verify(task,outcome),review_pending=bool(task.get('review_required')),
                           elapsed_ms=outcome.get('elapsed_ms'),model_requests=(outcome.get('usage') or {}).get('model_requests'))
                row['first_turn_delivered'] = row['delivered'] and len(task['turns']) == 1
                state['rows'].append(row)
                write_private(path,state)
                print(json.dumps({k:row[k] for k in ('key','state','delivered','tokens','elapsed_ms')}),flush=True)
        report={**identity,'expected_tasks':[t['id'] for t in tasks],'rows':state['rows'],
                'total_charged_tokens':sum(state['charges'].values()),'metrics':aggregate(state['rows']),
                'expected_attempts':len(tasks)*self.repeats,
                'all_structural_checks_passed':len(state['rows'])==len(tasks)*self.repeats and all(r['delivered'] for r in state['rows']),
                'review_pending':any(r['review_pending'] for r in state['rows']),
                'production_configuration_modified':False}
        write_private(self.root/'report.json',report)
        return report
