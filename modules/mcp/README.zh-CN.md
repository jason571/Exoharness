# MCP

[English](README.md) | 简体中文

`exoharness-mcp` 提供远程 MCP 服务的导入、连接、工具发现和调用。当前执行传输使用标准 SDK 的 Streamable HTTP，服务返回的 Schema、进度和结果交由上层执行链处理。

## 主要接口

- [imports/](src/imports/)：`ImportRequest` 与 `parse_import` 解析配置并拆分敏感 Header。
- [service.rs](src/service.rs)：`Mcp` 连接能力登记和 Secret Store，提供导入与服务验证。
- [client/](src/client/)：`Connection` 管理初始化、工具发现、调用和关闭；`ConnectionOptions` 配置启动、发现、无进度超时及结果体积限制。
- [progress.rs](src/progress.rs)：`Progress` 传递服务报告的进度。
- [secret_management.rs](src/secret_management.rs)：更新登记服务使用的凭据引用。

认证值保存在独立 Secret Store，配置与页面使用引用和配置状态。执行授权、稳定 invocation 和并发准入由 Tools 及持久调用账本管理。

后台任务句柄与 checkpoint 使用明确的 ExoHarness 元数据协议，和 MCP 标准消息分别处理。`stdio` 配置可解析登记，其运行需要独立的受管启动实现；本模块的远程调用路径采用 Streamable HTTP。

## 测试

[tests/protocol.rs](tests/protocol.rs) 使用本地 MCP 服务验证初始化、Schema、调用和进度。包含登记操作的用例需要独立 PostgreSQL：

```sh
cargo test --locked -p exoharness-mcp
```

相关说明：[能力接入](../../docs/CAPABILITIES.md)、[安全](../../docs/SECURITY.md)。
