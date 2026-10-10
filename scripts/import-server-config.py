#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
# Commercial licensing: contact flyangqn@gmail.com.

"""Read-only SSH configuration migration. Never restart or write the source.

Imported generations are private, outside the release. Original server bind,
origin, database and filesystem settings are evidence only, never sourced.
"""
from __future__ import annotations
import argparse
import base64
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import secrets
import subprocess
import sys
import time

SPEC = importlib.util.spec_from_file_location("local_service", Path(__file__).with_name("local-service.py"))
local = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(local)


def strict_json(text):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate JSON key in configuration")
            result[key] = value
        return result
    return json.loads(text, object_pairs_hook=pairs)


def remote_reader(root: str) -> str:
    # Code is administrator-owned, not supplied by the chat model. Every
    # source path is fixed or derived from validated catalogs; no writes.
    return '''import base64, hashlib, json, pathlib, re
root = pathlib.Path(ROOT_LITERAL)
files = {}
def add(relative, path, limit):
    if path.is_symlink() or not path.is_file():
        raise RuntimeError("source configuration file unavailable")
    data = path.read_bytes()
    if not data or len(data) > limit:
        raise RuntimeError("source configuration file size invalid")
    files[relative] = base64.b64encode(data).decode("ascii")
add("models.json", root / "data/models.json", 4*1024*1024)
add("agent-profiles.json", root / "data/agent-profiles.json", 4*1024*1024)
add("server.env", root / ".env", 1024*1024)
catalog = json.loads((root / "data/models.json").read_text())
for provider in catalog["providers"]:
    key = "model-provider-" + hashlib.sha256(provider["name"].encode()).hexdigest() + ".secret"
    path = root / "data/secrets/model-providers" / key
    if path.is_file():
        add("secrets/model-providers/" + key, path, 65536)
    elif provider.get("api_key_configured"):
        raise RuntimeError("configured provider credential unavailable")
profiles = json.loads((root / "data/agent-profiles.json").read_text())
for record in profiles["profiles"]:
    for revision in record["revisions"]:
        asset = revision.get("avatar")
        if not asset:
            continue
        name = asset["asset_id"]
        if not re.fullmatch(r"[a-f0-9]{64}\\.(png|jpg|webp)", name):
            raise RuntimeError("invalid source avatar identity")
        add("agent-assets/" + name, root / "data/agent-assets" / name, 2*1024*1024)
print(json.dumps({"version":1,"files":files}))
'''.replace("ROOT_LITERAL", repr(root))


def fetch(args) -> dict:
    target = f"{args.user}@{args.host}"
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", args.user) or not re.fullmatch(r"[A-Za-z0-9_.:-]+", args.host):
        raise ValueError("invalid SSH target")
    result = subprocess.run([
        "ssh", "-i", str(args.identity), "-p", str(args.port),
        "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", target, "python3 -",
    ], input=remote_reader(args.remote_root), text=True, capture_output=True, timeout=60)
    if result.returncode or len(result.stdout) > 48*1024*1024:
        # Neither remote stderr nor captured JSON may enter diagnostics.
        raise RuntimeError("read-only server configuration export failed")
    return strict_json(result.stdout)


def validate_bundle(bundle: dict) -> dict[str, bytes]:
    if bundle.get("version") != 1 or not isinstance(bundle.get("files"), dict):
        raise ValueError("invalid migration bundle")
    files = {}
    for name, encoded in bundle["files"].items():
        path = PurePosixPath(name)
        allowed = name in {"models.json", "agent-profiles.json", "server.env"} or bool(re.fullmatch(
            r"(?:secrets/model-providers/model-provider-[a-f0-9]{64}\.secret|agent-assets/[a-f0-9]{64}\.(?:png|jpg|webp))", name))
        if not allowed or path.is_absolute() or any(part in {".", ".."} for part in path.parts):
            raise ValueError("invalid migration path")
        data = base64.b64decode(encoded, validate=True)
        if not data or len(data) > 4*1024*1024:
            raise ValueError("invalid migration file size")
        files[name] = data
    if not {"models.json", "agent-profiles.json", "server.env"} <= files.keys():
        raise ValueError("required configuration absent")
    catalog = strict_json(files["models.json"])
    profiles = strict_json(files["agent-profiles.json"])
    if not isinstance(catalog.get("providers"), list) or not isinstance(catalog.get("models"), list) or not isinstance(profiles.get("profiles"), list):
        raise ValueError("invalid configuration collections")
    for provider in catalog["providers"]:
        key = "model-provider-" + hashlib.sha256(provider["name"].encode()).hexdigest() + ".secret"
        if provider.get("api_key_configured") and "secrets/model-providers/" + key not in files:
            raise ValueError("configured model credential missing")
    for name, data in files.items():
        if name.startswith("agent-assets/") and hashlib.sha256(data).hexdigest() != Path(name).stem:
            raise ValueError("avatar digest mismatch")
    return files


def secure_directory(path: Path):
    if path.is_symlink():
        raise ValueError("private migration directory cannot be a symbolic link")
    # mkdir(parents=True, mode=0700) applies the mode only to the final
    # directory; intermediate Secret Store directories would be 0755.
    if not path.parent.exists():
        secure_directory(path.parent)
    path.mkdir(mode=0o700, exist_ok=True)
    if path.stat().st_mode & 0o077:
        raise ValueError("private migration directory must enforce mode 0700")


def write_private(path: Path, data: bytes):
    secure_directory(path.parent)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as handle:
        if os.fstat(handle.fileno()).st_mode & 0o077:
            raise ValueError("private migration file must enforce mode 0600")
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())


def install(files: dict[str, bytes], data: Path, source: str, validator: Path) -> dict:
    secure_directory(data / "configuration")
    generation = f"server-{time.strftime('%Y%m%dT%H%M%S')}-{secrets.token_hex(4)}"
    destination = data / "configuration" / generation
    secure_directory(destination)
    for name, content in files.items():
        write_private(destination / name, content)
    # Same Rust validator as startup. Failure retains a private, inactive
    # generation for diagnosis; the current pointer is never changed.
    result = subprocess.run([str(validator), "--check-configuration", str(destination)], capture_output=True, text=True, timeout=30)
    if result.returncode:
        raise RuntimeError("configuration failed Rust validation; active configuration unchanged")
    checked = strict_json(result.stdout)
    report = {"version":1, "generation":generation, "source":source,
              "models":checked["models"], "providers":checked["providers"],
              "assistants":checked["assistants"], "avatars":checked["avatars"],
              "credentials":checked["credentials"], "runtime_ready":False,
              "server_environment_active":False, "local_bind_preserved":True}
    write_private(destination / "migration.json", (json.dumps(report, ensure_ascii=False, indent=2)+"\n").encode())
    # Existing generations are immutable backups. A private pointer selects
    # an entire validated generation, not a partial mix of files.
    local.atomic_json(data / "configuration" / "current.json", {"generation":generation})
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", required=True)
    parser.add_argument("--port", type=int, default=22)
    parser.add_argument("--user", required=True)
    parser.add_argument("--identity", type=Path, required=True)
    parser.add_argument("--remote-root", default="/mnt/data/ExoHarness")
    parser.add_argument("--validator", type=Path, required=True)
    args = parser.parse_args()
    if not 1 <= args.port <= 65535:
        raise ValueError("invalid SSH port")
    report = install(validate_bundle(fetch(args)), local.DATA, f"{args.host}:{args.port}", args.validator.resolve())
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError):
        print("Configuration migration failed; source server was not modified. Active configuration remains unchanged.", file=sys.stderr)
        raise SystemExit(1)
