"""Explicit live tests; not part of offline unit discovery."""
import json
from pathlib import Path
import subprocess
import unittest
import urllib.error
import urllib.request
import uuid
import importlib.util

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location("local_service",ROOT/"scripts/local-service.py")
local=importlib.util.module_from_spec(spec);spec.loader.exec_module(local)
DATA=local.DATA
records=json.loads((DATA/"credentials.json").read_text())
KEYS={record["id"]:record["token"] for record in records}

def request(port,path,who=None,method="GET",payload=None,extra=None):
    headers=dict(extra or {})
    if who:headers["Authorization"]="Bearer "+KEYS[who]
    data=None
    if payload is not None:
        data=json.dumps(payload).encode();headers["Content-Type"]="application/json"
    req=urllib.request.Request(f"http://127.0.0.1:{port}{path}",data=data,headers=headers,method=method)
    try:
        with urllib.request.urlopen(req,timeout=4) as response:return response.status,json.load(response)
    except urllib.error.HTTPError as error:return error.code,json.load(error)

class LiveHttpTests(unittest.TestCase):
    def test_both_ports_run_the_new_binary_and_storage(self):
        for port,surface in [(9797,"management"),(9798,"portal")]:
            status,body=request(port,"/health")
            self.assertEqual(status,200);self.assertEqual(body["service"],"exoharness")
            self.assertEqual(body["surface"],surface);self.assertFalse(body["product_ready"])
            self.assertEqual(body["mode"],"infrastructure_validation")

    def test_role_origin_and_cross_user_access(self):
        self.assertEqual(request(9798,"/v1/local/status")[0],401)
        self.assertEqual(request(9797,"/v1/local/status","tester")[0],403)
        self.assertEqual(request(9798,"/v1/local/sessions","tester","POST",extra={"Origin":"https://evil.example"})[0],403)
        _,created=request(9798,"/v1/local/sessions","tester","POST")
        sid=created["session_id"]
        self.assertEqual(request(9798,f"/v1/local/sessions/{sid}/runs","tester2")[0],404)
        self.assertEqual(request(9797,f"/v1/local/sessions/{sid}/runs?owner=tester","admin")[0],200)

    def test_restart_preserves_original_run_and_event_cursor(self):
        _,created=request(9798,"/v1/local/sessions","tester","POST")
        sid=created["session_id"];path=f"/v1/local/sessions/{sid}/runs"
        body={"request_id":str(uuid.uuid4()),"expected_session_version":0,"input":"live restart/cancel evidence; no external execution"}
        status,created=request(9798,path,"tester","POST",body);self.assertEqual(status,201)
        run=created["run"];self.assertFalse(created["execution_submitted"])
        self.assertEqual(request(9798,path,"tester","POST",body)[1]["run"],run)
        path+="/"+run["id"]
        for action in ["start","pause"]:
            status,result=request(9798,path,"tester","PATCH",{"expected_version":run["version"],"action":action})
            self.assertEqual(status,200);run=result["run"]
        old_pid=json.loads((DATA/"service.json").read_text())["pid"]
        subprocess.run(["bash","scripts/local-service.sh","restart"],cwd=ROOT,check=True,timeout=45)
        self.assertNotEqual(json.loads((DATA/"service.json").read_text())["pid"],old_pid)
        self.assertEqual(request(9798,path,"tester")[1]["run"],run)
        cursor=run["version"]
        for action in ["resume","cancel"]:
            status,result=request(9798,path,"tester","PATCH",{"expected_version":run["version"],"action":action})
            self.assertEqual(status,200);run=result["run"]
        self.assertEqual(run["state"],"cancelled")
        self.assertEqual(request(9798,path,"tester","PATCH",{"expected_version":run["version"],"action":"resume"})[0],409)
        events=request(9798,path+f"/events?after={cursor}","tester")[1]["events"]
        self.assertEqual([event["sequence"] for event in events],[cursor+1,cursor+2])
        self.assertEqual(len(request(9798,f"/v1/local/sessions/{sid}/runs","tester")[1]["runs"]),1)

    def test_missing_agent_and_sandbox_do_not_submit_work(self):
        before=request(9798,"/v1/local/sessions","tester2")[1]["sessions"]
        for path,code in [("/v1/chat","agent_not_ready"),("/v1/execute/python","sandbox_not_ready"),("/v1/execute/shell","sandbox_not_ready"),("/v1/execute/skill","sandbox_not_ready")]:
            status,body=request(9798,path,"tester2","POST",{"message":"do not run", "code":"raise RuntimeError('must not execute')"})
            self.assertEqual(status,503);self.assertEqual(body["error"]["code"],code);self.assertFalse(body["resumable"])
        self.assertEqual(request(9798,"/v1/local/sessions","tester2")[1]["sessions"],before)

if __name__=="__main__":unittest.main(verbosity=2)
