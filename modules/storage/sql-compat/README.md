# Database Access

English | [简体中文](README.zh-CN.md)

`exoharness-database` is a PostgreSQL SQL-access adapter that provides connection, parameter, row-mapping, and transaction interfaces to modules such as Identity.

## Main interfaces

- [pool.rs](src/pool.rs): `Database::connect` and `from_env` create the connection pool; `connection` obtains a connection; and namespace defines query scope.
- [connection.rs](src/connection.rs): `Connection` and `Executor` provide execution, single-row queries, statement preparation, and transaction entry points.
- [params.rs](src/params.rs): `Params` maps input parameters and handles the placeholder syntax used by callers.
- [rows.rs](src/rows.rs): `Row`, `Statement`, `MappedRows`, and optional result mapping.
- [transaction.rs](src/transaction.rs): `Transaction` manages transaction execution, commit, and release.

Connections obtain credentials from separate configuration and password files. This layer adapts the SQL interface and manages connection lifecycle; business table schemas are owned by Identity or PostgreSQL Storage. Higher layers schedule synchronous operations on blocking workers.

## Tests

[tests/unit/parameters.rs](tests/unit/parameters.rs) verifies parameters and placeholder conversion:

```sh
cargo test --locked -p exoharness-database --lib
```

Actual transactions and identity-table operations are covered by [Identity tests](../../identity/tests/). See [Database design](../../../docs/DATABASE.md) for database configuration.
