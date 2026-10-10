# PostgreSQL Storage

[English](README.md) | 简体中文

`exoharness-storage-postgres` 保存执行、能力登记、会话、资料和使用记录，提供带事务边界的持久化接口。连接池使用同步 PostgreSQL 驱动，异步调用方通过 blocking 工作线程访问。

## 接口与文件

- [lib.rs](src/lib.rs)：`PostgresStore` 管理连接、命名空间、迁移和健康检查。
- [sessions.rs](src/sessions.rs)、[runs.rs](src/runs.rs)、[journal.rs](src/journal.rs)：会话、Turn、Run、状态与事件。
- [invocations.rs](src/invocations.rs)、[mcp_admission.rs](src/mcp_admission.rs)：调用领取、回执和 MCP 并发准入。
- [scheduler.rs](src/scheduler.rs)：版本化调度配置、用户/全局 Run 准入和跨 MCP 共享资源池。
- [registry/](src/registry/)：能力版本、访问范围及登记视图。
- [knowledge/](src/knowledge/)、[artifacts.rs](src/artifacts.rs)、[model_observation.rs](src/model_observation.rs)：资料、文件元信息和模型请求观察记录。
- [migrations/](migrations/)：按版本管理的 SQL 迁移。

调用资格领取、状态变更与事件写入使用事务。外部网络请求在事务提交后进行，完成回执随后写回；已提交或结果未知的调用保留相应账本状态。执行 schema 由 namespace 指定。

当前连接使用 `NoTls`，数据库应位于 loopback 或受保护的私网。部署连接与密码文件的配置见 [数据库设计](../../../docs/DATABASE.md)。

## 测试

[tests/](tests/) 使用独立 PostgreSQL 检查持久化、并发准入、登记、标题和模型用量：

```sh
cargo test --locked -p exoharness-storage-postgres
```

执行前设置 `EXOHARNESS_PRO_TEST_DATABASE_URL`。完整准备步骤见 [测试与验收](../../../docs/TESTING.md)。
