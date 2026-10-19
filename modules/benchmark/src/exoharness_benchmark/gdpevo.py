# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Pinned GDPevo inputs and OCI-only official grading, outside the online Agent."""
import hashlib
import json
import re
import resource
from pathlib import Path
import subprocess
import tempfile
import uuid
from urllib.parse import urlsplit
from .contracts import digest
from .evidence import write_private
from .metrics import aggregate
from .verifiers import json_answer


class Dataset:
    def __init__(self, checkout, revision):
        self.root = Path(checkout).resolve()
        actual = subprocess.check_output(['git', '-C', str(self.root), 'rev-parse', 'HEAD'], text=True).strip()
        if not re.fullmatch('[0-9a-f]{40}', revision) or actual != revision:
            raise ValueError('Pin the exact upstream GDPevo commit')
        if subprocess.check_output(['git', '-C', str(self.root), 'status', '--porcelain', '--untracked-files=all'], text=True).strip():
            raise ValueError('Dataset checkout must be clean')
        self.revision = revision
        self.groups = self.root / 'data/task_groups'

    @classmethod
    def from_snapshot(cls, checkout, revision, manifest):
        """Verify a transported, frozen data-only export without a Git checkout."""
        if (not re.fullmatch('[0-9a-f]{40}', revision)
                or manifest.get('upstream_revision') != revision):
            raise ValueError('Snapshot revision differs from the pinned release')
        value = cls.__new__(cls)
        value.root, value.revision = Path(checkout).resolve(), revision
        value.groups = value.root / 'data/task_groups'
        if value.inventory() != manifest:
            raise ValueError('Snapshot content does not match its frozen inventory')
        return value

    def path(self, group, split, task):
        if not re.fullmatch(r'task_group_\d{3}', group) or split not in ('train', 'test') or not re.fullmatch(r'\d{3}', task):
            raise ValueError('Invalid official task identity')
        path = self.groups / group / (split + '_tasks') / task
        if not path.is_dir():
            raise ValueError('Official task does not exist')
        return path

    def read(self, path):
        # No links, repository traversal, or unbounded payload reads.
        path = Path(path)
        if not path.is_relative_to(self.root) or any(p.is_symlink() for p in [path, *path.parents] if p.is_relative_to(self.root)):
            raise ValueError('Dataset links are not allowed')
        raw = path.read_bytes()
        if len(raw) > 8 * 1024 * 1024:
            raise ValueError('Dataset file exceeds the bounded input size')
        return raw

    def file_sha256(self, path):
        path = Path(path)
        if (not path.is_relative_to(self.root) or path.stat().st_size > 64 * 1024 * 1024
                or any(p.is_symlink() for p in [path, *path.parents] if p.is_relative_to(self.root))):
            raise ValueError('Unsafe or oversized dataset file')
        with path.open('rb') as stream:
            return hashlib.file_digest(stream, 'sha256').hexdigest()

    def inventory(self):
        tasks = []
        for group in sorted(self.groups.glob('task_group_*')):
            for split in ('train', 'test'):
                for task in sorted((group / (split + '_tasks')).iterdir()):
                    if not task.is_dir():
                        continue
                    path = self.path(group.name, split, task.name)
                    files = {}
                    for item in sorted(path.rglob('*')):
                        if item.is_file():
                            files[str(item.relative_to(path))] = hashlib.sha256(self.read(item)).hexdigest()
                    if not {'input/prompt.txt', 'output/answer.json', 'eval/eval.sh'} <= set(files):
                        raise ValueError('Incomplete released task')
                    tasks.append({'group': group.name, 'split': split, 'task': task.name, 'files': files})
        environments = [self.environment_contract(group.name) for group in sorted(self.groups.glob('task_group_*'))]
        return {'version': 1, 'upstream_revision': self.revision, 'tasks': tasks,
                'environments': environments, 'dataset_sha256': digest({'tasks': tasks, 'environments': environments})}

    def solver_input(self, group, split, task, environment_url):
        url = urlsplit(environment_url)
        if (url.scheme not in ('http', 'https') or not url.hostname or url.username or url.password
                or url.query or url.fragment or url.path not in ('', '/')):
            raise ValueError('Use an explicit credential-free environment origin')
        path = self.path(group, split, task)
        # Only input/ crosses this boundary. No notes, eval, output, judge API,
        # shared judge_train_eval, or repository path is given to the solver.
        files = {str(p.relative_to(path / 'input')): self.read(p).decode('utf-8')
                 for p in sorted((path / 'input').rglob('*')) if p.is_file() and p.name != 'prompt.txt'}
        prompt = self.read(path / 'input/prompt.txt').decode('utf-8')
        return {'prompt': prompt, 'environment_origin': environment_url.rstrip('/'), 'input_files': files}

    def training_examples(self, group, environment_url):
        self.path(group, 'train', '001')
        return [{**self.solver_input(group, 'train', p.name, environment_url),
                 'answer': json.loads(self.read(p / 'output/answer.json'))}
                for p in sorted((self.groups / group / 'train_tasks').iterdir()) if p.is_dir()]

    def environment_contract(self, group):
        if not re.fullmatch(r'task_group_\d{3}', group) or not (self.groups / group).is_dir():
            raise ValueError('Invalid released group')
        env = self.groups / group / 'env'
        files = {str(p.relative_to(env)): self.file_sha256(p)
                 for p in sorted(env.rglob('*')) if p.is_file()}
        shared = {str(p.relative_to(env.parent)): self.file_sha256(p) for p in sorted(env.parent.rglob('*'))
                  if p.is_file() and p.relative_to(env.parent).parts[0] not in ('env', 'train_tasks', 'test_tasks')}
        metadata = self.read(env.parent / 'task_group.yaml').decode()
        mode = re.search(r'^  state_mode: (read_only|mutable)\s*$', metadata, re.M)
        if not mode:
            raise ValueError('Released environment state mode is missing')
        return {'upstream_revision': self.revision, 'group': group, 'state_mode': mode.group(1),
                'environment_sha256': digest({'files': files, 'shared': shared}), 'files': files, 'shared_files': shared,
                'solver_access': 'domain API only; deny /judge and train evaluator endpoints',
                'execution': 'separate OCI container; never source setup.sh on the host'}


