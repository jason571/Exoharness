# Secrets

English | [简体中文](README.zh-CN.md)

`exoharness-secrets` provides local file-based credential storage. Operations such as MCP import save sensitive values in private files; registered configuration stores references, and call adapters resolve the actual values when needed.

## Main interfaces

- [store.rs](src/store.rs): `SecretStore::open` opens the private directory; `put` creates a reference; `get` reads a value; `configured` checks status; `rotate` updates a value; and `delete` removes a reference.
- [value.rs](src/value.rs): `SecretValue` wraps sensitive content; `expose` makes it available to trusted adapters when sending a request.
- [filesystem.rs](src/filesystem.rs): validates directory and file permissions and safe-write conditions.

Credential files use mode `0600` and directories use `0700`. Updates use controlled file writes; configuration and pages expose only references or configured status. This module relies on filesystem permissions to protect stored values; disk encryption and backup protection are deployment responsibilities.

## Tests

[tests/store.rs](tests/store.rs) uses a temporary directory to cover writes, reads, rotation, deletion, and file permissions:

```sh
cargo test --locked -p exoharness-secrets
```

Related documentation: [Security](../../docs/SECURITY.md).
