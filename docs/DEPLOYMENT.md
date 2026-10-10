# Build and Deployment

[English](DEPLOYMENT.md) | [简体中文](DEPLOYMENT.zh-CN.md)

ExoHarness can run as a native binary or in a container using a mounted release package. Source builds use the host Rust, Python, and Node.js toolchains. Docker Engine is used for OCI sandboxes and an optionally containerized database. Code and page changes require a new binary release; runtime dependency changes require an image update. For initial setup, see the [English project guide](../README.md) and [Chinese guide](../README.zh-CN.md).

## Build

Builds require Rust/Cargo 1.88+, Python 3.11+, Node/npm, and dependencies matching the lockfiles. Offline builds use cached dependencies only; they do not download toolchains or build images automatically.

```sh
bash scripts/package-release.sh --offline
# Equivalent build entry point
bash scripts/build.sh --offline
# Rust debug build (skip frontend packaging)
bash scripts/build.sh --offline --skip-web --profile dev --output /tmp/exoharness-debug
```

The frontend is built before Rust so that the binary does not embed stale assets. Pages are embedded in the binary: replacing `release/web` alone does not update a running process. Page changes must be packaged into a new binary and the corresponding process restarted.

```text
release/
  bin/exoharness
  web/
  scripts/start-native.sh
  scripts/start-docker.sh
  scripts/local-service.py
  scripts/docker-service.py
  build-manifest.json
  README.txt
```

The release package contains the executable, page assets, launch scripts, and a manifest. The database, configuration generations, user files, and credentials remain in a separate private data directory. Before deploying to another host, verify CPU architecture, shared libraries, and glibc ABI compatibility.

## Native startup

```sh
bash release/scripts/start-native.sh prepare-db
bash release/scripts/start-native.sh start
bash release/scripts/start-native.sh status
bash release/scripts/start-native.sh restart
bash release/scripts/start-native.sh stop
```

`prepare-db` uses a cached PostgreSQL image to create this project's database container and volume. If the image is unavailable, the command stops; it does not pull or build one. An existing PostgreSQL server can be used instead, without running the database in Docker.

`access` displays local access credentials; do not share its output. `set-admin-password` hides input, updates the database, and revokes old sessions. `data-path` shows the active private data location. Set `EXOHARNESS_LOCAL_DATA_DIR` to use a Linux-private directory when a WSL-mounted drive cannot preserve Unix permissions. Secret-file permission checks are not relaxed.

Native logs are written to `service.log` in the active data directory. Scripts verify process ownership using the executable, startup marker, and release digest rather than relying on a PID alone. Restarting the main service should not restart PostgreSQL or business MCP services.

## Configuration

| Variable | Purpose |
| --- | --- |
| `EXOHARNESS_BIND` | Management listener; defaults to `127.0.0.1:9797` |
| `EXOHARNESS_PORTAL_BIND` | User listener; defaults to `127.0.0.1:9798` |
| `EXOHARNESS_MEDIA_BIND` | File listener; defaults to `127.0.0.1:9790` |
| `EXOHARNESS_MEDIA_PUBLIC_BASE_URL` | Public download/preview base URL |
| `EXOHARNESS_DATABASE_URL` | PostgreSQL connection URL |
| `EXOHARNESS_DATABASE_PASSWORD_FILE` | Separate password file |
| `EXOHARNESS_DATABASE_NAMESPACE` | Execution schema; defaults to `execution` |
| `EXOHARNESS_LOCAL_CREDENTIALS_FILE` | Private local access credentials |
| `EXOHARNESS_CONFIGURATION_DIR` | Private model/assistant configuration directory |
| `EXOHARNESS_CAPABILITY_DIRECTORY` | Immutable packages and related capability data |
| `EXOHARNESS_SANDBOX_IMAGE` | Prepared task image, pinned by digest at runtime |
| `EXOHARNESS_MODEL_REQUEST_TIMEOUT_SECONDS` | Model request timeout setting |

The application uses three independent IPv4 loopback listeners. When a production proxy exposes them as 8787 / 8788 / 8790, set the corresponding bind variables. This table and the launch code define supported settings. Public-access requirements are documented in [Security](SECURITY.md).

## Container startup

Build the native release from source, then create the main-service image:

```sh
docker build -f runtime/server/Dockerfile -t exoharness-server:0.1 .
docker run --rm --network none --read-only exoharness-server:0.1 --version
export EXOHARNESS_RUNTIME_IMAGE=exoharness-server:0.1
```

`runtime/server/Dockerfile.dockerignore` limits the build context to the executable and build metadata. The main-service image is separate from the task execution image built from `runtime/sandbox/Dockerfile`. See [sandbox image preparation](../README.md#preparing-the-sandbox-execution-image) for building, registering, and transferring the sandbox image offline.

Stop native listeners before starting the container deployment:

```sh
bash release/scripts/start-native.sh stop
bash release/scripts/start-docker.sh start
bash release/scripts/start-docker.sh status
bash release/scripts/start-docker.sh restart
bash release/scripts/start-docker.sh stop
```

Use a cached compatible image; `EXOHARNESS_RUNTIME_IMAGE` selects it. Before startup, check ABI compatibility with `--version` and no network access. On failure, the script does not select or download another image. The main service mounts the executable and other fixed assets read-only; private mutable directories are mounted separately. By default, the host Docker socket is not exposed to the API container.

The Docker deployment uses host networking and requires Linux Docker Engine or Docker Desktop with host networking enabled. The sandbox execution bridge from the containerized main service is not yet connected; use the native main-service mode when OCI code execution is required. The default container configuration keeps service data in separate mounts.

## Models and external services

Configure and test models in the management console. After importing a Skill or MCP, verify authorization and readiness. Video, music, image, TTS, ASR, and rendering services are independent dependencies and are not included in the main-service binary.

The source-tree script `scripts/start-asr-local.py` reuses the existing ASR project, image, and model weights. It is intended for local development; configure ASR separately for a standalone release package. Verify ASR with an audio sample whose transcript is known.

## Updates and rollback

1. Review active tasks and invocations with unknown outcomes; do not hide external work by restarting.
2. Verify the release manifest and target ABI. Back up the executable, database, and associated private data.
3. Replace release files without overwriting the data directory, then restart the three services.
4. Check health, authentication, configuration, events, representative downloads, and a real task.
5. Before rollback, verify data-format compatibility. Replacing the binary must not resubmit an invocation with an unknown outcome.

Keep execution logs and unknown-outcome invocation records during updates, then follow the existing recovery policy. See [Testing](TESTING.md) for test commands.
