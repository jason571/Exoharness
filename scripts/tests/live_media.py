#!/usr/bin/env python3
"""Running 9797 -> original artifact tables -> 9790 signed stream/preview.

Fixture bytes validate delivery, not upstream GPU generation or media quality.
"""
import base64
import importlib.util
import json
from pathlib import Path
import time
import unittest
import urllib.error
import urllib.request

ROOT=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location("local_service",ROOT/"scripts/local-service.py")
local=importlib.util.module_from_spec(spec);spec.loader.exec_module(local)
key=next(item["token"] for item in json.loads((local.DATA/"credentials.json").read_text()) if item["id"]=="admin")
def api(method,path,value=None):
    req=urllib.request.Request("http://127.0.0.1:9797"+path,method=method,
        data=None if value is None else json.dumps(value).encode(),headers={"Authorization":"Bearer "+key,"Content-Type":"application/json"})
    with urllib.request.urlopen(req,timeout=20) as response:return response.status,json.loads(response.read())
def read(url,method="GET",range_value=None):
    headers={} if range_value is None else {"Range":range_value}
    req=urllib.request.Request(url,method=method,headers=headers)
    try:
        with urllib.request.urlopen(req,timeout=20) as response:return response.status,response.headers,response.read()
    except urllib.error.HTTPError as response:return response.code,response.headers,response.read()
class MediaDeliveryTests(unittest.TestCase):
    def test_one_permanent_port_for_all_media_and_isolated_html_preview(self):
        self.assertEqual(read("http://127.0.0.1:9790/health")[0],200)
        _,session=api("POST","/v1/local/sessions",{})
        base="/v1/local/sessions/"+session["session_id"]
        _,created=api("POST",base+"/runs",{"request_id":"media-test-"+str(time.time_ns()),"expected_session_version":session["version"],"input":"Disposable media delivery protocol validation"})
        run=created["run"];path=base+"/runs/"+run["id"]
        try:
            for name,mime in [("result.mp4","video/mp4"),("封面.png","image/png"),("歌曲.mp3","audio/mpeg")]:
                _,published=api("POST",path+"/artifacts",{"filename":name,"content_base64":base64.b64encode(b"0123456789").decode()})
                url=published["url"]
                self.assertTrue(url.startswith("http://127.0.0.1:9790/artifacts/"))
                self.assertIn("expires=0",url)
                status,headers,body=read(url)
                self.assertEqual(status,200);self.assertEqual(body,b"0123456789");self.assertEqual(headers["Content-Type"],mime)
                self.assertTrue(headers["Content-Disposition"].startswith("inline"))
                status,headers,body=read(url,range_value="bytes=2-5")
                self.assertEqual(status,206);self.assertEqual(body,b"2345");self.assertEqual(headers["Content-Range"],"bytes 2-5/10")
                self.assertTrue(read(published["download_url"])[1]["Content-Disposition"].startswith("attachment"))
                self.assertEqual(read(url.replace("signature=","signature=x"))[0],404)
            source="<!doctype html><h1>中文网页预览</h1>".encode()
            _,published=api("POST",path+"/artifacts",{"filename":"index.html","content_base64":base64.b64encode(source).decode()})
            status,headers,body=read(published["preview_url"])
            self.assertEqual(status,200);self.assertEqual(body,source)
            self.assertIn("sandbox allow-scripts",headers["Content-Security-Policy"])
            self.assertTrue(read(published["url"])[1]["Content-Disposition"].startswith("attachment"))
        finally:
            _,current=api("GET",path)
            api("PATCH",path,{"expected_version":current["run"]["version"],"action":"cancel"})
if __name__=="__main__":unittest.main(verbosity=2)
