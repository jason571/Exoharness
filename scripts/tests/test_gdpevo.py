"""Native Rust dataset checks using a disposable local Git fixture."""
import json
import subprocess
from rust_eval_test_support import NativeEvaluationTest


class NativeGDPevoTests(NativeEvaluationTest):
    def checkout(self):
        root = self.root / 'dataset'; group = root / 'data/task_groups/task_group_001'
        (group / 'env').mkdir(parents=True)
        (group / 'env/endpoints.txt').write_text('GET /api/data\nPOST /api/judge\nPOST /api/operator/reset\n')
        (group / 'task_group.yaml').write_text('env:\n  state_mode: read_only\n')
        for split in ('train', 'test'):
            task = group / (split + '_tasks') / '001'
            for name in ('input', 'output', 'eval', 'notes'):
                (task / name).mkdir(parents=True)
            (task / 'input/prompt.txt').write_text('Solve ' + split)
            (task / 'output/answer.json').write_text('{"private":"GOLD_SECRET"}')
            (task / 'eval/eval.sh').write_text('echo PRIVATE_JUDGE')
            (task / 'notes/private.txt').write_text('PRIVATE_NOTES')
        subprocess.run(['git', 'init', '-q', root], check=True)
        subprocess.run(['git', '-C', root, 'add', '.'], check=True)
        subprocess.run(['git', '-C', root, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid', 'commit', '-qm', 'fixture'], check=True)
        return root, subprocess.check_output(['git', '-C', root, 'rev-parse', 'HEAD'], text=True).strip()

    def test_solver_receives_no_answers_notes_or_judges(self):
        root, sha = self.checkout()
        result = self.invoke('benchmark', 'gdpevo', 'input', '--checkout', root, '--revision', sha,
                             '--group', 'task_group_001', '--task', '001', '--environment-origin', 'http://127.0.0.1:19999', '--output', self.root / 'input.json')
        self.assertEqual(result['prompt'], 'Solve test')
        for private in ('GOLD_SECRET', 'PRIVATE_JUDGE', 'PRIVATE_NOTES'):
            self.assertNotIn(private, json.dumps(result))

    def test_frozen_inventory_and_changed_checkout(self):
        root, sha = self.checkout()
        result = self.invoke('benchmark', 'gdpevo', 'inspect', '--checkout', root, '--revision', sha, '--output', self.root / 'inventory.json')
        self.assertEqual({t['split'] for t in result['tasks']}, {'train', 'test'})
        (root / 'data/task_groups/task_group_001/test_tasks/001/input/prompt.txt').write_text('changed')
        self.invoke('benchmark', 'gdpevo', 'inspect', '--checkout', root, '--revision', sha, '--output', self.root / 'bad.json', success=False)
