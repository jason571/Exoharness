# Agent

English | [简体中文](README.zh-CN.md)

`exoharness-agent` implements the execution loop between models and tools. It requests model output from the current state, handles pending calls, saves receipts and decides whether to complete, cancel or pause. Model providers, HTTP and the database are supplied through adapters.

## Entry points and state

- [engine.rs](src/engine.rs): `run` implements the main loop.
- [ports.rs](src/ports.rs): `Driver` provides model requests, tool calls, state persistence and cancellation checks. `Receipt` distinguishes pre-execution rejection, execution failure, completion and unknown outcome.
- [state.rs](src/state.rs): `EngineState` tracks cursor, usage and counters; `EnginePolicy` configures budgets; `RunOutcome` and `StopReason` describe termination.
- [context.rs](src/context.rs), [projection.rs](src/projection.rs), [catalog_context.rs](src/catalog_context.rs): build history and capability projections for the current request.
- [workflow.rs](src/workflow.rs), [checkpoint_reads.rs](src/checkpoint_reads.rs): handle persistent workflow references and read records.
- [input_correction.rs](src/input_correction.rs): bounded argument repair before execution.

The Driver pins a configuration revision and saves exact arguments and receipts. A submitted call with an unknown outcome enters verification; completed results are reused from receipts. Context pairs calls and results by identity, deduplicates the root instructions for a Skill revision and uses persistent checkpoint references for full work data.

## Tests

[tests/](tests/) uses a Mock Driver to cover the model/tool loop, budgets, cancellation, repeated calls, context projection and workflow continuation:

```sh
cargo test --locked -p exoharness-agent
```

See [Context and Tokens](../../docs/CONTEXT_AND_TOKENS.md) and [Architecture](../../docs/ARCHITECTURE.md).
