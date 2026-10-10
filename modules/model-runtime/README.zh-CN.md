# 模型运行时

[English](README.md) | 简体中文

`exoharness-model-runtime` 将模型配置转换为运行请求，提供 LLM 流式调用、TTS 合成、ASR 转写和连接诊断。模型选择及凭据来自 Configuration，供应商协议处理集中在适配器中。

## 主要接口

- [service.rs](src/service.rs)：`ModelRuntime` 提供 `complete`、固定 revision 的完成请求、流式请求、模型测试和关闭；`RuntimePolicy` 管理运行策略。
- [types.rs](src/types.rs)：模型请求、工具选择、完成状态和用量类型。
- [streaming/](src/streaming/)：SSE 与协议分片解析，通过 `TextSink` 输出增量文本。
- [adapters/](src/adapters/)：OpenAI 兼容和 Anthropic 请求适配。
- [synthesis.rs](src/synthesis.rs)、[transcription.rs](src/transcription.rs)：语音合成、音频输出和转写。
- [diagnostics/](src/diagnostics/)：按协议类型生成连接测试报告。

请求绑定配置版本和对应凭据。流式响应同时处理文本、工具参数和完成标记；连接中断、输出截断、HTTP 错误与完整响应分别分类。用量按服务返回值记录，测试报告采用脱敏连接信息。

## 测试

[tests/](tests/) 使用本地 HTTP/SSE 服务覆盖协议解析、流式输出、诊断、语音适配和生命周期：

```sh
cargo test --locked -p exoharness-model-runtime
```

真实模型与语音服务另做连接及成品验收。配置说明见 [模型配置](../../docs/MODEL_CONFIGURATION_DESIGN.md)。
