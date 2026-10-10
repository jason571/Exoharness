#!/usr/bin/env python3
"""Exercise deployed Skill directory APIs. Only disposable test capabilities."""
import base64
import importlib.util
import json
from pathlib import Path
import time
import unittest
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("local_service", ROOT / "scripts/local-service.py")
local = importlib.util.module_from_spec(spec)
spec.loader.exec_module(local)
credentials = json.loads((local.DATA / "credentials.json").read_text())
key = next(item["token"] for item in credentials if item["id"] == "admin")


def request(method, path, value=None):
    body = None if value is None else json.dumps(value).encode()
    req = urllib.request.Request("http://127.0.0.1:9797" + path, data=body, method=method,
        headers={"Authorization": "Bearer " + key, "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=20) as reply:
            return reply.status, json.loads(reply.read())
    except urllib.error.HTTPError as error:
        return error.code, json.loads(error.read())


class DeployedSkillTests(unittest.TestCase):
    def test_directory_preview_import_refresh_update_delete(self):
        name = "local-validation-" + str(time.time_ns())
        text = f"---\nname: {name}\ndescription: Temporary deployed directory validation\n---\nExact 中文 instructions.\n"
        def package(text):
            return {"files": [{"path": name + "/SKILL.md", "content_base64": base64.b64encode(text.encode()).decode()},
                {"path": name + "/references/large.txt", "content_base64": base64.b64encode(b"x" * 70_000).decode()}]}
        status, preview = request("POST", "/v1/imports/skill/preview", package(text))
        self.assertEqual(status, 200); self.assertEqual(preview["action"], "create")
        status, imported = request("POST", "/v1/imports/skill", package(text))
        self.assertEqual(status, 200)
        record = imported["capabilities"][0]
        path = "/v1/capabilities/" + record["id"]
        try:
            status, read = request("GET", path + "/skill-instructions")
            self.assertEqual(status, 200); self.assertEqual(read["content"], text)
            self.assertEqual(request("GET", path + "/skill-files")[0], 200)
            next_text = text + "Updated instructions.\n"
            value = package(next_text)
            self.assertEqual(request("POST", "/v1/imports/skill", value)[0], 409)
            value["expected_revision"] = record["revision"]
            status, updated = request("POST", "/v1/imports/skill", value)
            self.assertEqual(status, 200); record = updated["capabilities"][0]
            self.assertEqual(request("GET", path + "/skill-instructions")[1]["content"], next_text)
            status, prepare = request("POST", path + "/preparation", {"confirm": True, "expected_revision": record["revision"]})
            self.assertEqual(status, 200); self.assertTrue(prepare["instructions_ready"])
            self.assertFalse(prepare["script_execution_ready"])
        finally:
            status, current = request("GET", path)
            if status == 200:
                self.assertEqual(request("DELETE", path, {"expected_revision": current["revision"]})[0], 200)
        self.assertEqual(request("GET", path)[0], 404)


if __name__ == "__main__":
    unittest.main(verbosity=2)
