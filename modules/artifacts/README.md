# Artifacts

English | [简体中文](README.zh-CN.md)

`exoharness-artifacts` stores execution artifacts and creates shared access URLs. File records include owner, source Run, filename, media type, size and SHA-256; binary contents reside in a separate file directory.

## Main interfaces

- [service.rs](src/service.rs): `Artifacts` publishes files, creates signed URLs, authorizes reads and reports health.
- [files.rs](src/files.rs), [types.rs](src/types.rs): file handling, `PublishedArtifact` and error types.
- [signing.rs](src/signing.rs): signature creation, expiry and verification.
- [remote.rs](src/remote.rs): `RemoteFile`, `same_service_origin` and remote artifact import with source/download validation.
- [project.rs](src/project.rs): publish multi-file static projects and signed previews.

Sandbox output, model audio and MCP artifacts are passed to this service by execution adapters. Download URLs use the configured public base. Static projects preserve resource directories and relative paths; the API file endpoint handles headers, Range and browser previews.

## Tests

[tests/](tests/) covers file storage, source validation, remote import, signatures and project resources. Tests require a dedicated PostgreSQL database; remote downloads use a local test server.

```sh
cargo test --locked -p exoharness-artifacts
```

See [Artifact delivery](../../docs/MEDIA_DELIVERY_DESIGN.md) and [Database](../../docs/DATABASE.md).
