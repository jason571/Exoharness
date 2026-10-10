#!/usr/bin/env python3
"""Start existing standalone ASR with the imported private credential.

No build, pull, model download, production change, or secret in arguments/env.
The environment contains a file path only, not the credential value.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
from urllib.parse import urlsplit

SPEC=importlib.util.spec_from_file_location("local_service",Path(__file__).with_name("local-service.py"))
local=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(local)

def configuration():
    base=local.DATA/"configuration"
    pointer=json.loads((base/"current.json").read_text())
    name=pointer["generation"]
    if not name or not all(character.isascii() and (character.isalnum() or character=="-") for character in name):
        raise ValueError("invalid configuration generation")
    root=base/name
    catalog=json.loads((root/"models.json").read_text())
    providers=[entry for entry in catalog["providers"] if entry.get("model_type")=="asr" and entry.get("enabled",True)]
    if len(providers)!=1 or providers[0]["api"]!="openai_transcription":
        raise ValueError("one enabled transcription provider is required")
    provider=providers[0]
    endpoint=urlsplit(provider["base_url"])
    if endpoint.scheme!="http" or endpoint.hostname not in {"127.0.0.1","localhost"} or endpoint.port!=8796:
        raise ValueError("local ASR requires loopback port 8796; remote endpoint was not rewritten")
    key=root/"secrets/model-providers"/("model-provider-"+hashlib.sha256(provider["name"].encode()).hexdigest()+".secret")
    info=key.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_mode&0o077 or not info.st_size:
        raise ValueError("private ASR credential must be a mode-0600 regular file")
    return key

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--asr-root",type=Path,required=True)
    args=parser.parse_args()
    key=configuration()
    root=args.asr_root.resolve()
    if not (root/"models/sensevoice/model.pt").is_file() or not (root/"models/vad/model.pt").is_file():
        raise ValueError("prepared offline ASR weights are required")
    subprocess.run(["docker","image","inspect","speech-asr:sensevoice-1.0.0-cu128"],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True)
    environment=dict(os.environ)
    environment["EXOHARNESS_ASR_KEY_FILE"]=str(key)
    subprocess.run(["docker","compose","-f",str(root/"compose.yaml"),"-f",str(Path(__file__).resolve().parents[1]/"deploy/asr-local.yaml"),"up","-d","--no-build","--pull","never","asr"],env=environment,check=True)
    print("Existing ASR image started on loopback 8796; wait for real readiness before transcription.")

if __name__=="__main__":
    try:main()
    except (OSError,ValueError,KeyError,subprocess.SubprocessError):
        print("Local ASR preparation/start failed; no image was built or downloaded.",file=sys.stderr)
        raise SystemExit(1)
