# API

English | [简体中文](README.zh-CN.md)

`exoharness-api` adapts framework services to HTTP, SSE and file interfaces. Management handles configuration and audit; the user portal handles personal sessions and tasks; the file endpoint serves artifacts. Identity and data ownership are checked server-side.

## Route organization

- [routes.rs](src/routes.rs), [application.rs](src/application.rs): route composition and application services.
- [auth.rs](src/auth.rs), [identity/](src/identity/): authentication, CSRF, registration, admin roles and user policy.
- [chat/](src/chat/), [conversations/](src/conversations/), [events.rs](src/events.rs): Turn submission, execution control, session management and durable event stream.
- [registry/](src/registry/), [skills/](src/skills/), [mcp/](src/mcp/): capability catalog, imports, validation, runtime preparation and credential updates.
- [configuration/](src/configuration/), [knowledge/](src/knowledge/): model, assistant, avatar, knowledge and memory APIs.
- [trace.rs](src/trace.rs), [management.rs](src/management.rs): Trace and execution overview.
- [media/](src/media/): downloads, Range, media metadata and project preview.

The public composition entry point is [lib.rs](src/lib.rs); handlers depend on typed services in `ApiState`. Synchronous database operations cross a blocking boundary. Task workers manage execution after the request ends.

Event cursors support replay and frontend recovery observation. The HTTP Run Resume route currently returns `resume_unavailable`; checkpoints remain in the execution journal. See [Feature status](../../docs/STATUS.md) for the recovery scope.

## Tests

[tests/](tests/) covers routes, identity, configuration, tasks, files and protocol adapters. Integration tests require a dedicated PostgreSQL database; execution and document-parsing cases also need a prepared sandbox image.

```sh
cargo test --locked -p exoharness-api
```

See [API reference](../../docs/INTERFACES.md) and [Feature status](../../docs/STATUS.md).
