# Registry

English | [简体中文](README.zh-CN.md)

`exoharness-registry` provides the service entry point for the capability catalog. It builds authorization-scoped views from PostgreSQL registration records for Skills, MCP, Tools, and management APIs.

## Interfaces and data

[src/lib.rs](src/lib.rs) exports `Registry`, implements Core's `Service` interface, and uses the service ID `capability-registry`.

`Registry::views(access, limit)` accepts `RegistryAccess` and a row limit, returning a list of `RegisteredCapabilityView` records. The [PostgreSQL registry implementation](../storage/postgres/src/registry/) handles record reads and writes, revisions, and permission operations.

Registration records describe capability identity, revision, and grants. Skill Runtime supplies environment-readiness evidence; the MCP adapter supplies connection and discovery evidence; the execution layer stores actual execution receipts. These sources remain distinct and are combined for catalog display.

## Tests

Integration tests for registration, authorization scope, and record reads are in [storage/postgres/tests/registry.rs](../storage/postgres/tests/registry.rs). Configure a dedicated test database, then run from the repository root:

```sh
cargo test --locked -p exoharness-storage-postgres --test registry
```

See [Testing and acceptance](../../docs/TESTING.md) for the test environment and [Database design](../../docs/DATABASE.md) for the data model.
