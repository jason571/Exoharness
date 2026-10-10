"""Explicit Docker/native switch acceptance. Restores the native service."""
import hashlib
import base64
import importlib.util
import json
from pathlib import Path
import subprocess
import unittest
import urllib.request
import uuid

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location("local_service",ROOT/"scripts/local-service.py")
local=importlib.util.module_from_spec(spec);spec.loader.exec_module(local)
records=json.loads((local.DATA/"credentials.json").read_text())
key=next(record["token"] for record in records if record["id"]=="tester")

def request(path,method="GET",payload=None,port=9798,token=key):
    headers={"Authorization":"Bearer "+token}
    data=None
    if payload is not None: data=json.dumps(payload).encode();headers["Content-Type"]="application/json"
    with urllib.request.urlopen(urllib.request.Request(f"http://127.0.0.1:{port}"+path,headers=headers,data=data,method=method),timeout=4) as result:
        return json.load(result)

def action(script,operation):
    subprocess.run(["bash",str(ROOT/"release/scripts"/script),operation],check=True,timeout=45,cwd=ROOT)

class DockerSwitchTests(unittest.TestCase):
    def test_same_release_security_and_persistence_across_modes(self):
        sid=request("/v1/local/sessions","POST")["session_id"]
        path=f"/v1/local/sessions/{sid}/runs"
        run=request(path,"POST",{"request_id":str(uuid.uuid4()),"expected_session_version":0,"input":"Docker/native persistence proof; no external work"})["run"]
        path+="/"+run["id"]
        admin=next(record["token"] for record in records if record["id"]=="admin")
        def manage(path,method="GET",payload=None):return request(path,method,payload,9797,admin)
        media_session=manage("/v1/local/sessions","POST")
        media_base="/v1/local/sessions/"+media_session["session_id"]
        media_run=manage(media_base+"/runs","POST",{"request_id":str(uuid.uuid4()),"expected_session_version":media_session["version"],"input":"File persistence acceptance"})["run"]
        media_path=media_base+"/runs/"+media_run["id"]
        artifact=manage(media_path+"/artifacts","POST",{"filename":"mode-test.mp3","content_base64":base64.b64encode(b"delivery persistence").decode()})
        def check_file():
            with urllib.request.urlopen(artifact["url"],timeout=4) as response:
                self.assertEqual(response.read(),b"delivery persistence")
                self.assertEqual(response.headers["Content-Type"],"audio/mpeg")
        check_file()
        action("start-native.sh","stop")
        try:
            action("start-docker.sh","start")
            self.assertEqual(request(path)["run"],run)
            check_file()
            config=json.loads(local.docker("inspect","exoharness-pro-local-service"))[0]
            self.assertTrue(config["HostConfig"]["ReadonlyRootfs"])
            self.assertEqual(config["HostConfig"]["NetworkMode"],"host")
            self.assertTrue(config["Config"]["User"])
            for mount in config["Mounts"]:
                self.assertEqual(mount["RW"], mount["Destination"] in ("/run/exoharness/capabilities", "/run/exoharness/configuration"))
                self.assertNotEqual(mount["Destination"], "/var/run/docker.sock")
            record=json.loads((local.DATA/"docker-service.json").read_text())
            self.assertEqual(record["sha256"],hashlib.sha256((local.RELEASE/"bin/exoharness").read_bytes()).hexdigest())
            action("start-docker.sh","restart")
            self.assertEqual(request(path)["run"],run)
            self.assertTrue(local.health(9797));self.assertTrue(local.health(9798));self.assertTrue(local.health(9790,"exoharness-media"))
            check_file()
        finally:
            action("start-docker.sh","stop")
            action("start-native.sh","start")
        self.assertEqual(request(path)["run"],run)
        check_file()
        current=manage(media_path)
        manage(media_path,"PATCH",{"expected_version":current["run"]["version"],"action":"cancel"})

if __name__=="__main__":unittest.main(verbosity=2)
