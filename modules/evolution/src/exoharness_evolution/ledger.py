# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Crash-safe ownership, spending reservations and publication evidence."""
import json
from pathlib import Path
from exoharness_benchmark.contracts import digest
from exoharness_benchmark.evidence import write_private


from exoharness_benchmark.evidence import exclusive


class Ledger:
    def __init__(self, root, identity):
        self.root, self.path = Path(root), Path(root) / 'evolution.json'
        if self.path.is_symlink():
            raise ValueError('Ledger must not be a symlink')
        self.state = json.loads(self.path.read_text()) if self.path.exists() else {
            'identity': identity, 'phase': 'learning', 'charges': {}, 'rounds': [],
            'tested_candidates': [], 'publications': [], 'monitor': {}}
        if self.state['identity'] != identity:
            raise ValueError('Policy, suite, baseline or management identity changed; create a new campaign')
        self.save()

    def save(self):
        write_private(self.path, self.state)

    @property
    def charged(self):
        return sum(self.state['charges'].values())

    def execute(self, key, port, prompts, fingerprint, *, ceiling, limit, attachments=()):
        if key not in self.state['charges']:
            reservation = ceiling * len(prompts)
            if self.charged + reservation > limit:
                raise RuntimeError('Evolution Token budget cannot admit another task')
            self.state['charges'][key] = reservation
            self.save()
        outcome = port.execute(self.root / key, prompts, fingerprint, attachments)
        if outcome.get('tokens') is not None:
            reservation = self.state['charges'][key]
            self.state['charges'][key] = outcome['tokens']; self.save()
            if outcome['tokens'] > reservation:
                raise RuntimeError('Observed Token reservation overrun; actual usage retained')
        if outcome.get('tokens') is None or outcome.get('runtime_drift'):
            raise RuntimeError('Unknown usage or runtime drift; retain reservation and stop')
        return outcome


def consume_holdout(root, suite, group, candidate_sha256):
    """A held-out input may gate one candidate only, including failed attempts."""
    path = Path(root) / 'holdouts.json'
    state = json.loads(path.read_text()) if path.exists() else {}
    keys = [digest({'turns': t['turns'], 'inputs': t.get('attachments', [])}) for t in suite.value['tasks'] if t['split'] == 'test' and t['group'] == group]
    if any(key in state and state[key] != candidate_sha256 for key in keys):
        raise ValueError('Held-out inputs already judged a different candidate; provide a fresh test partition')
    for key in keys:
        state[key] = candidate_sha256
    write_private(path, state)
