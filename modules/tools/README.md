# Tools

English | [简体中文](README.zh-CN.md)

`exoharness-tools` exposes registered capabilities as discoverable and activatable Agent tools, then validates and dispatches calls. MCP, model capabilities, Skill reads, and task sandboxes share one catalog and authorization entry point.

## Main interfaces

- [catalog.rs](src/catalog.rs): `Catalog` holds capability bindings in the current authorization scope; `search` returns a compact catalog and `activate` loads selected tool schemas.
- [service.rs](src/service.rs): `ToolRuntime` composes registration, configuration, model, MCP, Skill, sandbox, and artifact services.
- [dispatch.rs](src/dispatch.rs): `dispatch` dispatches calls and returns `DispatchResult`.
- [schema.rs](src/schema.rs): `validate` checks native JSON; `input_diagnostics` returns field-level diagnostics.
- [admission.rs](src/admission.rs), [background.rs](src/background.rs): handle authorization admission, background progress, and cancellation.
- [coding.rs](src/coding.rs), [workspace.rs](src/workspace.rs), [publication.rs](src/publication.rs): connect workspace operations, code execution, and file publication.

Discovery catalogs and loaded schemas are managed separately. Before a call, the runtime rechecks authorization, capability revision, readiness, and argument types. Dispatch results record actual execution, files, and workflow metadata. Background polling uses an acquired task handle, and the invocation ledger stores execution facts.

## Tests

[tests/](tests/) covers native tool schemas, catalog activation, field diagnostics, background tasks, and artifact extraction:

```sh
cargo test --locked -p exoharness-tools
```

End-to-end database and OCI cases are in the API module. See [Capabilities](../../docs/CAPABILITIES.md) for related design.
