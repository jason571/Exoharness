"""Pinned official dataset boundaries and isolated grader adapter contracts."""
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import urllib.request
import urllib.error
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
for module in ('benchmark', 'evolution'):
    sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'modules' / module / 'src'))
from exoharness_benchmark.gdpevo import Dataset, OciGrader, normalize_score, run
from exoharness_benchmark.domain_gateway import DomainGateway


class GDPevoTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.root = Path(self.temp.name)
        (self.root / 'data/task_groups/task_group_001/env').mkdir(parents=True)
        (self.root / 'data/task_groups/task_group_001/task_group.yaml').write_text('env:\n  state_mode: read_only\n')
        (self.root / 'data/task_groups/task_group_001/env/endpoints.txt').write_text('GET /api/data\nGET /api/items/{id}\nPOST /api/judge\nPOST /api/operator/reset\n')
        for split in ('train', 'test'):
            path = self.root / 'data/task_groups/task_group_001' / (split + '_tasks/001')
            for name in ('input/payloads', 'output', 'eval', 'notes'): (path / name).mkdir(parents=True)
            (path / 'input/prompt.txt').write_text('Solve the ' + split + ' task')
            (path / 'input/payloads/template.json').write_text('{"amount":null}')
            (path / 'output/answer.json').write_text('{"amount":"GOLD_SECRET"}')
            (path / 'notes/notes.md').write_text('PRIVATE RUBRIC')
            (path / 'eval/eval.sh').write_text('echo PRIVATE EVALUATOR')
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        subprocess.run(['git', '-C', str(self.root), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(self.root), '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'dataset fixture'], check=True)
        self.sha = subprocess.check_output(['git', '-C', str(self.root), 'rev-parse', 'HEAD'], text=True).strip()
        self.dataset = Dataset(self.root, self.sha)
        self.image = 'python@sha256:' + 'a' * 64

    def tearDown(self): self.temp.cleanup()

    def test_input_contains_no_gold_notes_or_judge(self):
        value = self.dataset.solver_input('task_group_001', 'test', '001', 'http://127.0.0.1:9001')
        text = json.dumps(value)
        self.assertNotIn('GOLD_SECRET', text); self.assertNotIn('PRIVATE', text)
        self.assertEqual(set(value['input_files']), {'payloads/template.json'})

    def test_fewshot_examples_are_train_only(self):
        examples = self.dataset.training_examples('task_group_001', 'http://127.0.0.1:9001')
        self.assertEqual(len(examples), 1)
        self.assertIn('train task', examples[0]['prompt'])
        self.assertEqual(examples[0]['answer']['amount'], 'GOLD_SECRET')

    def test_inventory_preserves_official_splits_and_content_identity(self):
        result = self.dataset.inventory()
        self.assertEqual({t['split'] for t in result['tasks']}, {'train', 'test'})
        self.assertEqual(result['upstream_revision'], self.sha)
        self.assertEqual(len(result['dataset_sha256']), 64)

    def test_data_only_snapshot_rejects_mutation(self):
        manifest = self.dataset.inventory()
        clone = Dataset.from_snapshot(self.root, self.sha, manifest)
        self.assertEqual(clone.revision, self.sha)
        (self.root / 'data/task_groups/task_group_001/env/new.txt').write_text('changed')
        with self.assertRaisesRegex(ValueError, 'content'): Dataset.from_snapshot(self.root, self.sha, manifest)

    def test_revision_and_modified_checkout_rejected(self):
        with self.assertRaises(ValueError): Dataset(self.root, 'b' * 40)
        (self.root / 'untracked.txt').write_text('changed')
        with self.assertRaisesRegex(ValueError, 'clean'): Dataset(self.root, self.sha)

    def test_path_and_environment_origin_validation(self):
        for group, split, task in (('../x', 'test', '001'), ('task_group_001', 'output', '001'), ('task_group_001', 'test', '../001')):
            with self.assertRaises(ValueError): self.dataset.path(group, split, task)
        for value in ('https://user:secret@example.com', 'file:///etc/passwd', 'http://host/judge'):
            with self.assertRaises(ValueError): self.dataset.solver_input('task_group_001', 'test', '001', value)

    def test_symlink_input_is_refused(self):
        file = self.root / 'data/task_groups/task_group_001/test_tasks/001/input/leak'
        file.symlink_to('/etc/passwd')
        with self.assertRaisesRegex(ValueError, 'links'): self.dataset.solver_input('task_group_001', 'test', '001', 'http://localhost:9001')

    def test_grader_has_no_network_credentials_or_host_execution(self):
        calls = []
        def command(args, **kwargs):
            calls.append(args)
            if args[1] == 'run': kwargs['stdout'].write(b'{"total_score":1,"points":[]}')
            return subprocess.CompletedProcess(args, 0)
        result = OciGrader(self.dataset, self.image, command=command).grade('task_group_001', 'test', '001', {'amount': 'a'}, self.root / 'score.json')
        self.assertTrue(result['passed'])
        self.assertIn('--network=none', calls[0]); self.assertIn('--read-only', calls[0])
        self.assertIn('--user=65534:65534', calls[0]); self.assertIn('--pull=never', calls[0])
        self.assertEqual(calls[1][:3], ['docker', 'rm', '-f'])
        self.assertNotIn('GOLD_SECRET', json.dumps(calls[0]))

    def test_grader_timeout_cleans_own_container_without_host_fallback(self):
        calls = []
        def command(args, **kwargs):
            calls.append(args)
            if args[1] == 'run': raise subprocess.TimeoutExpired(args, 60)
            return subprocess.CompletedProcess(args, 0)
        with self.assertRaises(subprocess.TimeoutExpired):
            OciGrader(self.dataset, self.image, command=command).grade('task_group_001', 'test', '001', {}, self.root / 'score.json')
        self.assertEqual(len(calls), 2); self.assertEqual(calls[1][1], 'rm')

    def test_all_official_score_formats_are_normalized(self):
        for payload, expected in [({'total_score': .75, 'max_score': 20}, .75),
                                  ({'score': 7, 'max_score': 14}, .5), ({'score': .6}, .6),
                                  ({'score': 10, 'normalized_score': .5, 'max_score': 20}, .5),
                                  ({'score': 1, 'points': 19, 'max_score': 19}, 1),
                                  ({'score': 1, 'raw_score': 16, 'max_score': 16}, 1)]:
            self.assertEqual(normalize_score(payload), expected)
        for payload in ({'score': True}, {'score': 1, 'max_score': 0}, {'total_score': float('nan')}):
            with self.assertRaises(ValueError): normalize_score(payload)

    def test_domain_gateway_forwards_declared_routes_and_blocks_judges(self):
        calls = []
        class Upstream(BaseHTTPRequestHandler):
            def do_GET(inner):
                calls.append(('GET', inner.path, inner.headers.get('Authorization')))
                inner.send_response(200); inner.end_headers(); inner.wfile.write(b'{"records":[1]}')
            def do_POST(inner):
                calls.append(('POST', inner.path, None))
                inner.send_response(200); inner.end_headers(); inner.wfile.write(b'{}')
            def log_message(inner, *args): pass
        server = ThreadingHTTPServer(('127.0.0.1', 0), Upstream)
        worker = threading.Thread(target=server.serve_forever, daemon=True); worker.start()
        try:
            with DomainGateway(self.dataset, 'task_group_001', 'http://127.0.0.1:' + str(server.server_port)) as gateway:
                for path in ('/api/data', '/api/items/abc-123'):
                    request = urllib.request.Request(gateway.origin + path, headers={'Authorization':'Bearer never-forward'})
                    with urllib.request.urlopen(request) as response: self.assertEqual(response.status, 200)
                for path in ('/api/judge', '/api/operator/reset', '/api/%6audge', '/api/items/../judge', '/api/unknown'):
                    with self.subTest(path=path), self.assertRaises(urllib.error.HTTPError) as error:
                        urllib.request.urlopen(gateway.origin + path)
                    self.assertEqual(error.exception.code, 403)
                with self.assertRaises(urllib.error.HTTPError):
                    urllib.request.urlopen(urllib.request.Request(gateway.origin + '/api/data', data=b'{}'))
                gateway.reset_owned_mutable()
            self.assertEqual(calls, [('GET', '/api/data', None), ('GET', '/api/items/abc-123', None), ('POST', '/api/operator/reset', None)])
        finally:
            server.shutdown(); server.server_close(); worker.join(timeout=3)

    def test_owned_mutable_runner_resets_once_per_new_attempt_and_resumes(self):
        group = self.root / 'data/task_groups/task_group_001'
        (group / 'task_group.yaml').write_text('env:\n  state_mode: mutable\n')
        resets = []
        class Upstream(BaseHTTPRequestHandler):
            def do_POST(inner):
                resets.append(inner.path); inner.send_response(200); inner.end_headers(); inner.wfile.write(b'{}')
            def log_message(inner, *args): pass
        server = ThreadingHTTPServer(('127.0.0.1', 0), Upstream)
        worker = threading.Thread(target=server.serve_forever, daemon=True); worker.start()
        class Port:
            run_ceiling = 100
            calls = []
            def fingerprint(inner): return 'frozen-runtime'
            def execute(inner, directory, prompts, fingerprint):
                inner.calls.append(prompts)
                source = json.loads(prompts[0])
                self.assertNotIn('GOLD_SECRET', prompts[0])
                self.assertTrue(source['environment_origin'].startswith('http://127.0.0.1:'))
                directory.mkdir(parents=True, mode=0o700)
                (directory / 'execution.json').write_text('{"submitted":true}')
                return {'tokens':10, 'runtime_drift':False, 'state':'succeeded', 'text':'{}', 'run_ids':[directory.name]}
        class Grader:
            image = self.image
            def grade(inner, *args): return {'passed':True,'score':1}
        port = Port(); origin = 'http://127.0.0.1:' + str(server.server_port)
        try:
            with self.assertRaisesRegex(ValueError, 'owned'):
                run(self.dataset, 'task_group_001', port, Grader(), self.root / 'run', environment_url=origin)
            first = run(self.dataset, 'task_group_001', port, Grader(), self.root / 'run', environment_url=origin,
                        max_tokens=1000, owned_environment=True)
            second = run(self.dataset, 'task_group_001', port, Grader(), self.root / 'run', environment_url=origin,
                         max_tokens=1000, owned_environment=True)
            self.assertEqual(first['charged_tokens'], 30); self.assertEqual(second['charged_tokens'], 30)
            self.assertEqual(len(port.calls), 3); self.assertEqual(resets, ['/api/operator/reset'] * 3)
        finally:
            server.shutdown(); server.server_close(); worker.join(timeout=3)

    def test_invalid_grader_result_fails_closed(self):
        def command(args, **kwargs):
            if args[1] == 'run': kwargs['stdout'].write(b'{"total_score":1.5}')
            return subprocess.CompletedProcess(args, 0)
        with self.assertRaises(ValueError): OciGrader(self.dataset, self.image, command=command).grade('task_group_001', 'test', '001', {}, self.root / 'score.json')
        with self.assertRaises(ValueError): OciGrader(self.dataset, 'python:latest')

if __name__ == '__main__': unittest.main()
