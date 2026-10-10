#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
# Commercial licensing: contact flyangqn@gmail.com.

"""Local new-framework lesson MCP. Reuses an existing image, never builds/pulls it."""
import argparse
import os
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[1]
DATA=Path.home()/".local/state/exoharness-pro/lesson-renderer"
IMAGE="lesson-renderer:0.1.0-manim0.20.1"
NAMES=["exoharness-pro-lesson-server","exoharness-pro-lesson-worker"]
def call(*args):
    return subprocess.run(["docker",*args],check=True,text=True,capture_output=True).stdout.strip()
def stop():
    for name in NAMES:
        if call("ps","-aq","--filter",f"name=^{name}$"):
            label=call("inspect","--format",'{{index .Config.Labels "exoharness.project"}}',name)
            if label!="exoharness-pro-lesson": raise RuntimeError("foreign container; refusing removal")
            call("rm","-f",name)
def start():
    call("image","inspect",IMAGE)
    DATA.mkdir(parents=True,exist_ok=True,mode=0o700)
    token=ROOT/"secrets/lesson_mcp_token"
    if not token.is_file() or len(token.read_text().strip())<32: raise RuntimeError("missing local lesson token")
    stop()
    for name,module in zip(NAMES,["server","worker"]):
        args=["run","-d","--name",name,"--label","exoharness.project=exoharness-pro-lesson","--restart","unless-stopped","--user",f"{os.getuid()}:{os.getgid()}","--read-only","--cap-drop","ALL","--security-opt","no-new-privileges","--memory","4g","--cpus","4","--pids-limit","256","--network","host" if module=="server" else "none","--tmpfs","/tmp:rw,nosuid,size=1g"]
        for source,dest,readonly in [(ROOT/"mcp/lesson-renderer-mcp/src","/opt/lesson-renderer/src",True),(ROOT/"skills/elementary-lesson-video","/opt/lesson-skill",True),(token,"/run/secrets/token",True),(DATA,"/data",False)]:
            args += ["--mount",f"type=bind,src={source},dst={dest}"+(",readonly" if readonly else "")]
        for key,value in {"LESSON_TOKEN_FILE":"/run/secrets/token","LESSON_SKILL_DIR":"/opt/lesson-skill","LESSON_ASSET_BASE":"http://127.0.0.1:9790","LESSON_PUBLIC_BASE":"http://127.0.0.1:9791","LESSON_DATA_DIR":"/data","LESSON_JOB_DIR":"/data/jobs","LESSON_PORT":"9791","LESSON_HOST":"127.0.0.1"}.items():
            args += ["-e",f"{key}={value}"]
        call(*args,IMAGE,"python","-m",f"lesson_renderer.{module}")
    print("Local lesson MCP started on 127.0.0.1:9791; worker isolated without network; old services unchanged.")
if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument("action",choices=["start","stop","restart"])
    args=parser.parse_args()
    stop() if args.action=="stop" else start()
