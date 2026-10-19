# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""ExoHarness benchmark adapter using existing authenticated HTTP interfaces."""
import hashlib
import json
from pathlib import Path
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request

from .contracts import digest, integer
from .evidence import write_private
from .metrics import usage
from .verifiers import inspect_artifact


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise ValueError("Redirect refused; credentials and downloads remain origin-bound")


def origin(value):
    parsed = urllib.parse.urlsplit(value)
    if parsed.scheme not in ('http', 'https') or not parsed.hostname or parsed.username or parsed.password or parsed.path not in ('', '/') or parsed.query or parsed.fragment:
        raise ValueError("An API endpoint must be an origin without credentials")
    if parsed.scheme == 'http' and parsed.hostname not in ('127.0.0.1', 'localhost', '::1'):
        raise ValueError("Plaintext remote evaluation endpoint refused")
    return value.rstrip('/')


class Harness:
    def __init__(self, auth_file, run_ceiling=300000, timeout=2400):
        path = Path(auth_file)
        if path.is_symlink() or path.stat().st_mode & 0o077:
            raise ValueError("Evaluation auth must be a private regular file")
        self.auth = json.loads(path.read_text())
        self.portal = origin(self.auth['portal_origin'])
        self.management = origin(self.auth['management_origin'])
        self.artifact_origin = origin(self.auth['artifact_origin'])
        extra = self.auth.get('additional_artifact_origins', [])
        if not isinstance(extra, list) or len(extra) > 4:
            raise ValueError('A bounded artifact-origin allowlist is required')
        self.artifact_origins = {self.artifact_origin, *(origin(value) for value in extra)}
        self.user_id = self.auth['user_id']
        if not self.auth.get('owned_test_account') or not self.user_id.startswith('exo_eval_'):
            raise ValueError("Use a separately owned exo_eval_ test account")
        if not integer(run_ceiling, 1) or not integer(timeout, 1):
            raise ValueError("Positive execution limits required")
        if not integer(self.auth.get('run_token_ceiling'), 1) or run_ceiling < self.auth['run_token_ceiling']:
            raise ValueError('Reservation must cover the attested server per-Run Token ceiling')
        self.run_ceiling, self.timeout = run_ceiling, timeout
        self.opener = urllib.request.build_opener(NoRedirect)

    def api(self, path, payload=None, admin=False):
        prefix = 'management' if admin else 'portal'
        headers = {**self.auth[prefix + '_headers'], 'Origin': self.management if admin else self.portal,
                   'Content-Type': 'application/json'}
        request = urllib.request.Request((self.management if admin else self.portal) + path,
            data=None if payload is None else json.dumps(payload, ensure_ascii=False).encode(), headers=headers)
        try:
            with self.opener.open(request, timeout=70) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            raise RuntimeError('Evaluation API returned HTTP ' + str(error.code)) from None

    def configuration(self):
        # Run the evaluator alongside the target service. Reading /proc/<pid>/exe
        # verifies the active process, including replacement/restart detection.
        binary = self.auth.get('runtime_binary')
        if not isinstance(binary, str) or not binary.startswith('/proc/') or not binary.endswith('/exe'):
            raise ValueError('A readable running-process binary is required for runtime pinning')
        with open(binary, 'rb') as stream:
            actual = hashlib.file_digest(stream, 'sha256').hexdigest()
        if actual != self.auth.get('runtime_version'):
            raise ValueError('Running service binary changed')
        account = self.api('/v1/account')
        account = account.get('account', account)
        if account.get('id') != self.user_id:
            raise ValueError('Evaluation principal does not match the owned account')
        profile = self.api('/v1/agent-profile')
        if profile.get('id') != self.auth['assistant_id']:
            raise ValueError('Evaluation assistant changed')
        capabilities = self.api('/v1/capabilities', admin=True)
        evidence = []
        for record in sorted(capabilities, key=lambda value: value['id']):
            path = '/v1/capabilities/' + urllib.parse.quote(record['id'], safe='')
            permissions = self.api(path + '/permissions', admin=True)
            value = {'id': record['id'], 'permissions': sorted(permissions, key=lambda item: item['permission'])}
            if record['enabled'] and record['manifest']['spec']['kind'] == 'mcp_server':
                value['tools'] = self.api(path + '/tools', admin=True)
            evidence.append(value)
        # Validation and permission decisions can change without incrementing
        # the manifest revision. Freeze the actual discoverable schemas/grants.
        return {'profile': profile, 'models': self.api('/v1/model-catalog', admin=True),
                'capabilities': capabilities, 'capability_evidence': evidence}

    def configuration_fingerprint(self):
        return digest(self.configuration())

    def fingerprint(self):
        return digest({**self.configuration(), 'runtime_version': self.auth['runtime_version']})

    def artifact(self, card, directory, index):
        url = card.get('download_url')
        if not url:
            return None
        parsed = urllib.parse.urlsplit(url)
        if parsed.username or parsed.password or parsed.fragment or origin(urllib.parse.urlunsplit((parsed.scheme, parsed.netloc, '', '', ''))) not in self.artifact_origins:
            raise ValueError('Artifact origin is outside the evaluation allowlist')
        # No Portal cookie, CSRF token or management key is forwarded.
        with self.opener.open(urllib.request.Request(url), timeout=120) as response:
            raw = response.read(128 * 1024 * 1024 + 1)
        if len(raw) > 128 * 1024 * 1024:
            raise ValueError('Artifact exceeds download limit')
        name = Path(card.get('filename', 'artifact')).name
        suffix = Path(name).suffix.lower()
        path = directory / f'artifact-{index}{suffix}'
        path.write_bytes(raw)
        path.chmod(0o600)
        probe = None
        if suffix in ('.mp4', '.mp3', '.wav'):
            value = json.loads(subprocess.check_output(['ffprobe', '-v', 'error',
                '-protocol_whitelist', 'file,pipe', '-show_entries',
                'format=duration:stream=codec_type', '-of', 'json', str(path)],
                timeout=30, stderr=subprocess.DEVNULL))
            probe = {'duration': float(value['format']['duration']),
                     'audio': any(s['codec_type'] == 'audio' for s in value['streams']),
                     'video': any(s['codec_type'] == 'video' for s in value['streams'])}
        return {**inspect_artifact(name, raw, probe), 'sha256': hashlib.sha256(raw).hexdigest()}

    def execute(self, directory, prompts, fingerprint, attachments=()):
        directory = Path(directory)
        directory.mkdir(mode=0o700, parents=True, exist_ok=True)
        state_path, result_path = directory / 'execution.json', directory / 'outcome.json'
        input_digest = digest({'prompts': prompts, 'attachments': attachments})
        state = json.loads(state_path.read_text()) if state_path.exists() else {
            'turns': [], 'input_sha256': input_digest, 'started_at_unix_ms': time.time_ns() // 1000000}
        if state.get('input_sha256') != input_digest:
            raise ValueError('Saved evaluation input changed')
        if result_path.exists():
            result = json.loads(result_path.read_text())
            if result.get('runtime_fingerprint') != fingerprint:
                raise ValueError('Cached outcome belongs to a different runtime')
            return result
        # An ambiguous Session POST or Turn POST is never repeated on restart.
        if state.get('session_intent') and not state.get('session_id'):
            raise RuntimeError('Session submission unknown; inspect retained intent before continuing')
        if not state.get('session_id'):
            state['session_intent'] = True
            write_private(state_path, state)
            session = self.api('/v1/sessions', {'title': 'Offline evaluation ' + directory.name})
            if session.get('agent_profile', {}).get('id') != self.auth['assistant_id']:
                raise ValueError('New Session did not freeze the expected assistant')
            state['session_id'] = session['id']
            write_private(state_path, state)
        all_calls, tools, run_ids, states, skill_reads, successful_tools, resource_reads = [], [], [], [], [], [], []
        text, cards = '', []
        for index, prompt in enumerate(prompts):
            if index >= len(state['turns']):
                state['turns'].append({'intent': True, 'prompt_sha256': digest(prompt)})
                write_private(state_path, state)
                payload = {'prompt': prompt}
                if index == 0 and attachments:
                    payload['attachments'] = attachments
                submitted = self.api('/v1/sessions/' + state['session_id'] + '/turns', payload)
                state['turns'][index]['run_id'] = submitted['run_id']
                write_private(state_path, state)
            turn = state['turns'][index]
            if turn['prompt_sha256'] != digest(prompt) or not turn.get('run_id'):
                raise RuntimeError('Turn submission unknown or changed; no automatic resubmission')
            run_id = turn['run_id']
            run_ids.append(run_id)
            deadline = time.monotonic() + self.timeout
            while True:
                trace = self.api('/v1/trace/runs/' + run_id + '?event_limit=1', admin=True)
                status = trace['run']['status']
                measured = usage(trace['model_calls'])
                if status not in ('running', 'queued', 'pending', 'waiting_authorization'):
                    break
                if time.monotonic() >= deadline or measured['total_tokens'] >= self.run_ceiling:
                    self.api('/v1/runs/' + run_id + '/cancel', {})
                    # Keep the reservation and stop the experiment. A late receipt
                    # can be observed by rerunning; no replacement Run is created.
                    raise RuntimeError('Evaluation Run cancelled at its observation limit; inspect saved Run')
                time.sleep(1)
            write_private(directory / f'trace-{index}.json', trace)
            all_calls.extend(trace['model_calls'])
            tools.extend(i.get('tool_name', '') for i in trace['invocations'])
            for invocation in trace['invocations']:
                result = invocation.get('result') or {}
                if result.get('_host_execution') is True and not result.get('isError') and not result.get('is_error') and invocation.get('outcome_status') == 'success':
                    successful_tools.append(invocation.get('tool_name', ''))
                if invocation.get('tool_name') == 'read_skill' and result.get('content') and not result.get('isError'):
                    skill_reads.append({'id': (invocation.get('arguments') or {}).get('id'),
                                        'revision': result.get('revision'), 'run_id': run_id})
                    if result.get('path') == 'resource-index.json':
                        resource_reads.append({'id': (invocation.get('arguments') or {}).get('id'),
                            'revision': result.get('revision'), 'run_id': run_id,
                            'resources': result.get('selected_resources', [])})
            states.append(status)
            history = self.api('/v1/sessions/' + state['session_id'] + '?turn_limit=8')
            saved = next((t for t in history['turns'] if t.get('run_id') == run_id), None)
            if saved is None:
                # Some history projections identify a Turn by its run list only.
                saved = history['turns'][-1]
            response = saved.get('response') or {}
            text = response.get('answer') or response.get('text') or saved.get('answer') or ''
            cards.extend(response.get('artifacts') or [])
        measured = usage(all_calls)
        artifacts, seen, artifact_errors = [], set(), []
        for card in cards:
            key = card.get('artifact_id') or card.get('download_url')
            if key and key not in seen:
                seen.add(key)
                try:
                    item = self.artifact(card, directory, len(artifacts))
                    if item:
                        artifacts.append(item)
                except Exception as error:
                    # Download/probe rejection must not lose known model usage
                    # or trigger another model/Turn submission on restart.
                    artifact_errors.append({'index':len(seen)-1,'code':type(error).__name__})
        bindings = [list(binding) for binding in sorted({(c.get('provider'), c.get('model')) for c in all_calls})]
        result = {'state': states[-1], 'turn_states': states, 'text': text,
                  'artifacts': artifacts, 'run_ids': run_ids, 'tool_names': tools,
                  'successful_tool_names': successful_tools,
                  'artifact_errors': artifact_errors,
                  'usage': measured, 'tokens': None if measured['unknown_model_calls'] else measured['total_tokens'],
                  'runtime_fingerprint': fingerprint, 'runtime_drift': self.fingerprint() != fingerprint,
                  'model_bindings': bindings}
        result['skill_reads'] = skill_reads
        result['skill_resource_reads'] = resource_reads
        result['elapsed_ms'] = (time.time_ns() // 1000000 - state['started_at_unix_ms']
                                if 'started_at_unix_ms' in state else None)
        durations = [call.get('duration_ms') for call in all_calls]
        result['model_duration_ms'] = sum(durations) if all(integer(t) for t in durations) else None
        write_private(result_path, result)
        return result
