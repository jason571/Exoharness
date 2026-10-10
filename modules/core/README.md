# Core

English | [简体中文](README.zh-CN.md)

`exoharness-core` provides the extension host, dependency resolution, typed services and resource lifecycle. During composition, the application registers extensions; the host validates the dependency graph before starting services.

## Main interfaces

- [graph.rs](src/graph.rs): `ExtensionSpec` declares service providers and dependencies; `activation_order` computes startup order and reports registration conflicts.
- [services.rs](src/services.rs): `Service` defines service identity; `ExtensionContext` publishes services, resolves declared dependencies and registers resources.
- [lifecycle.rs](src/lifecycle.rs): `Extension` defines startup/shutdown behavior; `Host` manages registration, startup, shutdown and lifecycle events.
- [resources.rs](src/resources.rs): `OwnedResource` and `ManagedTask` manage extension resources and background tasks.
- [context.rs](src/context.rs): `JournalEntry`, `ContextItem` and `project` define the base projection from execution records into a model window.

Before startup the host checks duplicate services, missing dependencies, cycles and ambiguous providers. If startup fails, acquired resources are cleaned up. Shutdown runs in reverse dependency order and waits for asynchronous resources to be released. Services are accessed through declared interfaces; business modules own their own state.

## Tests

[tests/](tests/) covers dependency graphs, service scope, startup-failure cleanup, asynchronous shutdown and call/result pairing. Tests use in-memory services and tasks.

```sh
cargo test --locked -p exoharness-core
```

See [Architecture](../../docs/ARCHITECTURE.md).
