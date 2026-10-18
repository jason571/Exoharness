#!/usr/bin/env python3
"""Opt-in real user acceptance. Never resubmit an uncertain generation.

The private auth file supplies separate portal and management headers. It must
belong to an owned test account, never to an existing user's browser session.
"""
import argparse
import base64
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
import urllib.error
import urllib.request
import urllib.parse
import zipfile
import xml.etree.ElementTree as ET

CASES = {
    "video": "为开源的ExoHarness生成15秒宣传视频，项目是Rust实现的通用Agent执行框架，提供模型接入、能力管理、沙盒执行和文件交付。科技感、AI元素，视频最后必须清楚显示 https://github.com/jason571/Exoharness 。请直接制作并交付可播放的视频文件。",
    "mv": "请使用本次上传的现成音频生成仙侠风格MV：雾中群山、白衣人物、飘动衣袖、古风意境。时长跟随上传音频，保持原音轨，不生成歌曲、配乐或歌词。请实际生成画面并合成，交付可播放下载的MP4文件。",
    "ppt": "生成一份可编辑的3页PPT，给一年级小朋友解释什么是人工智能。第一页是什么，第二页日常例子，第三页安全使用建议。字大、句短，带简单示意图。请直接交付实际PPTX文件。",
    "web": "为ExoHarness生成宣传网站页面。ExoHarness是Rust实现的通用Agent执行框架，提供模型接入、能力管理、隔离执行、持久化会话和文件交付，开源地址 https://github.com/jason571/Exoharness 。需要手机布局、功能介绍、可点击的中文/英文切换和GitHub入口。请直接交付能打开的网页及项目ZIP。",
    "music": "请将下面四句原创歌词制作成一首简短的中文古风歌曲，用古筝和笛子伴奏，有清晰的人声，交付能播放下载的音频文件。直接使用我提供的歌词，不需要自动写词。歌词：[Verse]\n山间清风过，云外月如歌。\n[Chorus]\n一盏星河落，与你看山河。",
}


