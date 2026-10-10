#!/usr/bin/env python3
"""Start/stop the isolated loopback validation service. Never builds an image."""
from __future__ import annotations
import argparse
import getpass
import hashlib
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
RELEASE = ROOT if (ROOT / "bin" / "exoharness").is_file() else ROOT / "release"
def data_directory(root: Path) -> Path:
    # Source and its release/scripts entrypoints share data; a standalone
    # extracted release owns its own data. Never copy secrets into a package.
    base = root.parent if root.name == "release" and (root.parent / "Cargo.toml").is_file() else root
    default = base / "data" / "local"
    if sys.platform == "linux" and len(base.parts) > 2 and base.parts[1] == "mnt" and len(base.parts[2]) == 1:
        # WSL drive mounts often report 0777 regardless of chmod. Keep private
        # state on the Linux filesystem, not on an insecure Windows mount.
        key = hashlib.sha256(str(base).encode()).hexdigest()[:12]
        default = Path.home() / ".local" / "state" / "exoharness" / key / "local"
    return Path(os.environ.get("EXOHARNESS_LOCAL_DATA_DIR", str(default))).resolve()

DATA = data_directory(ROOT)
LABEL = "com.exoharness.pro.local"
CONTAINER = "exoharness-pro-local-postgres"
VOLUME = "exoharness-pro-local-postgres-data"
IMAGE = "postgres:18.6-alpine3.23"

def atomic_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp-" + secrets.token_hex(8))
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        if os.fstat(descriptor).st_mode & 0o077:
            os.close(descriptor)
            raise RuntimeError("data directory cannot enforce 0600; set EXOHARNESS_LOCAL_DATA_DIR to a private Linux path")
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()

def private_text(path: Path, text: str) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    if os.fstat(descriptor).st_mode & 0o077:
        os.close(descriptor)
        path.unlink()
        raise RuntimeError("data directory cannot enforce 0600; use a private Linux path")
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        handle.write(text)

def docker(*args: str, optional: bool = False) -> str:
    result = subprocess.run(["docker", *args], capture_output=True, text=True)
    if result.returncode and not optional:
        # Never print inspect output/environment, which may contain credentials.
        raise RuntimeError("Docker operation failed: " + " ".join(args[:2]))
    return result.stdout if result.returncode == 0 else ""

def configuration() -> dict:
    path = DATA / "config.json"
    if not path.is_file():
        raise RuntimeError("run prepare-db first; no existing database is reused implicitly")
    value=json.loads(path.read_text(encoding="utf-8"))
    value.setdefault("media_port",9790)
    value.setdefault("media_public_base_url",f"http://127.0.0.1:{value['media_port']}")
    return value

def set_runtime_image(image: str) -> None:
    if not image or image.startswith("-") or len(image)>256 or any(c.isspace() for c in image):
        raise RuntimeError("runtime image identifier is invalid")
    info=json.loads(docker("image","inspect",image))[0]
    image_id=info.get("Id", "")
    if len(image_id)!=71 or not image_id.startswith("sha256:") or any(c not in "0123456789abcdef" for c in image_id[7:]):
        raise RuntimeError("cached runtime image identity is invalid; no image is pulled or built")
    config=configuration();config["sandbox_image"]=image_id
    atomic_json(DATA/"config.json",config)
    print("已登记本机现有沙盒镜像的固定 SHA-256；重启新服务后生效，未构建或拉取镜像。")

def free_port(port: int) -> None:
    with socket.socket() as probe:
        # Match the HTTP listener's reuse policy: closed connections in
        # TIME_WAIT must not be mistaken for another live service.
        probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        probe.bind(("127.0.0.1", port))

