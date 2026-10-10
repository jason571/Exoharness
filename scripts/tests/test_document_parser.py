import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DocumentParserTests(unittest.TestCase):
    def test_real_isolated_docx_pdf_parser(self):
        image = os.environ.get("EXOHARNESS_PRO_TEST_SANDBOX_IMAGE")
        self.assertTrue(image, "prepared local sandbox image required; no silent skip")
        with tempfile.TemporaryDirectory(prefix="exoharness-document-test-") as folder:
            path = Path(folder)
            path.chmod(0o777)  # test-only, non-sensitive fixture mount for UID1000
            for source in (ROOT / "modules/api/src/knowledge/imports/parse_document.py",
                           ROOT / "modules/api/tests/support/document_fixtures.py"):
                (path / source.name).write_bytes(source.read_bytes())
                (path / source.name).chmod(0o444)
            result = subprocess.run([
                "docker", "run", "--rm", "--pull=never", "--read-only", "--network", "none",
                "--cap-drop", "ALL", "--security-opt", "no-new-privileges", "--user", "1000:1000",
                "--memory", "536870912", "--pids-limit", "64", "--cpus", "1",
                "--mount", f"type=bind,src={folder},dst=/workspace", "--workdir", "/workspace",
                "--entrypoint", "python", image, "-B", "document_fixtures.py", "-v"
            ], capture_output=True, text=True, timeout=90)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("Ran 8 tests", result.stderr)
