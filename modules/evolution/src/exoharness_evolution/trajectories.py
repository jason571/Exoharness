# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Reviewed, train-only projections of real adapter evidence; no raw payload reuse."""
import json
import re
from pathlib import Path
from exoharness_benchmark.contracts import digest, integer
from exoharness_benchmark.metrics import usage
from exoharness_benchmark.verifiers import verify


def _read(root, name, maximum=8 * 1024 * 1024):
    if not isinstance(name, str) or not re.fullmatch(r'[A-Za-z0-9_./-]{1,240}', name):
        raise ValueError('Invalid evidence relative path')
    relative = Path(name)
    if relative.is_absolute() or any(p in ('.', '..') for p in relative.parts):
        raise ValueError('Evidence must stay inside its private directory')
    current = root
    for part in relative.parts:
        current = current / part
        if current.is_symlink():
            raise ValueError('Evidence symlinks are not permitted')
    if not current.is_file() or current.stat().st_size > maximum or current.stat().st_mode & 0o077:
        raise ValueError('Evidence requires bounded private regular files')
    return json.loads(current.read_text())


def _failure(invocation):
    result = invocation.get('result') or {}
    if result.get('_host_authorization_denied'):
        return 'capability_denied'
    if result.get('_host_input_rejection'):
        return 'input_invalid'
    if invocation.get('outcome_status') in ('unknown', 'submitted', 'running', None):
        return 'unknown_submission'
    return 'execution_failed'


class Trajectories:
    def __init__(self, directory, suite, group, owner=None):
        self.rows = []
        self.identity = None
        if directory is None:
            return
        root = Path(directory)
        if root.is_symlink() or not root.is_dir() or root.stat().st_mode & 0o077:
            raise ValueError('Training evidence directory must be private')
        manifest = _read(root, 'manifest.json', 64 * 1024)
        if set(manifest) != {'version', 'suite_sha256', 'samples'} or manifest['version'] != 1 or manifest['suite_sha256'] != suite.sha256:
            raise ValueError('Training evidence does not match the frozen suite')
        samples = manifest['samples']
        if not isinstance(samples, list) or not 1 <= len(samples) <= 16:
            raise ValueError('Training evidence requires 1..16 reviewed samples')
        tasks = {t['id']: t for t in suite.value['tasks'] if t['split'] == 'train' and t['group'] == group}
        seen = set()
        for sample in samples:
            if not isinstance(sample, dict) or set(sample) != {'task_id', 'directory', 'outcome_sha256', 'session_sha256', 'trace_sha256', 'owner_user_id', 'reviewer', 'delivery_passed'}:
                raise ValueError('Invalid reviewed sample')
            task = tasks.get(sample['task_id'])
            if task is None:
                raise ValueError('Only matching training tasks may supply trajectories')
            if (not isinstance(sample['reviewer'], str) or not sample['reviewer'].strip()
                    or len(sample['reviewer']) > 120 or type(sample['delivery_passed']) is not bool
                    or not isinstance(sample['owner_user_id'], str) or not sample['owner_user_id']):
                raise ValueError('A sample needs an identified owner and delivery reviewer')
            if owner is not None and sample['owner_user_id'] != owner:
                raise ValueError('User-scoped learning cannot consume another owner\'s trajectories')
            directory = sample['directory']
            outcome = _read(root, directory + '/outcome.json')
            execution = _read(root, directory + '/execution.json', 64 * 1024)
            session = _read(root, directory + '/session.json', 64 * 1024)
            if (digest(session) != sample['session_sha256'] or session.get('id') != execution.get('session_id')
                    or session.get('owner_user_id') != sample['owner_user_id']):
                raise ValueError('Training session ownership or reviewed identity mismatch')
            if digest(outcome) != sample['outcome_sha256']:
                raise ValueError('Reviewed outcome changed')
            turns, runs = execution.get('turns', []), outcome.get('run_ids', [])
            hashes = sample['trace_sha256']
            if (not isinstance(hashes, list) or len(hashes) != len(task['turns'])
                    or len(turns) != len(hashes) or len(runs) != len(hashes)):
                raise ValueError('Evidence must cover every original training turn')
            calls, steps, states = [], [], []
            for index, (prompt, turn, run_id, expected) in enumerate(zip(task['turns'], turns, runs, hashes)):
                if run_id in seen or turn.get('run_id') != run_id or turn.get('prompt_sha256') != digest(prompt):
                    raise ValueError('Duplicated Run or mismatched training input')
                seen.add(run_id)
                trace = _read(root, directory + f'/trace-{index}.json')
                run = trace.get('run', {})
                if (digest(trace) != expected or run.get('id') != run_id or trace.get('prompt') != prompt
                        or run.get('session_id') != session['id']
                        or run.get('status') not in ('succeeded', 'failed', 'recoverable')):
                    raise ValueError('Trace identity, ownership, review or terminal state mismatch')
                states.append(run['status'])
                calls.extend(trace.get('model_calls', []))
                for invocation in trace.get('invocations', []):
                    name = invocation.get('tool_name')
                    if not isinstance(name, str) or not re.fullmatch(r'[A-Za-z0-9_]{1,80}', name):
                        raise ValueError('Invalid recorded tool name')
                    result = invocation.get('result') or {}
                    success = (invocation.get('outcome_status') == 'success'
                               and not result.get('isError') and not result.get('is_error')
                               and not result.get('_host_input_rejection') and not result.get('_host_authorization_denied'))
                    # Payloads, source, text, paths and handles never enter learning context.
                    steps.append({'tool': name, 'outcome': 'success' if success else _failure(invocation)})
            if len(json.dumps(steps, ensure_ascii=False)) > 16000:
                raise ValueError('Trajectory exceeds the bounded induction window')
            measured = usage(calls)
            if (measured['unknown_model_calls'] or not calls or not integer(outcome.get('tokens'))
                    or outcome['tokens'] != measured['total_tokens'] or outcome.get('runtime_drift')
                    or outcome.get('turn_states') != states or outcome.get('state') != states[-1]):
                raise ValueError('Evidence needs known usage and consistent runtime outcomes')
            delivered = verify(task, outcome)['delivered'] and all(s == 'succeeded' for s in states)
            if sample['delivery_passed'] and not delivered:
                raise ValueError('Review cannot turn a failed execution into a successful workflow')
            verified = delivered and sample['delivery_passed']
            self.rows.append({'train_id': task['id'], 'verified_delivery': verified,
                'state': states[-1], 'tokens': outcome['tokens'], 'steps': steps,
                'failure_classes': sorted({s['outcome'] for s in steps if s['outcome'] != 'success'}
                                          | ({'delivery_invalid'} if not verified else set())),
                'evidence_sha256': digest({'outcome': sample['outcome_sha256'], 'traces': hashes})})
        self.identity = digest({'manifest': manifest, 'projection': self.rows})

    def tools(self, sources):
        return {s['tool'] for row in self.rows if row['train_id'] in sources and row['verified_delivery']
                for s in row['steps'] if s['outcome'] == 'success'}
