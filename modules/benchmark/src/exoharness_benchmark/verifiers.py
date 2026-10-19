# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
"""Deterministic benchmark checks of answers and downloaded artifacts."""
import io
import json
from pathlib import PurePosixPath
import zipfile
import xml.etree.ElementTree as ET


def json_answer(text):
    text = text.strip()
    if text.startswith('```') and text.endswith('```'):
        text = text[text.index('\n') + 1:-3].strip()
    return json.loads(text)


def inspect_artifact(filename, raw, probe=None):
    result = {"extension": PurePosixPath(filename).suffix.lower(), "nonempty": bool(raw)}
    if result["extension"] == ".html":
        text = raw.decode("utf-8")
        result.update(valid_html="<html" in text.lower() or "<!doctype html" in text.lower(), text=text)
    elif result["extension"] in (".zip", ".pptx"):
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            members = archive.infolist()
            if len(members) > 2000 or sum(m.file_size for m in members) > 64 * 1024 * 1024:
                raise ValueError("Expanded artifact exceeds inspection limit")
            if any(PurePosixPath(m.filename).is_absolute() or ".." in PurePosixPath(m.filename).parts for m in members):
                raise ValueError("Unsafe archive member")
            if result["extension"] == ".pptx":
                slides = [m for m in members if m.filename.startswith("ppt/slides/slide") and m.filename.endswith(".xml")]
                text = " ".join("".join(ET.fromstring(archive.read(m)).itertext()) for m in slides)
                result.update(slides=len(slides), text=text)
            else:
                pages = [m for m in members if m.filename.lower().endswith(".html")]
                result.update(html_files=len(pages), text="\n".join(archive.read(m).decode("utf-8") for m in pages))
    elif probe is not None:
        result.update(probe)
    return result


def verify(task, outcome):
    checks = []
    for check in task["checks"]:
        kind, ok = check["kind"], False
        if kind == "json_equals":
            try:
                actual = json_answer(outcome.get("text", ""))
                ok = isinstance(actual, dict) and all(k in actual and type(actual[k]) is type(v) and actual[k] == v for k, v in check["fields"].items())
            except (ValueError, TypeError):
                pass
        elif kind == "text_contains":
            ok = all(value in outcome.get("text", "") for value in check["values"])
        elif kind == "tool_count":
            names = outcome.get('successful_tool_names' if check.get('successful_only') else 'tool_names', [])
            count = sum(check["name_contains"] in name for name in names)
            ok = check.get('min', 0) <= count <= check["max"]
        elif kind == "artifact":
            for item in outcome.get("artifacts", []):
                if item.get("extension") != check["extension"] or not item.get("nonempty"):
                    continue
                if check["extension"] == ".html" and not item.get("valid_html"):
                    continue
                if check["extension"] == ".zip" and not item.get("html_files"):
                    continue
                if "slides" in check and item.get("slides") != check["slides"]:
                    continue
                if any(value.casefold() not in item.get("text", "").casefold() for value in check.get("contains", [])):
                    continue
                if "min_duration" in check and item.get("duration", 0) < check["min_duration"]:
                    continue
                if "max_duration" in check and item.get("duration", float("inf")) > check["max_duration"]:
                    continue
                if check.get("audio") and not item.get("audio"):
                    continue
                if check["extension"] == ".mp4" and not item.get("video"):
                    continue
                if check["extension"] in (".mp3", ".wav") and not item.get("audio"):
                    continue
                ok = True
                break
        checks.append({"kind": kind, "passed": bool(ok)})
    return {"delivered": outcome.get("state") == "succeeded" and not outcome.get('artifact_errors') and all(c["passed"] for c in checks), "checks": checks}
