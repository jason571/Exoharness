# 模型配置与调用

[English](MODEL_CONFIGURATION_DESIGN.md) | 简体中文

模型配置区分模型类型、Provider 协议、模型 ID、默认值、启用状态和私有认证。模型/助理配置通过不可变 generation 保存和刷新；一次 Run 固定所使用的 revision，后续编辑不偷偷改变已有执行。

## 当前协议

| 类型 | 执行协议 | 用法和边界 |
| --- | --- | --- |
| LLM | OpenAI-compatible Chat Completions、Anthropic Messages | 流式文本/工具调用；选择匹配的 adapter，不按服务商名称注入参数 |
| TTS | `volcengine_tts_sse` | 实际合成并持久保存音频；其他 TTS 协议不能仅通过配置就声称可执行 |
| ASR | `openai_transcription` | 独立 ASR 服务的 models / audio/transcriptions 兼容接口；录音不进入聊天上下文 |
| 图片/音乐/视频 | 对应已授权 MCP | 原生执行需对应 model adapter；当前通过 MCP 接入 |

当前 `modules/model-runtime/src/synthesis.rs` 只派发火山 TTS SSE。阿里云 HTTP/WebSocket TTS 等非此协议必须有对应执行 adapter，不能用页面已保存或某个模型名称冒充支持。独立 ASR 服务/权重由部署准备，框架不自动下载模型。

## 助理和默认值

助理通过 `default_llm` 绑定对话模型，通过模型能力范围授权 TTS 等执行能力。语音模型单独选择，未指定时解析该类型的默认配置。模型及 provider 均需启用，参数按对应协议校验。

LLM 的输出上限、思考配置、超时来自对应配置/adapter。关闭思考不应仅依赖系统提示词；参数是否被上游支持要由类型化测试验证。上游空响应、达到输出上限、缺失完成标记或 HTTP 错误不算完整回答，也不自动提交未完成的工具参数。

## 密钥与测试

密钥保存到私有文件，公共 catalog 和 API 返回配置状态。轮换创建独立凭据版本，Trace 对敏感信息脱敏，旧 Run 保留既有引用。共享文档、截图和示例使用占位值。

`POST /v1/models/{name}/test` 执行类型化协议检查。端到端验收进一步核对文本输出、音频播放、已知语音转写及真实工具调用。

ASR 入口为 `/v1/speech-input`；最大音频 8 MiB，支持 WebM、Ogg、WAV、M4A/MP4 和 MP3 的受支持 MIME。语音输入和用于生成 MV 的歌曲附件是不同上传路径，不共享同一个大小/转写语义。

源码：`modules/configuration/src/models/`、`modules/model-runtime/src/`、`modules/api/src/configuration/`。
