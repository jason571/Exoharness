#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
# Commercial licensing: contact flyangqn@gmail.com.
"""Explicit GPU acceptance. Never retries a generation with an unknown outcome."""
import argparse
import asyncio
from contextlib import asynccontextmanager
from datetime import timedelta
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


async def run(args):
    import httpx
    from mcp import ClientSession
    from mcp.client.streamable_http import streamable_http_client

    token = os.environ[args.token_env]
    root = Path(args.output)
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    report = {"submissions": [], "completed": [], "cancelled": [], "checks": []}

    def save():
        path = root / "report.json"
        path.write_text(json.dumps(report, indent=2))
        path.chmod(0o600)

    @asynccontextmanager
    async def connection():
        async with httpx.AsyncClient(headers={"Authorization": "Bearer " + token},
                                     timeout=httpx.Timeout(90, connect=10)) as http:
            async with streamable_http_client(args.url, http_client=http) as streams:
                async with ClientSession(streams[0], streams[1],
                                         read_timeout_seconds=timedelta(seconds=90)) as client:
                    await client.initialize()
                    yield client

    async def call(client, name, arguments):
        result = await client.call_tool(name, arguments)
        assert not result.isError, "MCP rejected " + name + "; inspect the private service trace"
        if result.structuredContent is not None:
            return result.structuredContent
        return json.loads(next(part.text for part in result.content if part.type == "text"))

    async with connection() as client:
        tools = {tool.name: tool.inputSchema for tool in (await client.list_tools()).tools}
        assert {"submit_video_generation", "get_video_job", "get_video_result",
                "cancel_video_job", "get_video_service_status"} <= tools.keys()
        assert "simple" in tools["submit_video_generation"]["properties"]["prompt_mode"]["enum"]
        status = await call(client, "get_video_service_status", {})
        assert status["active_jobs"] == 0, "Existing production jobs: defer GPU acceptance"
        assert status["backend"]["ready"], "Video backend is not ready"
        before = await call(client, "list_video_jobs", {"limit": 100})
        rejected = await client.call_tool("submit_video_generation", {
            "prompt": "Invalid-mode acceptance: must not enqueue", "prompt_mode": "structured"})
        assert rejected.isError
        after = await call(client, "list_video_jobs", {"limit": 100})
        assert before == after, "Invalid input created a video job"
        report["checks"].append("invalid_mode_rejected_without_job")
        save()
        prompts = [
            "A warm sunlit forest, one small friendly robot waves gently, smooth slow camera push, no text, clear stable composition.",
            "A peaceful misty mountain lake at sunrise, slow lateral camera movement, soft cinematic light, no text, stable horizon.",
            "Queued cancellation acceptance, a still peaceful blue sky, no text.",
        ]
        for index, prompt in enumerate(prompts):
            # Persist intent BEFORE submitting. If this call loses its reply,
            # stop; the caller must inspect list_video_jobs before any resubmission.
            report["submission_intent"] = index
            save()
            value = await call(client, "submit_video_generation", {
                "prompt": prompt, "prompt_mode": "simple", "duration_seconds": 5,
                "resolution": "512p", "audio_enabled": False, "seed": 31012 + index})
            job_id = value.get("job_id") or value["id"]
            report["submissions"].append({"job_id": job_id, "initial_status": value["status"]})
            report.pop("submission_intent", None)
            save()
            print(json.dumps({"submitted": job_id, "status": value["status"]}), flush=True)
        cancel_id = report["submissions"][2]["job_id"]
        value = await call(client, "cancel_video_job", {"job_id": cancel_id})
        assert value["status"] in ("canceled", "cancelled", "cancel_requested", "running", "queued")
    report["checks"].append("disconnect_after_confirmed_submission")
    save()
    deadline = time.monotonic() + args.timeout
    observed = {item["job_id"]: set() for item in report["submissions"]}
    # A new transport queries the SAME jobs; generation is never resubmitted.
    async with connection() as client:
        while time.monotonic() < deadline:
            values = [await call(client, "get_video_job", {"job_id": item["job_id"]})
                      for item in report["submissions"]]
            for value in values:
                job_id = value.get("job_id") or value["id"]
                observed[job_id].add((value["status"], value.get("stage"), value.get("progress")))
            print(json.dumps({"status": [{"job_id": v.get("job_id") or v["id"],
                                          "status": v["status"], "progress": v.get("progress")}
                                         for v in values]}), flush=True)
            if all(v["status"] in ("completed", "failed", "canceled", "cancelled") for v in values):
                break
            await asyncio.sleep(5)
        else:
            report["checks"].append("deadline_reached_existing_jobs_retained")
            save()
            raise RuntimeError("Acceptance deadline reached; query existing recorded job IDs")
        assert values[2]["status"] in ("canceled", "cancelled")
        report["cancelled"].append(cancel_id)
        for index, value in enumerate(values[:2]):
            assert value["status"] == "completed", "GPU job did not complete; inspect service trace"
            job_id = report["submissions"][index]["job_id"]
            result = await call(client, "get_video_result", {"job_id": job_id})
            url = result["result"]["download_url"]
            assert url.startswith("https://media.flyang.art/")
            # Download via a separate unauthenticated client: never forward MCP auth.
            async with httpx.AsyncClient(timeout=60, follow_redirects=False) as files:
                response = await files.get(url)
                assert response.status_code == 200
                partial = await files.get(url, headers={"Range": "bytes=0-1023"})
                assert partial.status_code == 206 and partial.content == response.content[:1024]
            path = root / ("video-" + str(index + 1) + ".mp4")
            path.write_bytes(response.content)
            path.chmod(0o600)
            probe = json.loads(subprocess.check_output([
                "ffprobe", "-v", "error", "-show_entries", "format=duration:stream=codec_type,width,height",
                "-of", "json", str(path)], text=True))
            assert float(probe["format"]["duration"]) >= 4.5
            assert any(s["codec_type"] == "video" and s["width"] > 0 and s["height"] > 0
                       for s in probe["streams"])
            report["completed"].append({"job_id": job_id, "sha256": hashlib.sha256(response.content).hexdigest(),
                                        "bytes": len(response.content), "probe": probe,
                                        "observed_updates": len(observed[job_id]), "http": 200, "range_http": 206})
            save()
        status = await call(client, "get_video_service_status", {})
        assert status["active_jobs"] == 0
    report["checks"] += ["reconnected_without_resubmission", "queued_job_cancelled",
                         "two_real_mp4_downloads_and_range_requests", "queue_drained"]
    save()
    print(json.dumps({"passed": True, "report": str(root / "report.json"),
                      "completed": len(report["completed"]), "cancelled": len(report["cancelled"])}), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True)
    parser.add_argument("--token-env", default="H3_MCP_API_TOKEN")
    parser.add_argument("--output", required=True)
    parser.add_argument("--timeout", type=int, default=1800)
    parser.add_argument("--allow-generation", action="store_true", required=True)
    asyncio.run(run(parser.parse_args()))
