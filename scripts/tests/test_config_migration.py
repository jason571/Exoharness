import base64
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SPEC=importlib.util.spec_from_file_location("config_migration",Path(__file__).resolve().parents[1]/"import-server-config.py")
MODULE=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(MODULE)

def bundle():
    files={"models.json":json.dumps({"providers":[],"models":[]}).encode(),"agent-profiles.json":b'{"profiles":[]}',"server.env":b'EXOHARNESS_BIND=0.0.0.0:8787\n'}
    return {"version":1,"files":{name:base64.b64encode(value).decode() for name,value in files.items()}}

class MigrationTests(unittest.TestCase):
    def test_nested_secret_directories_are_all_private(self):
        with tempfile.TemporaryDirectory() as temporary:
            path=Path(temporary)/"generation/secrets/model-providers/key.secret"
            MODULE.write_private(path,b"test-only-secret")
            for directory in [path.parent,path.parent.parent,path.parent.parent.parent]:
                self.assertEqual(os.stat(directory).st_mode&0o077,0)
            self.assertEqual(os.stat(path).st_mode&0o077,0)

    def test_paths_duplicate_json_and_avatar_digest_are_rejected(self):
        value=bundle();value["files"]["../credentials.json"]=base64.b64encode(b"secret").decode()
        with self.assertRaises(ValueError):MODULE.validate_bundle(value)
        value=bundle();value["files"]["models.json"]=base64.b64encode(b'{"providers":[],"providers":[],"models":[]}').decode()
        with self.assertRaises(ValueError):MODULE.validate_bundle(value)
        value=bundle();value["files"]["agent-assets/"+"a"*64+".png"]=base64.b64encode(b"bad image").decode()
        with self.assertRaises(ValueError):MODULE.validate_bundle(value)

    def test_missing_configured_provider_key_is_rejected(self):
        value=bundle();value["files"]["models.json"]=base64.b64encode(json.dumps({"providers":[{"name":"api","api_key_configured":True}],"models":[]}).encode()).decode()
        with self.assertRaises(ValueError):MODULE.validate_bundle(value)

    def test_rejected_generation_does_not_replace_pointer_or_local_binds(self):
        with tempfile.TemporaryDirectory() as temporary:
            data=Path(temporary);config=data/"config.json";config.write_text('{"management_port":9797}')
            (data/"configuration").mkdir(mode=0o700)
            pointer=data/"configuration/current.json";pointer.write_text('{"generation":"old"}')
            previous=pointer.read_bytes()
            with patch.object(MODULE.subprocess,"run",return_value=subprocess.CompletedProcess([],1,"secret-response","secret-error")):
                with self.assertRaises(RuntimeError):MODULE.install(MODULE.validate_bundle(bundle()),data,"server",Path("/validator"))
            self.assertEqual(pointer.read_bytes(),previous);self.assertEqual(json.loads(config.read_text())["management_port"],9797)

    def test_success_is_atomic_private_preserves_originals_and_never_activates_env(self):
        with tempfile.TemporaryDirectory() as temporary:
            data=Path(temporary)
            report=json.dumps({"models":0,"providers":0,"credentials":0,"assistants":0,"avatars":0})
            with patch.object(MODULE.subprocess,"run",return_value=subprocess.CompletedProcess([],0,report,"")):
                result=MODULE.install(MODULE.validate_bundle(bundle()),data,"server",Path("/validator"))
            self.assertFalse(result["runtime_ready"]);self.assertFalse(result["server_environment_active"])
            generation=data/"configuration"/json.loads((data/"configuration/current.json").read_text())["generation"]
            self.assertEqual((generation/"server.env").read_bytes(),b'EXOHARNESS_BIND=0.0.0.0:8787\n')
            self.assertEqual(os.stat(generation/"server.env").st_mode&0o077,0)
            self.assertEqual(os.stat(generation).st_mode&0o077,0)

if __name__=="__main__":unittest.main()
