"""Native binary test driver; no benchmark or evolution implementation."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / 'scripts/tests/fixtures/rust-evaluation'


class NativeEvaluationTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.root.chmod(0o700)

    def tearDown(self):
        self.temp.cleanup()

    def invoke(self, module, *args, success=True, no_path=False):
        folder = Path(os.environ.get('EXOHARNESS_EVALUATION_BIN_DIR', ROOT / 'target/release'))
        binary = folder / ('exoharness-' + module)
        self.assertTrue(binary.is_file(), 'Build the native evaluation binaries before these tests')
        env = dict(os.environ)
        if no_path:
            env['PATH'] = str(self.root / 'no-interpreters')
        result = subprocess.run([str(binary), *map(str, args)], text=True,
                                capture_output=True, timeout=90, env=env)
        if success:
            self.assertEqual(result.returncode, 0, result.stderr)
            return json.loads(result.stdout)
        self.assertNotEqual(result.returncode, 0)
        return result

    def validate(self, module, kind, value, *, success=True, extra=()):
        path = self.root / 'input.json'
        path.write_text(json.dumps(value, ensure_ascii=False))
        path.chmod(0o600)
        return self.invoke(module, 'validate', '--kind', kind, '--input', path,
                           *extra, success=success)

    def workflow(self):
        return json.loads((FIXTURES / 'workflow.json').read_text())