class OciGrader:
    def __init__(self, dataset, image, *, command=subprocess.run):
        if not re.fullmatch(r'[\w./:-]+@sha256:[0-9a-f]{64}', image):
            raise ValueError('Pin a prebuilt Python/Bash grading image by digest')
        self.dataset, self.image, self.command = dataset, image, command

    def grade(self, group, split, task, prediction, output):
        path = self.dataset.path(group, split, task)
        # The solver's answer is data, never an executable path or command.
        json.dumps(prediction, allow_nan=False)
        with tempfile.TemporaryDirectory(prefix='exo-gdpevo-judge-') as tmp:
            folder = Path(tmp); folder.chmod(0o755)
            file = folder / 'prediction.json'; file.write_text(json.dumps(prediction)); file.chmod(0o444)
            container = 'exo-gdpevo-judge-' + uuid.uuid4().hex
            mounted = '/benchmark/' + group + '/' + split + '_tasks/' + task
            args = ['docker', 'run', '--rm', '--name', container, '--pull=never', '--network=none', '--read-only',
                    '--cap-drop=ALL', '--security-opt=no-new-privileges', '--pids-limit=64',
                    '--memory=256m', '--cpus=1', '--user=65534:65534',
                    '--tmpfs=/tmp:rw,noexec,nosuid,size=16m', '-e', 'PYTHONDONTWRITEBYTECODE=1',
                    '--mount', f'type=bind,src={self.dataset.groups / group},dst=/benchmark/{group},readonly',
                    '--mount', f'type=bind,src={file},dst=/prediction.json,readonly',
                    '--entrypoint=bash', self.image, mounted + '/eval/eval.sh', '/prediction.json']
            # Container output goes to a bounded private file, not a pipe held in
            # memory. The container is disposable and has no host credentials.
            with tempfile.TemporaryFile() as log:
                try:
                    result = self.command(args, stdout=log, stderr=subprocess.STDOUT, timeout=60,
                        preexec_fn=lambda: resource.setrlimit(resource.RLIMIT_FSIZE, (1048577, 1048577)))
                    log.seek(0); raw = log.read(1024 * 1024 + 1)
                finally:
                    self.command(['docker', 'rm', '-f', container], stdout=subprocess.DEVNULL,
                                 stderr=subprocess.DEVNULL, timeout=15)
            if result.returncode or len(raw) > 1024 * 1024:
                write_private(Path(output).with_suffix('.execution.json'), {'returncode': result.returncode,
                    'grading_image': self.image, 'diagnostic': raw[:65536].decode('utf-8', errors='replace')})
                raise RuntimeError('Official grader failed or exceeded its output bound')
        value = json.loads(raw)
        score = normalize_score(value)
        receipt = {'upstream_revision': self.dataset.revision, 'group': group, 'split': split,
                   'task': task, 'prediction_sha256': digest(prediction), 'grading_image': self.image,
                   'score': score, 'passed': score == 1, 'official_result': value}
        write_private(output, receipt)
        return receipt


