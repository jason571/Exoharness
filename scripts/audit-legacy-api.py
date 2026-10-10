#!/usr/bin/env python3
"""Inventory the two legacy routers; source registration is NOT acceptance.

No service calls, downloads, credentials, or production writes. The generated
inventory is a work list. Every endpoint starts pending until actual tests exist.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re

METHODS = r"get|post|put|patch|delete|head|options|trace"


def masked(source):
    """Keep offsets while masking strings/comments for delimiter matching."""
    out = list(source)
    index = 0
    while index < len(source):
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = len(source) if end < 0 else end
        elif source.startswith("/*", index):
            end = source.find("*/", index + 2)
            if end < 0:
                raise ValueError("unterminated Rust comment")
            end += 2
        elif source[index] == '"':
            end = index + 1
            while end < len(source):
                if source[end] == "\\":
                    end += 2
                elif source[end] == '"':
                    end += 1
                    break
                else:
                    end += 1
            else:
                raise ValueError("unterminated Rust string")
        else:
            index += 1
            continue
        for position in range(index, end):
            if out[position] != "\n":
                out[position] = " "
        index = end
    return "".join(out)


def closing(source, start, left, right):
    depth = 0
    for index in range(start, len(source)):
        if source[index] == left:
            depth += 1
        elif source[index] == right:
            depth -= 1
            if depth == 0:
                return index
    raise ValueError("unterminated Rust delimiter")


def inventory(source):
    # Restrict parsing to actual app/portal_app definitions, not test literals
    # or unrelated .route calls. Keep handler fragments as source evidence.
    clean = masked(source)
    entries = []
    for function, surface in [("app", "management"), ("portal_app", "portal")]:
        match = re.search(r"\bpub\s+fn\s+" + function + r"\s*\(", clean)
        if not match:
            raise ValueError("legacy router definition is missing")
        start = clean.index("{", match.end())
        end = closing(clean, start, "{", "}")
        for route in re.finditer(r"\.route\s*\(\s*(\"(?:\\.|[^\"\\])*\")\s*,", source[start:end]):
            position = start + route.start()
            if not clean[position:].startswith(".route"):
                continue
            open_ = clean.index("(", position)
            close = closing(clean, open_, "(", ")")
            path = json.loads(route.group(1))
            routing = clean[start + route.end():close]
            methods = re.findall(r"(?<!\w)(?:axum::routing::)?(" + METHODS + r")\s*\(", routing)
            if not methods:
                raise ValueError("route method could not be determined")
            for method in sorted(set(methods)):
                entries.append({"surface": surface, "method": method.upper(), "path": path,
                    "kind": "api" if path.startswith("/v1/") or path == "/health" else "page_or_asset",
                    "source": "crates/exoharness-api/src/lib.rs", "line": source.count("\n", 0, position) + 1,
                    "acceptance": "pending", "evidence": [],
                    "required_checks": ["success_or_empty", "authentication", "authorization", "native_json_contract"]
                        + (["persist_restart", "invalid_input", "conflict_or_idempotency"] if method not in {"get", "head", "options"} else [])})
    keys = [(item["surface"], item["method"], item["path"]) for item in entries]
    if len(keys) != len(set(keys)):
        raise ValueError("duplicate endpoint in the same legacy surface")
    return sorted(entries, key=lambda item: (item["surface"], item["kind"], item["path"], item["method"]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy-root", type=Path, required=True)
    parser.add_argument("--write", action="store_true", help="write the source-derived work list, never mark tests passed")
    args = parser.parse_args()
    source_path = args.legacy_root / "crates/exoharness-api/src/lib.rs"
    data = source_path.read_bytes()
    entries = inventory(data.decode())
    report = {"schema_version": 1, "source_sha256": hashlib.sha256(data).hexdigest(),
        "meaning": "Legacy registered endpoints; pending is not missing, implemented or passed. See feature acceptance matrix for actual evidence.",
        "endpoints": entries}
    if args.write:
        root = Path(__file__).resolve().parents[1]
        (root / "docs/legacy-api-inventory.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({surface: {"api_method_cases": sum(item["surface"] == surface and item["kind"] == "api" for item in entries),
        "page_asset_method_cases": sum(item["surface"] == surface and item["kind"] != "api" for item in entries)} for surface in ["management", "portal"]}))


if __name__ == "__main__":
    main()
