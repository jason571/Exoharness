# Application

English | [简体中文](README.zh-CN.md)

`exoharness-app` builds the `exoharness` executable and is the service composition/startup entry point. It reads runtime configuration, registers framework extensions, starts management, user and file listeners, and manages background tasks and shutdown.

## Startup structure

- [main.rs](src/main.rs): startup and CLI dispatch.
- [config.rs](src/config.rs): listener, database namespace, data-directory and sandbox-image settings.
- [extensions.rs](src/extensions.rs) and `*_extension.rs`: compose storage, identity, model, capability, sandbox, file and chat services.
- [server.rs](src/server.rs): listeners, task management and graceful shutdown.
- [identity_cli.rs](src/identity_cli.rs): local administrator password recovery.

The three listeners default to IPv4 loopback ports 9797, 9798 and 9790. Ports and public file base URL use `EXOHARNESS_*` settings. `--version` reports the build version, `--check-configuration` checks a specified configuration directory, and `--set-local-admin-password` starts password recovery.

## Build and run

The full build packages the frontend before compiling the executable. From the repository root:

```sh
bash scripts/package-release.sh --offline
bash release/scripts/start-native.sh start
```

Prepare the database, private credentials and model configuration first. Script execution also requires an OCI backend and execution image. See the [README](../../README.md) and [Deployment](../../docs/DEPLOYMENT.md).

## Tests

[tests/cli.rs](tests/cli.rs) checks CLI behavior:

```sh
cargo test --locked -p exoharness-app
```

Service composition, authenticated access and task delivery are covered in the integration checks described in [Testing and acceptance](../../docs/TESTING.md).
