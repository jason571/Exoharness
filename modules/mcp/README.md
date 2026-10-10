# MCP

English | [简体中文](README.zh-CN.md)

`exoharness-mcp` imports, connects to, discovers tools from, and calls remote MCP services. Runtime transport currently uses the standard SDK's Streamable HTTP. Schemas, progress, and results returned by a service are handled by the higher-level execution pipeline.

## Main interfaces

- [imports/](src/imports/): `ImportRequest` and `parse_import` parse configuration and separate sensitive headers.
- [service.rs](src/service.rs): `Mcp` connects capability registration with Secret Store operations for import and service verification.
- [client/](src/client/): `Connection` handles initialization, tool discovery, calls, and shutdown. `ConnectionOptions` sets startup/discovery timeouts, no-progress timeout, and result-size limits.
- [progress.rs](src/progress.rs): `Progress` carries progress reported by the service.
- [secret_management.rs](src/secret_management.rs): updates credential references used by registered services.

Authentication values are stored in a separate Secret Store; configuration and pages use references and configured status. Tools and the persistent invocation ledger manage execution authorization, stable invocation IDs, and concurrency admission.

Background-task handles and checkpoints use explicit ExoHarness metadata and remain distinct from standard MCP messages. `stdio` configuration can be parsed and registered, but requires a separate managed-launch implementation to run. This module's remote call path uses Streamable HTTP.

## Tests

[tests/protocol.rs](tests/protocol.rs) verifies initialization, schemas, calls, and progress using a local MCP service. Tests that register capabilities require a dedicated PostgreSQL database:

```sh
cargo test --locked -p exoharness-mcp
```

Related documentation: [Capabilities](../../docs/CAPABILITIES.md), [Security](../../docs/SECURITY.md).
