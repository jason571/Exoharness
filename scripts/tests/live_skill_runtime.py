#!/usr/bin/env python3
"""Running API -> real cached OCI dependency check; uploaded code never runs."""
import base64
import time
import unittest
from live_skills import request

class SkillRuntimeTests(unittest.TestCase):
    def test_registered_revision_checks_python_without_granting_execution(self):
        name="environment-check-"+str(time.time_ns())
        files={"SKILL.md":f"---\nname: {name}\ndescription: Disposable environment check\n---\nExplicit isolated execution only.\n",
            "main.py":"raise RuntimeError('uploaded code must never execute during preparation')\n",
            "pyproject.toml":"[project]\nname = 'environment-check'\nversion = '0.0.0'\nrequires-python = '>=3.10'\n"}
        package={"files":[{"path":name+"/"+path,"content_base64":base64.b64encode(content.encode()).decode()} for path,content in files.items()]}
        status,result=request("POST","/v1/imports/skill",package)
        self.assertEqual(status,200);record=result["capabilities"][0];path="/v1/capabilities/"+record["id"]
        try:
            status,proof=request("POST",path+"/preparation",{"confirm":True,"expected_revision":record["revision"]})
            self.assertEqual(status,200,proof);self.assertTrue(proof["environment_ready"])
            self.assertEqual(proof["status"],"ready")
            status,record=request("GET",path)
            self.assertEqual(status,200);self.assertTrue(record["enabled"])
            self.assertFalse(proof["script_execution_ready"])
            configured=request("GET","/v1/local/runtime")[1]
            self.assertEqual(proof["evidence"]["image_id"],configured["image_id"])
            view=request("GET",path+"/runtime")[1]
            self.assertTrue(view["applicable"]);self.assertTrue(view["ready"])
            self.assertFalse(view["script_execution_ready"])
            next_package={**package,"expected_revision":record["revision"]}
            next_package["files"]=[dict(item) for item in package["files"]]
            next_package["files"][0]["content_base64"]=base64.b64encode((files["SKILL.md"]+"New revision.\n").encode()).decode()
            status,updated=request("POST","/v1/imports/skill",next_package)
            self.assertEqual(status,200);record=updated["capabilities"][0]
            self.assertFalse(request("GET",path+"/runtime")[1]["ready"],"revision update must invalidate prior evidence")
        finally:
            status,current=request("GET",path)
            if status==200:self.assertEqual(request("DELETE",path,{"expected_revision":current["revision"]})[0],200)

if __name__=="__main__":unittest.main(verbosity=2)