def prepare_database() -> None:
    DATA.mkdir(parents=True, exist_ok=True)
    config_path = DATA / "config.json"
    if not config_path.exists():
        atomic_json(config_path, {"management_port":9797,"portal_port":9798,"media_port":9790,"database_port":9799,"database_namespace":"execution"})
    config = configuration()
    password = DATA / "postgres-password"
    if not password.exists():
        private_text(password, secrets.token_urlsafe(36) + "\n")
    credentials = DATA / "credentials.json"
    if not credentials.exists():
        atomic_json(credentials, [{"id":name,"role":role,"token":secrets.token_urlsafe(36)} for name,role in [("admin","admin"),("tester","user"),("tester2","user")]])
    env = DATA / "postgres.env"
    if not env.exists():
        private_text(env, "POSTGRES_USER=exoharness_pro\nPOSTGRES_DB=exoharness_pro\nPOSTGRES_PASSWORD=" + password.read_text().strip() + "\n")
    raw = docker("inspect", CONTAINER, optional=True)
    if raw:
        info = json.loads(raw)[0]
        if info["Config"].get("Labels", {}).get(LABEL) != "1":
            raise RuntimeError("existing container is not owned by this local deployment")
        ports = info["HostConfig"]["PortBindings"].get("5432/tcp", [])
        if ports != [{"HostIp":"127.0.0.1","HostPort":str(config["database_port"])}]:
            raise RuntimeError("existing test database port differs from local configuration")
        if not info["State"]["Running"]:
            docker("start", CONTAINER)
    else:
        free_port(config["database_port"])
        image = docker("image", "inspect", IMAGE, optional=True)
        if not image:
            raise RuntimeError("cached PostgreSQL image unavailable; no automatic pull/build performed")
        mounts = list(json.loads(image)[0]["Config"].get("Volumes", {}))
        if len(mounts) != 1:
            raise RuntimeError("unexpected PostgreSQL image volume layout")
        volume = docker("volume", "inspect", VOLUME, optional=True)
        if volume and json.loads(volume)[0].get("Labels", {}).get(LABEL) != "1":
            raise RuntimeError("existing database volume is not owned by this local deployment")
        if not volume:
            docker("volume", "create", "--label", LABEL + "=1", VOLUME)
        docker("run", "-d", "--name", CONTAINER, "--label", LABEL + "=1", "--restart", "unless-stopped", "--env-file", str(env), "-p", f"127.0.0.1:{config['database_port']}:5432", "--mount", f"type=volume,source={VOLUME},target={mounts[0]}", IMAGE)
    deadline = time.monotonic() + 25
    while time.monotonic() < deadline:
        if docker("exec", CONTAINER, "pg_isready", "-U", "exoharness_pro", "-d", "exoharness_pro", optional=True):
            print("Isolated local PostgreSQL ready; existing ExoHarness database untouched.")
            return
        time.sleep(0.4)
    raise RuntimeError("local PostgreSQL did not become ready")

def process_identity(pid: int) -> tuple[str, str] | None:
    try:
        executable = os.readlink(f"/proc/{pid}/exe").removesuffix(" (deleted)")
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return executable, fields[19]
    except (FileNotFoundError, ProcessLookupError, PermissionError):
        return None

def owned_process() -> int | None:
    path = DATA / "service.json"
    if not path.exists(): return None
    record = json.loads(path.read_text())
    identity = process_identity(record["pid"])
    if identity is None: return None
    if identity[1] != record["start_ticks"]:
        raise RuntimeError("PID identity changed; refusing to signal another process")
    if identity[0] != record["executable"]:
        # A process started through a mount namespace can report /workspace
        # in /proc/exe while its trusted invocation used /mnt/e. Never accept
        # a basename or PID alone: pin start time, original argv, registered
        # release path, manifest and the actual executable content.
        try:
            binary, release_digest = verified_binary()
            process = Path("/proc") / str(record["pid"])
            argument = (process / "cmdline").read_bytes().split(b"\0", 1)[0].decode()
            actual_digest = hashlib.sha256((process / "exe").read_bytes()).hexdigest()
            if record["executable"] != str(binary) or argument != str(binary) or actual_digest not in {record.get("sha256"), release_digest}:
                raise RuntimeError("PID identity changed; refusing to signal another process")
        except (OSError, ValueError, UnicodeError):
            raise RuntimeError("PID identity changed; refusing to signal another process") from None
    return record["pid"]

def stop_service() -> None:
    pid = owned_process()
    if pid is None:
        print("Local validation service is not running.")
        return
    os.kill(pid, signal.SIGINT)
    deadline = time.monotonic() + 15
    while process_identity(pid) is not None and time.monotonic() < deadline:
        # A detached child may briefly be a zombie; /proc/exe disappears then.
        time.sleep(0.1)
    if process_identity(pid) is not None:
        raise RuntimeError("graceful shutdown timed out; no forced kill was performed")
    print("Local validation service stopped; PostgreSQL and old services kept running.")

def health(port: int, expected_service: str = "exoharness") -> dict | None:
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=1) as response:
            payload = json.load(response)
        if not isinstance(payload, dict) or payload.get("service") != expected_service or payload.get("status") != "ok":
            return None
        if expected_service == "exoharness":
            return payload if payload.get("mode") == "infrastructure_validation" else None
        if expected_service == "exoharness-media":
            return payload if payload.get("delivery_only") is True else None
        return None
    except (OSError, ValueError, urllib.error.URLError):
        return None

def verified_binary() -> tuple[Path, str]:
    binary = (RELEASE / "bin" / "exoharness").resolve()
    manifest_path = RELEASE / "build-manifest.json"
    if not binary.is_file() or not manifest_path.is_file():
        raise RuntimeError("compile the release executable before starting")
    manifest = json.loads(manifest_path.read_text())
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if not manifest.get("local_test_service_ready") or not any(item["file"] == "bin/exoharness" and item["sha256"] == digest for item in manifest["binaries"]):
        raise RuntimeError("release executable does not match its build manifest")
    return binary, digest

