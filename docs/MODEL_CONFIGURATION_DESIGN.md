# Model configuration and invocation

English | [简体中文](MODEL_CONFIGURATION_DESIGN.zh-CN.md)

Model configuration separates model type, provider protocol, model ID, default, enabled state and private credentials. Model and assistant configuration is saved as immutable generations and refreshed through a current pointer. Each Run pins the revision it uses; later edits do not silently change an existing execution.

## Supported protocols

| Type | Execution protocol | Use and boundary |
| --- | --- | --- |
| LLM | OpenAI-compatible Chat Completions, Anthropic Messages | Streaming text and tool calls; use the matching adapter without injecting parameters based on provider name |
| TTS | `volcengine_tts_sse` | Synthesizes and persists audio; other TTS protocols are not executable merely because they are configured |
| ASR | `openai_transcription` | `/models` and `/audio/transcriptions` compatible endpoint for a separate ASR service; recordings do not enter chat context |
| Image/music/video | Corresponding authorized MCP | Native execution requires a model adapter; current integration is through MCP |

`modules/model-runtime/src/synthesis.rs` currently dispatches Volcengine TTS SSE only. Aliyun HTTP/WebSocket TTS and other protocols require their own execution adapter; a saved UI setting or model name alone does not make them executable. ASR service and weights are prepared by deployment; the framework does not download them automatically.

## Assistants and defaults

Assistants use `default_llm` for the conversation model and separately grant TTS and other model-backed capabilities. Speech models are selected independently; if one is not specified, the default for its type is resolved. Both model and provider must be enabled, and parameters are validated against the relevant protocol.

LLM output limits, reasoning settings and timeouts come from the corresponding configuration/adapter. Disabling reasoning must not rely on a system prompt alone; support from the upstream API must be verified with a typed protocol test. An empty upstream response, output-limit termination, missing completion marker or HTTP error is not a complete answer and must not submit incomplete tool arguments.

## Credentials and tests

Credentials are stored in private files. Public catalogs and APIs expose configuration status, not secret values. Rotation creates a separate credential revision; Trace redacts sensitive values while old Runs retain their existing references. Shared documents, screenshots and examples use placeholders.

`POST /v1/models/{name}/test` performs a typed protocol check. End-to-end acceptance also checks text output, audio playback, a known speech transcription and a real tool call.

ASR uses `/v1/speech-input`, with an 8 MiB audio limit and supported MIME types for WebM, Ogg, WAV, M4A/MP4 and MP3. Voice input and a song attachment for MV creation are separate upload paths with different size and transcription semantics.

Source: `modules/configuration/src/models/`, `modules/model-runtime/src/` and `modules/api/src/configuration/`.