def normalize_score(value):
    # Released graders use either normalized total_score, raw score/max_score,
    # or normalized score. Preserve the official payload alongside this ratio.
    if 'normalized_score' in value:
        score = value['normalized_score']
    elif 'total_score' in value:
        score = value['total_score']
    elif 'raw_score' in value or type(value.get('points')) in (int, float):
        # Some released graders call the normalized ratio "score" while
        # reporting its integer numerator separately as raw_score or points.
        score = value.get('score')
    else:
        score = value.get('score')
        maximum = value.get('max_score', 1)
        if type(maximum) not in (int, float) or maximum <= 0:
            raise ValueError('Invalid official maximum score')
        if type(score) in (int, float):
            score /= maximum
    if type(score) not in (int, float) or not 0 <= score <= 1:
        raise ValueError('Official grader returned an invalid score')
    return score


def run(dataset, group, port, grader, output, *, environment_url, repeats=3, max_tokens=2000000, owned_environment=False):
    """One explicitly authorized condition, frozen/resumable; no implicit model retries."""
    from .evidence import exclusive
    from .domain_gateway import DomainGateway
    output = Path(output)
    dataset.path(group, 'test', '001')
    if not owned_environment:
        raise ValueError('Use an explicitly owned disposable domain environment')
    mutable = dataset.environment_contract(group)['state_mode'] == 'mutable'
    if repeats not in (3, 4, 5) or max_tokens < port.run_ceiling:
        raise ValueError('Use 3..5 repeats and a bounded admissible Token budget')
    with exclusive(output):
        gateway_state = output / 'gateway.json'
        pin = json.loads(gateway_state.read_text()) if gateway_state.exists() else {}
        with DomainGateway(dataset, group, environment_url, port=pin.get('port', 0)) as gateway:
            write_private(gateway_state, {'port': gateway.server.server_port})
            return _run(dataset, group, port, grader, output, environment_url=environment_url,
                        repeats=repeats, max_tokens=max_tokens, gateway=gateway, mutable=mutable)


def _run(dataset, group, port, grader, output, *, environment_url, repeats, max_tokens, gateway, mutable):
    fingerprint = port.fingerprint()
    identity = {'revision': dataset.revision, 'inventory_sha256': dataset.inventory()['dataset_sha256'],
                'group': group, 'runtime': fingerprint, 'environment_url': environment_url, 'gateway': gateway.origin,
                'repeats': repeats, 'max_tokens': max_tokens, 'grading_image': grader.image}
    path = output / 'gdpevo.json'
    state = json.loads(path.read_text()) if path.exists() else {'identity': identity, 'charges': {}, 'rows': []}
    if state['identity'] != identity:
        raise ValueError('GDPevo run identity changed')
    tasks = sorted((dataset.groups / group / 'test_tasks').iterdir())
    for attempt in range(repeats):
        for task in tasks:
            key = task.name + '-' + str(attempt)
            if any(r['key'] == key for r in state['rows']):
                continue
            if port.fingerprint() != fingerprint:
                raise RuntimeError('Runtime drift; no new submission')
            if key not in state['charges']:
                if sum(state['charges'].values()) + port.run_ceiling > max_tokens:
                    raise RuntimeError('GDPevo Token admission limit reached')
                state['charges'][key] = port.run_ceiling; write_private(path, state)
            if mutable and not (output / key / 'execution.json').exists():
                gateway.reset_owned_mutable()
            source = dataset.solver_input(group, 'test', task.name, gateway.origin)
            outcome = port.execute(output / key, [json.dumps(source, ensure_ascii=False)], fingerprint)
            if outcome.get('tokens') is not None:
                state['charges'][key] = outcome['tokens']; write_private(path, state)
            if outcome.get('tokens') is None or outcome.get('runtime_drift') or outcome['tokens'] > port.run_ceiling:
                raise RuntimeError('Unknown or excess usage; stop without resubmission')
            try:
                prediction = json_answer(outcome['text'])
            except (ValueError, json.JSONDecodeError):
                prediction = {}  # Measured failure, not another model request.
            receipt = grader.grade(group, 'test', task.name, prediction, output / key / 'grade.json')
            state['rows'].append({'key': key, 'tokens': outcome['tokens'], 'state': outcome['state'],
                'delivered': outcome['state'] == 'succeeded' and receipt['passed'], 'score': receipt['score'],
                'run_ids': outcome['run_ids'], 'elapsed_ms': outcome.get('elapsed_ms')})
            write_private(path, state)
    report = {**identity, 'rows': state['rows'], 'summary': aggregate(state['rows']),
              'charged_tokens': sum(state['charges'].values()),
              'mean_official_score': sum(r['score'] for r in state['rows']) / len(state['rows'])}
    write_private(output / 'report.json', report)
    return report
