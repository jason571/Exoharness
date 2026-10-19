# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Persistent finite-budget queue; changed or failed jobs are never blindly replayed."""
import json
import time
from pathlib import Path
from exoharness_benchmark.contracts import digest, identifier
from exoharness_benchmark.evidence import write_private
from .ledger import exclusive


def tick(queue_root, state_root, limit, factory):
    queue_root, state_root = Path(queue_root), Path(state_root)
    if queue_root.is_symlink() or queue_root.stat().st_mode & 0o077:
        raise ValueError('Queue must be a private directory')
    with exclusive(state_root):
        path = state_root / 'queue.json'
        state = json.loads(path.read_text()) if path.exists() else {'limit': limit, 'jobs': {}}
        if state['limit'] != limit:
            raise ValueError('Queue Token ceiling is immutable')
        write_private(path, state)
        paths = sorted(queue_root.glob('*.json'))
        if len(paths) > 100:
            raise ValueError('Queue is bounded to 100 campaigns; archive completed job files')
        for job_file in paths:
            if job_file.is_symlink() or job_file.stat().st_mode & 0o077:
                raise ValueError('Queue entries must be private regular files')
            job = json.loads(job_file.read_text())
            required = {'version', 'id', 'policy', 'suite', 'baseline_auth', 'canary_auth', 'publication_auth', 'input_root'}
            if not required <= set(job) or set(job) - required - {'training_evidence'} or job['version'] != 1 or not identifier(job['id']):
                raise ValueError('Invalid evolution job')
            key = job['id']; saved = state['jobs'].get(key)
            identity = digest(job)
            if saved and saved['job_sha256'] != identity:
                raise ValueError('Queued job changed; use a new campaign identity')
            controller = factory(job, state_root / key)
            if not saved:
                reserve = controller.policy.value['max_tokens']
                if sum(v['charged_tokens'] for v in state['jobs'].values()) + reserve > limit:
                    continue
                saved = {'job_sha256': identity, 'charged_tokens': reserve, 'phase': 'admitted'}
                state['jobs'][key] = saved; write_private(path, state)
            if saved['phase'] == 'blocked':
                continue
            try:
                if saved['phase'] == 'published':
                    # Ongoing observation does not depend on keeping obsolete
                    # evaluation instances running or freezing unrelated tools.
                    now = time.time()
                    if now - saved.get('last_monitor_at', 0) >= controller.policy.value['interval_seconds']:
                        saved['monitor'] = controller.monitor()
                        saved['last_monitor_at'] = now
                    write_private(path, state)
                    continue
                result = controller.run()
                ledger = json.loads((controller.root / 'evolution.json').read_text())
                saved['charged_tokens'] = (sum(ledger['charges'].values()) if result['phase'] in
                    ('published', 'validated', 'validation_rejected', 'test_rejected', 'unchanged', 'exhausted')
                    else controller.policy.value['max_tokens'])
                saved['phase'] = result['phase']
                if result['phase'] == 'published':
                    saved['monitor'] = controller.monitor()
                    saved['last_monitor_at'] = time.time()
            except Exception as error:
                # Keep unresolved reservations and a sanitized class. Do not put
                # API payloads, credentials, test answers or stack traces in UI.
                saved.update(phase='blocked', error_class=type(error).__name__)
                evidence = controller.root / 'evolution.json'
                if evidence.exists():
                    saved['charged_tokens'] = sum(json.loads(evidence.read_text())['charges'].values())
            write_private(path, state)
        return state
