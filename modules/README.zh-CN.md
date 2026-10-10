# 代码模块

[English](README.md) | 简体中文

ExoHarness 使用 Rust workspace 组织服务、协议与执行组件。下表列出各 crate 的职责和入口文档；模块边界及依赖原则见 [架构设计](../docs/ARCHITECTURE.md)。

| 模块 | 职责 |
| --- | --- |
| [Contracts](contracts/README.md) | 跨模块协议、数据类型和纯校验 |
| [Core](core/README.md) | 扩展宿主、服务依赖和资源生命周期 |
| [Agent](agent/README.md) | 模型与工具执行循环、上下文及预算 |
| [Tools](tools/README.md) | 能力发现、Schema 激活与执行派发 |
| [Registry](registry/README.md) | 授权能力目录 |
| [Skills](skills/README.md) | Skill 包、版本和资源 |
| [Skill Runtime](skill-runtime/README.md) | 执行环境检查与版本证据 |
| [MCP](mcp/README.md) | MCP 配置、连接、发现和进度 |
| [Model Runtime](model-runtime/README.md) | LLM、TTS、ASR 协议适配 |
| [Sandbox](sandbox/README.md) | OCI 执行及任务工作区 |
| [Coding](coding/README.md) | 代码检索、精确编辑和补丁算法 |
| [Configuration](configuration/README.md) | 模型、助理、头像与配置版本 |
| [Identity](identity/README.md) | 用户、认证、政策与助理分配 |
| [Secrets](secrets/README.md) | 私有凭据存储 |
| [Artifacts](artifacts/README.md) | 文件保存、签名访问和项目发布 |
| [PostgreSQL Storage](storage/postgres/README.md) | 执行、登记、会话与资料持久化 |
| [Database Access](storage/sql-compat/README.md) | PostgreSQL SQL 访问适配 |
| [API](api/README.md) | HTTP、SSE、访问控制和文件响应 |
| [Application](app/README.md) | 服务装配、监听、启动与关闭 |

各模块的 `src/` 保存生产实现，`tests/` 保存测试。模块 README 中的命令均从仓库根目录执行。数据库与 OCI 测试的环境准备见 [测试与验收](../docs/TESTING.md)。
