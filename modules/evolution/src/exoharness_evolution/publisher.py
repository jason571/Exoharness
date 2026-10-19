# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Optimistic, reconcilable Skill publication through existing management APIs."""
import hashlib
import json
from pathlib import Path
import urllib.error
import urllib.parse
import urllib.request
from exoharness_benchmark.evidence import write_private
from exoharness_benchmark.harness import NoRedirect, origin
from .package import check_managed, package_digest, package_files, EXPERIENCE, PROVENANCE


class Management:
    def __init__(self, auth_file):
        path = Path(auth_file)
        if path.is_symlink() or path.stat().st_mode & 0o077:
            raise ValueError('Management credentials must be private')
        self.auth = json.loads(path.read_text())
        self.origin = origin(self.auth['management_origin'])
        self.opener = urllib.request.build_opener(NoRedirect)

    def api(self, path, payload=None, *, method=None, optional=False):
        request = urllib.request.Request(self.origin + path,
            data=None if payload is None else json.dumps(payload).encode(), method=method,
            headers={**self.auth['management_headers'], 'Origin': self.origin, 'Content-Type': 'application/json'})
        try:
            with self.opener.open(request, timeout=70) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            if optional and error.code == 404:
                return None
            raise RuntimeError('Skill management API returned HTTP ' + str(error.code)) from None

    def runtime(self):
        binary = self.auth.get('runtime_binary', '')
        if not binary.startswith('/proc/') or not binary.endswith('/exe'):
            raise ValueError('Pin the actual running production executable')
        with open(binary, 'rb') as stream:
            value = hashlib.file_digest(stream, 'sha256').hexdigest()
        if value != self.auth.get('runtime_version'):
            raise ValueError('Publication runtime changed')
        return value

    def configuration(self, assistant_id):
        capabilities = self.api('/v1/capabilities')
        evidence = []
        for record in sorted(capabilities, key=lambda r: r['id']):
            path = '/v1/capabilities/' + urllib.parse.quote(record['id'], safe='')
            value = {'id': record['id'], 'permissions': sorted(self.api(path + '/permissions'), key=lambda v: v['permission'])}
            if record['enabled'] and record['manifest']['spec']['kind'] == 'mcp_server':
                value['tools'] = self.api(path + '/tools')
            evidence.append(value)
        return {'profile': self.api('/v1/agent-profiles/' + urllib.parse.quote(assistant_id, safe='')),
                'models': self.api('/v1/model-catalog'), 'capabilities': capabilities, 'capability_evidence': evidence}


class Publisher:
    def __init__(self, client, policy):
        self.client, self.policy = client, policy
        self.path = '/v1/capabilities/' + urllib.parse.quote(policy.skill_id, safe='')

    def record(self):
        return self.client.api(self.path, optional=True)

    def existing(self):
        record = self.record()
        if record:
            view = self.client.api(self.path + '/skill-instructions')
            if view['revision'] != record['revision']:
                raise ValueError('Skill changed while reading its current version')
            check_managed(record, view['content'], self.policy)
        return record

    def snapshot(self):
        record = self.existing()
        if not record:
            return None
        value = self.client.api(self.path + '/skill-package?revision=' + str(record['revision']))
        if value['capability'] != record:
            raise ValueError('Immutable export does not match the current Skill')
        body = {'version': record['manifest']['version'], 'scope': self.policy.value['scope'],
                'permissions': [], 'files': value['files']}
        from .package import validate_inventory
        validate_inventory(package_files(body))
        if package_digest(body) != record['source_digest']:
            raise ValueError('Managed package contains unexpected files or a different digest')
        provenance = json.loads(package_files(body)[PROVENANCE])
        if provenance.get('policy_id') != self.policy.value['id']:
            raise ValueError('Skill is managed by another evolution policy')
        return body

    def publish(self, body, directory, *, expected_revision):
        directory = Path(directory); path = directory / 'publication.json'
        identity = {'skill_id': self.policy.skill_id, 'source_digest': package_digest(body),
                    'expected_revision': expected_revision, 'management_origin': self.client.origin}
        state = json.loads(path.read_text()) if path.exists() else {'identity': identity, 'phase': 'planned'}
        if state['identity'] != identity:
            raise ValueError('A pending publication cannot change identity')
        record = self.existing()
        if state['phase'] == 'ready':
            if not record or record['revision'] != state['revision'] or record['source_digest'] != identity['source_digest']:
                raise ValueError('Published Skill was changed by another administrator')
            return state
        if record and record['source_digest'] == identity['source_digest']:
            # Reconcile a lost import reply from its exact immutable digest.
            if state['phase'] not in ('importing', 'imported', 'preparing') and record['revision'] != expected_revision:
                raise ValueError('Unrelated revision cannot satisfy this publication')
        else:
            if (record['revision'] if record else None) != expected_revision:
                raise ValueError('Skill revision conflict; nothing was replaced')
            if state['phase'] == 'importing':
                raise RuntimeError('Import outcome unconfirmed; do not resubmit')
            state['phase'] = 'importing'; write_private(path, state)
            payload = {**body, **({'expected_revision': expected_revision} if expected_revision is not None else {})}
            result = self.client.api('/v1/imports/skill', payload)
            record = result['capabilities'][0]
            if record['id'] != identity['skill_id'] or record['source_digest'] != identity['source_digest']:
                raise ValueError('Import receipt does not match the prepared package')
        state.update(phase='imported', revision=record['revision']); write_private(path, state)
        # Preparation checks only the instruction-only package, and never grants permissions.
        status = self.client.api(self.path + '/preparation')
        if not record['enabled'] or not status.get('runtime_ready'):
            state['phase'] = 'preparing'; write_private(path, state)
            status = self.client.api(self.path + '/preparation', {'expected_revision': record['revision'], 'confirm': True})
        record = self.existing()
        if not record or record['source_digest'] != identity['source_digest'] or not status.get('runtime_ready'):
            raise ValueError('Published Skill is not ready; preserve the receipt for reconciliation')
        state.update(phase='ready', revision=record['revision']); write_private(path, state)
        return state

    def suspend(self, expected_revision, expected_digest):
        record = self.existing()
        if not record or record['revision'] != expected_revision or record['source_digest'] != expected_digest:
            raise ValueError('Refuse to suspend a different Skill version')
        return self.client.api(self.path + '/enabled', {'enabled': False, 'expected_revision': expected_revision}, method='PUT')