def start_service() -> None:
    config = configuration()
    if owned_process() is not None:
        raise RuntimeError("local validation service already running; use restart")
    binary, digest = verified_binary()
    for port in (config["management_port"], config["portal_port"], config["media_port"]): free_port(port)
    environment = dict(os.environ)
    environment["EXOHARNESS_CAPABILITY_DIRECTORY"] = str(DATA / "capabilities")
    environment["EXOHARNESS_MEDIA_BIND"] = f"127.0.0.1:{config['media_port']}"
    environment["EXOHARNESS_MEDIA_PUBLIC_BASE_URL"] = config["media_public_base_url"]
    if config.get("sandbox_image"):
        environment["EXOHARNESS_SANDBOX_IMAGE"] = config["sandbox_image"]
    environment.update({"EXOHARNESS_BIND":f"127.0.0.1:{config['management_port']}","EXOHARNESS_PORTAL_BIND":f"127.0.0.1:{config['portal_port']}","EXOHARNESS_DATABASE_URL":f"postgresql://exoharness_pro@127.0.0.1:{config['database_port']}/exoharness_pro","EXOHARNESS_DATABASE_PASSWORD_FILE":str(DATA/"postgres-password"),"EXOHARNESS_DATABASE_NAMESPACE":config["database_namespace"],"EXOHARNESS_LOCAL_CREDENTIALS_FILE":str(DATA/"credentials.json"),"EXOHARNESS_CONFIGURATION_DIR":str(DATA/"configuration")})
    log_path = DATA / "service.log"
    log_fd = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    with os.fdopen(log_fd, "ab", buffering=0) as log:
        child = subprocess.Popen([str(binary)], cwd=ROOT, env=environment, stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True)
    try:
        deadline = time.monotonic() + 30
        while time.monotonic() < deadline:
            if child.poll() is not None: raise RuntimeError("service exited during startup; inspect " + str(log_path))
            if health(config["management_port"]) and health(config["portal_port"]) and health(config["media_port"], "exoharness-media"):
                identity = process_identity(child.pid)
                if identity is None: raise RuntimeError("started process identity unavailable")
                atomic_json(DATA/"service.json", {"pid":child.pid,"executable":identity[0],"start_ticks":identity[1],"sha256":digest})
                print(f"Local service ready: http://127.0.0.1:{config['management_port']} / http://127.0.0.1:{config['portal_port']} / media http://127.0.0.1:{config['media_port']}")
                print("已保留配置与数据；启动服务不会自动提交生成任务。接入与验收进度见项目文档。")
                return
            time.sleep(0.2)
        raise RuntimeError("local service startup timed out")
    except BaseException:
        if child.poll() is None:
            child.send_signal(signal.SIGINT)
            try: child.wait(timeout=12)
            except subprocess.TimeoutExpired: child.kill(); child.wait()
        raise

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare-db","start","stop","restart","status","access","data-path","set-admin-password","set-runtime-image"))
    parser.add_argument("--image",help="already cached OCI image; only with set-runtime-image")
    args = parser.parse_args()
    if args.action=="set-runtime-image":
        if not args.image:parser.error("set-runtime-image requires --image")
        set_runtime_image(args.image);return 0
    if args.image:parser.error("--image is only allowed with set-runtime-image")
    if args.action == "prepare-db": prepare_database()
    elif args.action == "start": start_service()
    elif args.action == "stop": stop_service()
    elif args.action == "restart": stop_service(); start_service()
    elif args.action == "status":
        config=configuration()
        print(json.dumps({"pid":owned_process(),"management":health(config["management_port"]),"portal":health(config["portal_port"]),"media":health(config["media_port"], "exoharness-media")},ensure_ascii=False,indent=2))
    elif args.action == "data-path": print(DATA)
    elif args.action == "set-admin-password":
        path=DATA/"credentials.json"
        records=json.loads(path.read_text())
        admin=next((record for record in records if record["id"]=="admin" and record["role"]=="admin"),None)
        if admin is None: raise RuntimeError("local administrator is not configured; run prepare-db first")
        password=getpass.getpass("New local administrator password (at least 10 characters): ")
        if len(password)<10: raise RuntimeError("local administrator password requires at least 10 characters")
        if any(record is not admin and record["token"]==password for record in records):
            raise RuntimeError("password must differ from other local credentials")
        binary,_=verified_binary()
        config=configuration()
        environment=dict(os.environ)
        environment["EXOHARNESS_DATABASE_URL"]=f"postgresql://exoharness_pro@127.0.0.1:{config['database_port']}/exoharness_pro"
        environment["EXOHARNESS_DATABASE_PASSWORD_FILE"]=str(DATA/"postgres-password")
        # The database owns the live password and its revision. File replacement
        # alone must never masquerade as a successful password rotation.
        subprocess.run([str(binary),"--set-local-admin-password"],input=json.dumps({"password":password}),text=True,env=environment,check=True)
        admin.update({"kind":"local_password","token":password})
        atomic_json(path,records)
        print("Local administrator password updated immediately; previous sessions revoked. Other credentials unchanged.")
    else:
        records=json.loads((DATA/"credentials.json").read_text())
        for record in records: print(f"{record['id']} ({record['role']}): {record['token']}")
    return 0

if __name__ == "__main__":
    try: raise SystemExit(main())
    except (RuntimeError,OSError,ValueError,subprocess.CalledProcessError) as error:
        print(f"Local deployment failed: {error}",file=sys.stderr)
        raise SystemExit(1)
