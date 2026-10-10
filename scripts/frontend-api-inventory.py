#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
# Commercial licensing: contact flyangqn@gmail.com.

"""Static route coverage inventory, NOT runtime/method/payload acceptance.

Only inspects literal/template /v1 paths in authored frontend and literal
Axum route declarations. Dynamic URL concatenations require manual review.
No network, credentials, data mutation, or false success placeholders.
"""
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def inventory(root=ROOT):
    routes = {}
    for file in sorted((root / "modules/api/src").rglob("*.rs")):
        for match in re.finditer(r'\.route\(\s*"([^"\n]+)"', file.read_text()):
            routes.setdefault(match[1], []).append(str(file.relative_to(root)))
    compiled = [(re.compile("^" + re.sub(r'\\\{[^}]+\\\}', r'[^/]+', re.escape(path)) + "$"), path)
                for path in routes]
    refs = {}
    for directory in ["web-console/src", "web"]:
        for file in sorted((root / directory).rglob("*")):
            if file.suffix not in [".ts", ".js"]:
                continue
            if directory == "web" and file.stem in {"chat", "portal-settings", "knowledge", "agents", "models"}:
                continue  # Generated bundles duplicate authored TypeScript.
            text = file.read_text()
            for match in re.finditer(r'(["\x27`])(/v1/[^\n]*?)\1', text):
                literal = match[2]
                path = re.sub(r'\$\{.*?\}', "{value}", literal).split("?")[0]
                line = text.count("\n", 0, match.start()) + 1
                refs.setdefault(path, []).append(f"{file.relative_to(root)}:{line}")
    entries = []
    for path, source in sorted(refs.items()):
        sample = path.replace("{value}", "VALUE")
        matching = [route for regex, route in compiled if regex.fullmatch(sample)]
        # String prefixes and query suffix interpolation are not whole URLs.
        review = path.endswith("/") or path.endswith("users{value}")
        entries.append({"path": path, "source": source, "routes": matching,
                        "status": "manual_review" if review else "declared" if matching else "missing"})
    return {"scope": "static_path_coverage_only", "frontend_paths": len(entries),
            "declared": sum(e["status"] == "declared" for e in entries),
            "missing": sum(e["status"] == "missing" for e in entries),
            "manual_review": sum(e["status"] == "manual_review" for e in entries),
            "entries": entries}


if __name__ == "__main__":
    print(json.dumps(inventory(), ensure_ascii=False, indent=2))
