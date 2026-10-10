#!/usr/bin/env python3
"""Run the SAME published executable in an existing Linux runtime image.

No build, pull, dependency installation or arbitrary host execution occurs.
Host networking is required for the loopback-only local validation service.
"""
from __future__ import annotations
import argparse
import importlib.util
import json
import os
from pathlib import Path
import sys
import time

SPEC = importlib.util.spec_from_file_location("local_service", Path(__file__).with_name("local-service.py"))
local = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(local)
CONTAINER = "exoharness-pro-local-service"
LABEL = "com.exoharness.pro.local.service"
IMAGE = os.environ.get("EXOHARNESS_RUNTIME_IMAGE", "ubuntu:24.04")

def owned_container() -> dict | None:
    raw = local.docker("inspect", CONTAINER, optional=True)
    if not raw:
        return None
    value = json.loads(raw)[0]
    if value["Config"].get("Labels", {}).get(LABEL) != "1":
        raise RuntimeError("container name is owned by another deployment; refusing to change it")
    return value

def mount(source: Path, target: str) -> str:
    # Docker --mount's comma syntax is not a shell; reject ambiguous paths.
    if "," in str(source):
        raise RuntimeError("Docker mount source cannot contain a comma")
    return f"type=bind,source={source},target={target},readonly"

def runtime_arguments(binary: Path, config: dict) -> list[str]:
    trust = []
    bundle = Path(os.environ.get("EXOHARNESS_CA_BUNDLE", "/etc/ssl/certs/ca-certificates.crt"))
    if bundle.is_file():
        trust = ["--mount", mount(bundle.resolve(), "/etc/ssl/certs/ca-certificates.crt"),
                 "--env", "SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt"]
    imported = []
    if (local.DATA / "configuration" / "current.json").is_file():
        imported = ["--mount", mount(local.DATA / "configuration", "/run/exoharness/configuration").removesuffix(",readonly"),
                    "--env", "EXOHARNESS_CONFIGURATION_DIR=/run/exoharness/configuration"]
    capability_root = local.DATA / "capabilities"
    return [
        "run", "-d", "--name", CONTAINER, "--label", LABEL + "=1",
        "--network", "host", "--read-only", "--cap-drop", "ALL",
        "--security-opt", "no-new-privileges", "--user", f"{os.getuid()}:{os.getgid()}",
        "--tmpfs", "/tmp:rw,nosuid,nodev,noexec,size=64m",
        "--mount", mount(binary, "/opt/exoharness/bin/exoharness"),
        "--mount", mount(local.RELEASE / "web", "/opt/exoharness/web"),
        "--mount", mount(local.DATA / "credentials.json", "/run/exoharness/credentials.json"),
        "--mount", mount(local.DATA / "postgres-password", "/run/exoharness/postgres-password"),
        "--mount", mount(capability_root, "/run/exoharness/capabilities").removesuffix(",readonly"),
        "--env", "EXOHARNESS_CAPABILITY_DIRECTORY=/run/exoharness/capabilities",
        "--env", f"EXOHARNESS_BIND=127.0.0.1:{config['management_port']}",
        "--env", f"EXOHARNESS_PORTAL_BIND=127.0.0.1:{config['portal_port']}",
        "--env", f"EXOHARNESS_MEDIA_BIND=127.0.0.1:{config.get('media_port',9790)}",
        "--env", f"EXOHARNESS_MEDIA_PUBLIC_BASE_URL={config.get('media_public_base_url','http://127.0.0.1:9790')}",
        "--env", f"EXOHARNESS_DATABASE_URL=postgresql://exoharness_pro@127.0.0.1:{config['database_port']}/exoharness_pro",
        "--env", "EXOHARNESS_DATABASE_PASSWORD_FILE=/run/exoharness/postgres-password",
        "--env", "EXOHARNESS_LOCAL_CREDENTIALS_FILE=/run/exoharness/credentials.json",
        "--env", f"EXOHARNESS_DATABASE_NAMESPACE={config['database_namespace']}",
        *imported, *trust,
        "--entrypoint", "/opt/exoharness/bin/exoharness", IMAGE,
    ]

def preflight(binary: Path) -> None:
    if not local.docker("image", "inspect", IMAGE, optional=True):
        raise RuntimeError("runtime image is not installed; load a compatible image first (no automatic pull/build)")
    output = local.docker("run", "--rm", "--network", "none", "--read-only", "--cap-drop", "ALL",
                          "--security-opt", "no-new-privileges", "--user", f"{os.getuid()}:{os.getgid()}",
                          "--mount", mount(binary, "/opt/exoharness/bin/exoharness"),
                          "--entrypoint", "/opt/exoharness/bin/exoharness", IMAGE, "--version")
    if not output.startswith("ExoHarness "):
        raise RuntimeError("runtime image failed executable ABI preflight; no service was started")

def start() -> None:
    config = local.configuration()
    if local.owned_process() is not None:
        raise RuntimeError("native service is running; stop it before switching to Docker")
    existing = owned_container()
    if existing and existing["State"]["Running"]:
        raise RuntimeError("Docker service already running; use restart")
    binary, digest = local.verified_binary()
    capability_root = local.DATA / "capabilities"
    capability_root.mkdir(mode=0o700, exist_ok=True)
    if capability_root.is_symlink() or capability_root.stat().st_mode & 0o077:
        raise RuntimeError("capability content directory must be private and not a symlink")
    if not (local.RELEASE / "web").is_dir():
        raise RuntimeError("full release pages are missing; package without --skip-web")
    for port in (config["management_port"], config["portal_port"], config["media_port"]):
        local.free_port(port)
    preflight(binary)
    if existing:
        local.docker("rm", CONTAINER)  # already verified own stopped container; volumes untouched
    local.docker(*runtime_arguments(binary, config))
    try:
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            value = owned_container()
            if not value or not value["State"]["Running"]:
                raise RuntimeError("Docker service exited; inspect its container logs")
            if local.health(config["management_port"]) and local.health(config["portal_port"]) and local.health(config["media_port"], "exoharness-media"):
                local.atomic_json(local.DATA / "docker-service.json", {"container": CONTAINER, "sha256": digest, "image": IMAGE})
                print(f"ExoHarness Docker validation service ready on {config['management_port']}/{config['portal_port']}/{config['media_port']}; no image rebuilt.")
                return
            time.sleep(0.3)
        raise RuntimeError("Docker service startup timed out")
    except BaseException:
        if owned_container():
            local.docker("stop", "--time", "12", CONTAINER)
        raise

def stop() -> None:
    value = owned_container()
    if value and value["State"]["Running"]:
        local.docker("stop", "--time", "15", CONTAINER)
        print("Own Docker service stopped; database and old services untouched.")
    else:
        print("Docker service is not running.")

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("start", "stop", "restart", "status"), nargs="?", default="start")
    action = parser.parse_args().action
    if action == "start": start()
    elif action == "stop": stop()
    elif action == "restart": stop(); start()
    else:
        value = owned_container()
        config = local.configuration()
        print(json.dumps({"container": CONTAINER, "running": bool(value and value["State"]["Running"]),
                          "management": local.health(config["management_port"]),
                          "portal": local.health(config["portal_port"]),"media":local.health(config["media_port"], "exoharness-media")}, ensure_ascii=False, indent=2))

if __name__ == "__main__":
    try: main()
    except (RuntimeError, OSError, ValueError) as error:
        print(f"Docker deployment failed: {error}", file=sys.stderr)
        raise SystemExit(1)
