#!/usr/bin/env python3
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
# Commercial licensing: contact flyangqn@gmail.com.

"""Native Cargo/Node build. Docker is neither detected nor invoked."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
PACKAGE_DOCUMENTS = {
    "docs/RELEASE.txt": "README.txt",
    "docs/RELEASE.zh-CN.txt": "README.zh-CN.txt",
    "LICENSE": "LICENSE",
    "COMMERCIAL-LICENSE.md": "COMMERCIAL-LICENSE.md",
}
EVALUATION_SCRIPTS = (
    "create-evaluation-suite.py", "evaluate-offline.py",
    "evaluate-gdpevo.py", "evolve-skills.py",
)


def evaluation_sources() -> list[Path]:
    sources = []
    for module in ("benchmark", "evolution"):
        folder = ROOT / "modules" / module
        sources.append(folder / "pyproject.toml")
        sources.extend(sorted((folder / "src").rglob("*.py")))
    sources.extend(ROOT / "scripts" / name for name in EVALUATION_SCRIPTS)
    sources.append(ROOT / "skills/workflow-evolution/SKILL.md")
    return sources


def package_evaluation(output_dir: Path) -> None:
    # Execution modules and the learning prompt only; never copy experiment
    # workspaces, auth files, traces, caches or generated candidate packages.
    for source in evaluation_sources():
        if not source.is_file() or source.is_symlink():
            raise ValueError(f"Missing or unsafe evaluation source: {source.relative_to(ROOT)}")
        publish_file(source, output_dir / "evaluation" / source.relative_to(ROOT))

def command(argv: list[str], *, capture: bool = False) -> str:
    result = subprocess.run(argv, cwd=ROOT, text=True, check=True,
                            stdout=subprocess.PIPE if capture else None)
    return result.stdout if capture else ""

def build_options(args: argparse.Namespace) -> list[str]:
    result = ["cargo", "build", "--workspace", "--locked", "--message-format=json-render-diagnostics"]
    if args.profile == "release":
        result.append("--release")
    if args.target:
        result.extend(["--target", args.target])
    if args.offline:
        result.append("--offline")
    return result

def binary_artifacts(output: str) -> list[Path]:
    result: list[Path] = []
    for line in output.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (event.get("reason") == "compiler-artifact"
            and "bin" in event.get("target", {}).get("kind", [])
            and not event.get("profile", {}).get("test", False)
            and event.get("executable")):
            result.append(Path(event["executable"]))
    return sorted(set(result))

def checksum(path: Path) -> str:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()

def publish_file(source: Path, destination: Path) -> None:
    # Replace an existing artifact atomically; never truncate a running ELF.
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_name(destination.name + ".new")
    shutil.copy2(source, temporary)
    temporary.replace(destination)

def release_inventory(output_dir: Path) -> list[dict]:
    # Explicit allowlist: never package local data, credentials or caches.
    paths = [file for folder in ("bin", "web", "scripts")
             for file in (output_dir / folder).rglob("*")
             if file.is_file() and "__pycache__" not in file.parts]
    paths.extend(output_dir / name for name in PACKAGE_DOCUMENTS.values())
    paths.extend(output_dir / "evaluation" / source.relative_to(ROOT)
                 for source in evaluation_sources())
    return [{"file": str(path.relative_to(output_dir)), "sha256": checksum(path)}
            for path in sorted(paths) if path.is_file()]

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="release")
    parser.add_argument("--target", help="Rust target already installed on this machine")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--skip-web", action="store_true")
    parser.add_argument("--output", type=Path, default=ROOT / "release")
    args = parser.parse_args()
    if not shutil.which("cargo") or not shutil.which("rustc"):
        parser.error("install Rust/Cargo on the build machine; Docker is not required")
    if not args.skip_web and (not shutil.which("node") or not shutil.which("npm")):
        parser.error("install Node/npm or use --skip-web for a Rust-only build")
    output_dir = args.output.resolve()
    if output_dir in (ROOT.resolve(), ROOT.parent.resolve(), Path("/")):
        parser.error("release output must be a dedicated directory")
    if not args.skip_web:
        if not (ROOT / "web-console" / "node_modules").is_dir():
            install = ["npm", "--prefix", str(ROOT / "web-console"), "ci"]
            if args.offline:
                install.append("--offline")
            command(install)
        command(["npm", "--prefix", str(ROOT / "web-console"), "run", "build"])
    output = command(build_options(args), capture=True)
    executables = binary_artifacts(output)
    # Manifest describes only artifacts produced by this build.
    output_dir.mkdir(parents=True, exist_ok=True)
    binary_dir = output_dir / "bin"
    binaries = []
    for source in executables:
        binary_dir.mkdir(exist_ok=True)
        dest = binary_dir / source.name
        publish_file(source, dest)
        binaries.append({"file": str(dest.relative_to(output_dir)), "sha256": checksum(dest)})
    # --skip-web skips Node compilation, not packaging its existing output.
    # These are the same files embedded by Cargo; retaining stale release/web
    # would make the published manifest disagree with the running binary.
    shutil.copytree(ROOT / "web", output_dir / "web", dirs_exist_ok=True)
    script_dir = output_dir / "scripts"
    script_dir.mkdir(exist_ok=True)
    for name in ("local-service.py", "local-service.sh", "docker-service.py", "start-native.sh", "start-docker.sh", "import-server-config.py"):
        source = ROOT / "scripts" / name
        if source.is_file():
            publish_file(source, script_dir / name)
    for source, name in PACKAGE_DOCUMENTS.items():
        publish_file(ROOT / source, output_dir / name)
    package_evaluation(output_dir)
    verbose = command(["rustc", "-vV"], capture=True)
    host = next(line.split(": ", 1)[1] for line in verbose.splitlines() if line.startswith("host: "))
    target = args.target or host
    libc_name, libc_version = platform.libc_ver()
    manifest = {
        "format_version": 1, "profile": args.profile,
        "compiler": command(["rustc", "--version"], capture=True).strip(),
        "target": target, "build_machine": platform.system(),
        "build_host_libc": {"name": libc_name, "version": libc_version},
        "compatibility_verified": False,
        "compatibility_note": "Native GNU builds require target-host ABI verification; build-host libc is not the binary's minimum required libc.",
        "binaries": binaries, "web_built": not args.skip_web,
        "deployment_ready": False,
        "local_test_service_ready": any(path.name == "exoharness" for path in executables),
        "status": "Native release packaged; target-host ABI and deployment verification required",
        "docker_required_for_build": False,
        "files": release_inventory(output_dir),
    }
    (output_dir / "build-manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Native build completed: {output_dir}")
    if not executables:
        print("Libraries compiled; no service executable exists yet. This is not a deployable release.")
    return 0

if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as error:
        print(f"Build failed (exit {error.returncode}); no successful release manifest was written.", file=sys.stderr)
        raise SystemExit(error.returncode)
