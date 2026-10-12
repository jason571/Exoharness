"""The live acceptance script must never spend tokens without explicit opt-in."""
from pathlib import Path
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("live_task_cost_profiles.py")


class TaskCostProfileSafetyTests(unittest.TestCase):
    def test_missing_opt_in_rejects_before_credentials_output_or_network(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "private-evidence"
            result = subprocess.run([
                "python3", str(SCRIPT), "--url", "http://127.0.0.1:1",
                "--output", str(output),
            ], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 2)
            self.assertIn("requires --allow-model-requests", result.stderr)
            self.assertFalse(output.exists())

    def test_help_documents_the_four_cases_without_loading_credentials(self):
        result = subprocess.run(["python3", str(SCRIPT), "--help"],
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0)
        for case in ["ppt-normal", "web-normal", "ppt-recovery", "web-recovery"]:
            self.assertIn(case, result.stdout)
