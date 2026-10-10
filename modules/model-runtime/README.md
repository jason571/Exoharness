# Model Runtime

English | [简体中文](README.zh-CN.md)

`exoharness-model-runtime` turns model configuration into runtime requests and provides streaming LLM calls, TTS synthesis, ASR transcription, and connection diagnostics. Model selection and credentials come from Configuration; provider-specific protocols are handled by adapters.

## Main interfaces

- [service.rs](src/service.rs): `ModelRuntime` provides `complete`, revision-pinned completion, streaming requests, model tests, and shutdown; `RuntimePolicy` manages runtime policy.
- [types.rs](src/types.rs): model requests, tool selection, completion states, and usage types.
- [streaming/](src/streaming/): SSE and protocol-chunk parsing with incremental text output through `TextSink`.
- [adapters/](src/adapters/): OpenAI-compatible and Anthropic request adapters.
- [synthesis.rs](src/synthesis.rs), [transcription.rs](src/transcription.rs): speech synthesis, audio output, and transcription.
- [diagnostics/](src/diagnostics/): protocol-specific connection-test reports.

Requests are pinned to a configuration revision and its credentials. Streaming handles text, tool arguments, and completion markers; interrupted connections, truncated output, HTTP errors, and complete responses are classified separately. Usage is recorded from provider responses, and diagnostic reports redact connection details.

## Tests

[tests/](tests/) uses local HTTP/SSE services to cover protocol parsing, streaming output, diagnostics, speech adapters, and lifecycle:

```sh
cargo test --locked -p exoharness-model-runtime
```

Real model and speech services require separate connectivity and deliverable acceptance. See [Model configuration](../../docs/MODEL_CONFIGURATION_DESIGN.md).
