"""Offline checks of the opt-in original-prompt acceptance harness."""
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import runpy
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import URLError


SCRIPT = Path(__file__).with_name("live_task_cost_profiles.py")


class RawWebsiteAcceptance(unittest.TestCase):
    def run_case(self, output, html, fail_submission=False):
        requests = []

        def response(request, timeout):
            body = json.loads(request.data) if getattr(request, "data", None) else None
            address = request if isinstance(request, str) else request.full_url
            requests.append((address, body))
            if address.endswith("/v1/sessions"):
                result = {"id": "session-private-fixture"}
            elif address.endswith("/turns"):
                if fail_submission:
                    raise URLError("ambiguous submission transport failure")
                result = {"run_id": "run-private-fixture"}
            elif "/v1/trace/runs/" in address:
                result = {"run": {"status": "succeeded", "last_sequence": 0}, "events": [], "model_calls": [], "invocations": []}
            elif "/v1/sessions/" in address:
                result = {"turns": [{"response": {"artifacts": [{"kind": "file", "filename": "index.html", "download_url": "https://media.invalid/index.html", "sha256": hashlib.sha256(html).hexdigest()}]}}]}
            else:
                return contextlib.closing(io.BytesIO(html))
            return contextlib.closing(io.BytesIO(json.dumps(result).encode()))

        argv = [str(SCRIPT), "--allow-model-requests", "--url", "https://api.invalid", "--output", str(output), "--case", "web-raw"]
        with patch("sys.argv", argv), patch.dict(os.environ, {"EXOHARNESS_TEST_ADMIN_TOKEN": "private-fixture-token"}), patch("urllib.request.urlopen", response), contextlib.redirect_stdout(io.StringIO()):
            runpy.run_path(str(SCRIPT), run_name="__main__")
        return requests

    def test_original_prompt_has_no_hidden_tool_or_product_hints(self):
        with tempfile.TemporaryDirectory() as output:
            requests = self.run_case(output, b"<!doctype html><h1>ExoHarness Rust Agent framework</h1>")
            submitted = [body for address, body in requests if address.endswith("/turns")]
            self.assertEqual(submitted, [{"prompt": "为ExoHarness生成宣传网站页面"}])
            report = json.loads((Path(output) / "report.json").read_text())
            self.assertTrue(report[0]["delivered_files"][0]["subject_verified"])

    def test_successful_run_with_wrong_subject_is_not_accepted(self):
        html = "<!doctype html><style>/* Rust Agent */</style><h1>ExoHarness 空间娱乐操作系统</h1><p>BOXLINK</p>".encode()
        with tempfile.TemporaryDirectory() as output:
            with self.assertRaisesRegex(AssertionError, "subject/content acceptance"):
                self.run_case(output, html)
            self.assertTrue((Path(output) / "web-raw-trace.json").exists())

    def test_ambiguous_submission_is_saved_and_never_automatically_repeated(self):
        with tempfile.TemporaryDirectory() as output:
            with self.assertRaises(URLError):
                self.run_case(output, b"", fail_submission=True)
            report = json.loads((Path(output) / "report.json").read_text())
            self.assertEqual(report[0]["status"], "submission_intent")
            self.assertEqual(self.run_case(output, b""), [])


if __name__ == "__main__":
    unittest.main()