def acceptance_cases(case_file=None, requested=None, mv_audio=None):
    cases = {name: {"kind": name, "prompt": prompt} for name, prompt in CASES.items()}
    if case_file:
        cases = json.loads(Path(case_file).read_text())
    elif "mv" in (requested or cases):
        if not mv_audio or not Path(mv_audio).is_file():
            raise ValueError("MV acceptance requires --mv-audio with existing audio; music generation is a separate case")
        cases["mv"]["attachment_paths"] = [{"type": "audio", "path": str(Path(mv_audio).resolve())}]
    if not all(re.fullmatch(r"[a-z][a-z0-9-]{0,60}", name) and spec["kind"] in CASES
        and isinstance(spec["prompt"], str) and spec["prompt"].strip() for name, spec in cases.items()):
        raise ValueError("Invalid acceptance cases")
    if requested and not set(requested) <= set(cases):
        raise ValueError("Unknown acceptance case")
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-generation", action="store_true", required=True)
    parser.add_argument("--auth-file", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--case", action="append")
    parser.add_argument("--cases-file", help="Private JSON cases with kind, prompt and optional owned session_id")
    parser.add_argument("--mv-audio", help="Existing audio to upload for the default MV case; never generates replacement music")
    parser.add_argument("--max-tokens", type=int, default=300000)
    parser.add_argument("--timeout", type=int, default=2400)
    parser.add_argument("--resume-existing", action="store_true", help="Observe saved Run IDs without resubmitting")
    args = parser.parse_args()
    try:
        cases = acceptance_cases(args.cases_file, args.case, args.mv_audio)
    except ValueError as error:
        parser.error(str(error))
    os.umask(0o077)
    authpath = Path(args.auth_file)
    assert authpath.stat().st_mode & 0o077 == 0, "Auth file must be private"
    auth = json.loads(authpath.read_text())
    root = Path(args.output)
    root.mkdir(parents=True, mode=0o700, exist_ok=True)
    reportfile = root / "report.json"
    rows = json.loads(reportfile.read_text()) if reportfile.exists() else []

    def save():
        temporary = reportfile.with_suffix(".new")
        temporary.write_text(json.dumps(rows, ensure_ascii=False, indent=2))
        temporary.replace(reportfile)

    def api(path, payload=None, admin=False):
        prefix = "management" if admin else "portal"
        headers = dict(auth[prefix + "_headers"])
        headers.update({"Content-Type": "application/json", "Origin": auth[prefix + "_origin"]})
        request = urllib.request.Request(auth[prefix + "_origin"] + path,
            data=None if payload is None else json.dumps(payload).encode(), headers=headers)
        try:
            with urllib.request.urlopen(request, timeout=70) as response:
                return json.load(response)
        except urllib.error.HTTPError as error:
            raise RuntimeError("API status " + str(error.code)) from None

    def probe(path):
        command = ["ffprobe", "-v", "error", "-show_entries", "format=duration:stream=codec_type,width,height", "-of", "json", str(path)]
        if not shutil.which("ffprobe"):
            command = ["docker", "run", "--rm", "--user", str(os.getuid()) + ":" + str(os.getgid()), "--network", "none", "--read-only", "--entrypoint", "ffprobe", "-v", str(root.resolve()) + ":/evidence:ro", "lesson-renderer:0.1.0-manim0.20.1", *command[1:-1], "/evidence/" + path.name]
        return json.loads(subprocess.check_output(command, text=True, stderr=subprocess.DEVNULL))

    for name, spec in cases.items():
        prompt, kind = spec["prompt"], spec["kind"]
        if args.case and name not in args.case:
            continue
        existing = next((r for r in rows if r["case"] == name), None)
        if existing:
            if not args.resume_existing or existing.get("artifact_acceptance"):
                continue
            assert existing.get("run_id"), "Submission outcome unknown; inspect Session before any repeat"
            row = existing
            session = {"id": row["session_id"]}
        else:
            session = api("/v1/sessions/" + spec["session_id"] + "?turn_limit=8") if spec.get("session_id") else api("/v1/sessions", {"title": "Generation acceptance " + name})
            assert session["agent_profile"]["id"] == auth["assistant_id"]
            row = {"case": name, "kind": kind, "session_id": session["id"], "status": "submission_intent", "actor_role": "user", "assistant_id": auth["assistant_id"], "prompt": prompt}
            rows.append(row)
            save()
        try:
            if not row.get("run_id"):
                payload = {"prompt": prompt}
                if spec.get("attachment_paths"):
                    payload["attachments"] = [{"type": item["type"], "name": Path(item["path"]).name,
                        "data": base64.b64encode(Path(item["path"]).read_bytes()).decode()} for item in spec["attachment_paths"]]
                submitted = api("/v1/sessions/" + session["id"] + "/turns", payload)
                row.update(run_id=submitted["run_id"], status="submitted")
                save()
            print(json.dumps({k: row[k] for k in ("case", "session_id", "run_id", "status")}), flush=True)
            deadline = time.monotonic() + args.timeout
            next_update = 0
            while True:
                trace = api("/v1/trace/runs/" + row["run_id"] + "?event_limit=500", admin=True)
                status = trace["run"]["status"]
                history = api("/v1/sessions/" + session["id"] + "?turn_limit=8")
                turn = history["turns"][-1]
                totals = []
                for call in trace["model_calls"]:
                    usage = call.get("usage") or call.get("response", {}).get("usage") or {}
                    total = usage.get("total_tokens")
                    if total is None and usage.get("input_tokens") is not None and usage.get("output_tokens") is not None:
                        total = usage["input_tokens"] + usage["output_tokens"]
                    if total is not None:
                        totals.append(total)
                measured = sum(totals) if totals else None
                if time.monotonic() >= next_update:
                    print(json.dumps({"case": name, "status": status, "model_requests": len(trace["model_calls"]), "observed_tokens": measured}), flush=True)
                    next_update = time.monotonic() + 45
                if status not in ("running", "queued", "pending", "waiting_authorization"):
                    break
                if time.monotonic() > deadline or (measured is not None and measured > args.max_tokens):
                    api("/v1/runs/" + row["run_id"] + "/cancel", {})
                    row["acceptance_limit_reached"] = True
                    save()
                    raise RuntimeError("Acceptance limit reached; saved Run was cancelled, not resubmitted")
                time.sleep(3)
            row.update(status=status, model_requests=len(trace["model_calls"]), capability_calls=len(trace["invocations"]), usage=(turn.get("response") or {}).get("usage"))
            (root / (name + "-trace.json")).write_text(json.dumps(trace, ensure_ascii=False))
            (root / (name + "-history.json")).write_text(json.dumps(history, ensure_ascii=False))
            row["delivered_files"] = []
            save()
            for index, card in enumerate((turn.get("response") or {}).get("artifacts", [])):
                url = card.get("download_url")
                if not url:
                    continue
                # Downloads deliberately carry no account or MCP credentials.
                with urllib.request.urlopen(url, timeout=120) as response:
                    raw = response.read()
                    content_type = response.headers.get("Content-Type", "")
                filename = Path(card.get("filename", "artifact-" + str(index))).name
                assert filename not in (".", "..", ""), "Invalid artifact filename"
                path = root / (name + "-" + str(index) + "-" + filename)
                path.write_bytes(raw)
                item = {"filename": filename, "local_file": path.name, "sha256": hashlib.sha256(raw).hexdigest(), "http": 200, "content_type": content_type}
                extension = path.suffix.lower()
                if extension in (".mp4", ".webm", ".mp3", ".wav", ".m4a", ".flac", ".ogg"):
                    item["probe"] = probe(path)
                    assert float(item["probe"]["format"]["duration"]) > 0
                if extension == ".pptx":
                    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
                        slides = [p for p in archive.namelist() if re.fullmatch(r"ppt/slides/slide\d+\.xml", p)]
                        item["slides"] = len(slides)
                        item["topic_present"] = any("人工智能" in "".join(ET.fromstring(archive.read(p)).itertext()) or "AI" in "".join(ET.fromstring(archive.read(p)).itertext()) for p in slides)
                if extension == ".zip":
                    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
                        item["project_files"] = archive.namelist()
                        pages = [p for p in archive.infolist() if p.filename.endswith(".html") and p.file_size <= 16 * 1024 * 1024]
                        item["project_subject_present"] = any(b"exoharness" in archive.read(p).lower() and b"rust" in archive.read(p).lower() for p in pages)
                if extension == ".html":
                    item["html_present"] = b"<html" in raw.lower() or b"<!doctype" in raw.lower()
                    item["subject_present"] = b"ExoHarness" in raw and b"Rust" in raw
                row["delivered_files"].append(item)
                save()
            files = row["delivered_files"]
            assert status == "succeeded", "Run did not succeed; inspect retained private Trace"
            if kind in ("video", "mv"):
                videos = [f for f in files if any(s["codec_type"] == "video" for s in f.get("probe", {}).get("streams", []))]
                assert videos, "No playable video delivered"
                assert any(float(f["probe"]["format"]["duration"]) >= 13.5 for f in videos), "Requested duration not delivered"
                if kind == "mv":
                    assert any(any(s["codec_type"] == "audio" for s in f["probe"]["streams"]) for f in videos), "MV lacks music"
            elif kind == "music":
                assert any(any(s["codec_type"] == "audio" for s in f.get("probe", {}).get("streams", [])) for f in files), "No playable music delivered"
            elif kind == "ppt":
                assert any(f.get("slides") == 3 and f.get("topic_present") for f in files), "Three-slide editable PPT not delivered"
            elif kind == "web":
                assert any((f.get("html_present") and f.get("subject_present")) or f.get("project_subject_present") for f in files), "ExoHarness HTML not delivered"
                assert any(any(p.endswith(".html") for p in f.get("project_files", [])) for f in files), "Web project ZIP not delivered"
            row["artifact_acceptance"] = "passed"
            row["visual_review"] = "pending"
        except Exception as error:
            row["acceptance_error"] = str(error) if isinstance(error, AssertionError | RuntimeError) else type(error).__name__
            row["artifact_acceptance"] = "failed"
        save()
        print(json.dumps({k: row.get(k) for k in ("case", "status", "artifact_acceptance", "usage", "acceptance_error")}, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()
