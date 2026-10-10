#!/usr/bin/env python3
"""Real local ASR protocol/inference check. Never calls an LLM or prints keys."""
import argparse
import importlib.util
import json
from pathlib import Path
import time
import urllib.error
import urllib.request

SPEC=importlib.util.spec_from_file_location("asr_start",Path(__file__).resolve().parents[1]/"start-asr-local.py")
asr=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(asr)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--audio",type=Path,required=True)
    args=parser.parse_args()
    key=asr.configuration().read_text().strip()
    base="http://127.0.0.1:8796"
    with urllib.request.urlopen(base+"/health",timeout=5) as response:
        health=json.load(response)
    assert health["status"]=="ok" and health["model"]=="sensevoice-small"
    try:
        urllib.request.urlopen(base+"/v1/metrics",timeout=5)
        raise AssertionError("unauthenticated ASR access was accepted")
    except urllib.error.HTTPError as error:
        assert error.code==401
    data=args.audio.read_bytes()
    assert 0<len(data)<=8*1024*1024
    boundary="exoharness-asr-validation"
    body=(f'--{boundary}\r\nContent-Disposition: form-data; name="model"\r\n\r\nsensevoice-small\r\n'
          f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="sample.wav"\r\nContent-Type: audio/wav\r\n\r\n').encode()+data+f'\r\n--{boundary}--\r\n'.encode()
    started=time.monotonic()
    request=urllib.request.Request(base+"/v1/audio/transcriptions",data=body,headers={"Authorization":"Bearer "+key,"Content-Type":"multipart/form-data; boundary="+boundary})
    with urllib.request.urlopen(request,timeout=120) as response:
        result=json.load(response)
    assert result["text"].strip()
    request=urllib.request.Request(base+"/v1/metrics",headers={"Authorization":"Bearer "+key})
    with urllib.request.urlopen(request,timeout=5) as response:
        metrics=json.load(response)
    print(json.dumps({"health":health,"auth_rejection":401,"transcription":result,"seconds":round(time.monotonic()-started,3),"metrics":metrics},ensure_ascii=False))

if __name__=="__main__":main()
