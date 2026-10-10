# Contracts

English | [简体中文](README.zh-CN.md)

`exoharness-contracts` defines shared execution protocols and data types. It contains data structures and validation logic only, for use by model adapters, tool dispatchers and persistence layers.

## Protocols

- [model.rs](src/model.rs): `ChatRequest`, `Message`, `ToolCall`, `Completion` and `Usage` unify model inputs, tool calls and usage records.
- [execution.rs](src/execution.rs): Session, Run, execution snapshots, events and state-transition rules.
- [invocation.rs](src/invocation.rs): external-call states and submission eligibility.
- [capability/](src/capability/): capability registration, revisions, permissions and public views.
- [strict_json.rs](src/strict_json.rs), [json_syntax_repair.rs](src/json_syntax_repair.rs): strict JSON parsing and bounded syntax repair with a modification record.
- [retry.rs](src/retry.rs): detects consecutive repeated calls by tool name and arguments.

Syntax repair returns evidence of its changes. Tools handles live-schema validation and execution authorization. Invocation state and Run state are recorded separately so terminal task state is distinguishable from the result of an external operation.

## Tests

[tests/](tests/) covers state transitions, repeated-call policy, JSON parsing and repair. Run from the repository root:

```sh
cargo test --locked -p exoharness-contracts
```

See [Architecture](../../docs/ARCHITECTURE.md) and [Context and Tokens](../../docs/CONTEXT_AND_TOKENS.md).
