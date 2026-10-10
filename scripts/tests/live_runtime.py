#!/usr/bin/env python3
"""Verify the deployed provider, not claim a complete Agent execution path."""
import importlib.util
import json
from pathlib import Path
import unittest
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("local_service", ROOT / "scripts/local-service.py")
local = importlib.util.module_from_spec(spec)
spec.loader.exec_module(local)

class DeployedRuntimeTests(unittest.TestCase):
    def test_running_provider_uses_the_configured_immutable_image(self):
        config = json.loads((local.DATA / "config.json").read_text())
        credentials = json.loads((local.DATA / "credentials.json").read_text())
        key = next(item["token"] for item in credentials if item["id"] == "admin")
        request = urllib.request.Request("http://127.0.0.1:9797/v1/local/runtime", headers={"Authorization": "Bearer " + key})
        with urllib.request.urlopen(request, timeout=20) as response:
            value = json.loads(response.read())
        self.assertTrue(value["sandbox_ready"])
        self.assertEqual(value["image_id"], config["sandbox_image"])
        self.assertEqual(value["backend"], "linux_oci")
        self.assertEqual(value["network"], "disabled")
        self.assertEqual(value["runtimes"], ["python", "shell", "javascript", "typescript"])
        self.assertFalse(value["agent_integrated"])

if __name__ == "__main__":
    unittest.main(verbosity=2)
