# PostgreSQL Storage

English | [简体中文](README.zh-CN.md)

`exoharness-storage-postgres` persists execution, capability registration, sessions, knowledge, and usage records through transactional storage interfaces. Its connection pool uses a synchronous PostgreSQL driver; asynchronous callers access it through blocking worker threads.

## Interfaces and files

- [lib.rs](src/lib.rs): `PostgresStore` manages connections, namespaces, migrations, and health checks.
- [sessions.rs](src/sessions.rs), [runs.rs](src/runs.rs), [journal.rs](src/journal.rs): sessions, Turns, Runs, state, and events.
- [invocations.rs](src/invocations.rs), [mcp_admission.rs](src/mcp_admission.rs): invocation claims, receipts, and MCP concurrency admission.
- [scheduler.rs](src/scheduler.rs): versioned scheduler configuration, user/global Run admission, and shared resource pools across MCP services.
- [registry/](src/registry/): capability revisions, access scopes, and registration views.
- [knowledge/](src/knowledge/), [artifacts.rs](src/artifacts.rs), [model_observation.rs](src/model_observation.rs): knowledge, file metadata, and model-request observations.
- [migrations/](migrations/): versioned SQL migrations.

Invocation claims, state changes, and event writes use transactions. External network requests occur after commit, and completion receipts are persisted afterward. Submitted invocations and calls with unknown outcomes retain their ledger state. The execution schema is selected by namespace.

Connections currently use `NoTls`; place the database on loopback or a protected private network. See [Database design](../../../docs/DATABASE.md) for connection and password-file configuration.

## Tests

[tests/](tests/) uses a dedicated PostgreSQL database to check persistence, concurrency admission, registration, titles, and model usage:

```sh
cargo test --locked -p exoharness-storage-postgres
```

Set `EXOHARNESS_PRO_TEST_DATABASE_URL` before running. See [Testing and acceptance](../../../docs/TESTING.md) for complete setup steps.
