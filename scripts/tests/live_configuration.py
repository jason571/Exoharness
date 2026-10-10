#!/usr/bin/env python3
"""Authenticated deployed configuration comparison; secrets stay in memory."""
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest
import urllib.error
import urllib.request

ROOT=Path(__file__).resolve().parents[2]
SPEC=importlib.util.spec_from_file_location("local_service",ROOT/"scripts/local-service.py")
local=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(local)
credentials=json.loads((local.DATA/"credentials.json").read_text())
keys={record["id"]:record["token"] for record in credentials}
generation=json.loads((local.DATA/"configuration/current.json").read_text())["generation"]
data=local.DATA/"configuration"/generation

def request(port,path,who):
    request=urllib.request.Request(f"http://127.0.0.1:{port}{path}",headers={"Authorization":"Bearer "+keys[who]})
    try:
        with urllib.request.urlopen(request,timeout=5) as response:
            return response.status,response.headers,response.read()
    except urllib.error.HTTPError as error:
        return error.code,error.headers,error.read()

class DeployedConfigurationTests(unittest.TestCase):
    def test_model_catalog_matches_import_and_never_returns_keys(self):
        status,headers,raw=request(9797,"/v1/model-catalog","admin")
        self.assertEqual(status,200);self.assertEqual(headers["Cache-Control"],"no-store")
        self.assertEqual(json.loads(raw),json.loads((data/"models.json").read_text()))
        for path in (data/"secrets/model-providers").glob("*.secret"):
            self.assertNotIn(path.read_bytes(),raw)
        self.assertEqual(request(9797,"/v1/model-catalog","tester")[0],403)
        self.assertEqual(request(9798,"/v1/model-catalog","tester")[0],404)

    def test_assistants_current_revisions_selections_and_avatars_are_preserved(self):
        original=json.loads((data/"agent-profiles.json").read_text())
        status,_,raw=request(9797,"/v1/agent-profiles","admin");self.assertEqual(status,200)
        profiles=json.loads(raw);self.assertEqual(len(profiles),len(original["profiles"]))
        for record in original["profiles"]:
            profile=next(entry for entry in profiles if entry["id"]==record["id"])
            snapshot=next(entry for entry in record["revisions"] if entry["revision"]==record["active_revision"])
            for field in ["system_prompt","skill_ids","access","avatar","digest"]:
                self.assertEqual(profile.get(field),snapshot.get(field))
            self.assertEqual(profile["active_revision"],record["active_revision"])
            self.assertEqual(profile["enabled"],record["enabled"])
            if profile.get("avatar"):
                asset=profile["avatar"]
                status,headers,bytes_=request(9797,"/v1/agent-assets/"+asset["asset_id"],"admin")
                self.assertEqual(status,200);self.assertEqual(headers["Content-Type"],asset["content_type"])
                self.assertEqual(hashlib.sha256(bytes_).hexdigest(),Path(asset["asset_id"]).stem)
        status,_,raw=request(9798,"/v1/agent-profiles/default","tester");self.assertEqual(status,200)
        public=json.loads(raw);self.assertEqual(public["id"],original["default_profile"])
        self.assertNotIn("system_prompt",public);self.assertNotIn("access",public)

if __name__=="__main__":unittest.main(verbosity=2)
