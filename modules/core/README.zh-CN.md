# Core

[English](README.md) | 简体中文

`exoharness-core` 提供扩展宿主、依赖解析、类型化服务和资源生命周期。应用在装配阶段注册扩展，宿主检查依赖图后按顺序启动服务。

## 核心接口

- [graph.rs](src/graph.rs)：`ExtensionSpec` 声明服务提供项与依赖，`activation_order` 计算启动顺序并报告注册冲突。
- [services.rs](src/services.rs)：`Service` 定义服务标识；`ExtensionContext` 用于发布服务、解析声明的依赖和登记资源。
- [lifecycle.rs](src/lifecycle.rs)：`Extension` 定义启动和关闭行为，`Host` 管理注册、启动、关闭及生命周期事件。
- [resources.rs](src/resources.rs)：`OwnedResource` 和 `ManagedTask` 管理扩展持有的资源与后台任务。
- [context.rs](src/context.rs)：`JournalEntry`、`ContextItem` 和 `project` 定义执行记录到模型窗口的基础投影。

启动前检查重复服务、缺失依赖、循环和多提供者歧义。启动失败时清理已取得的资源；关闭按逆依赖顺序进行，并等待异步资源释放。服务通过声明的接口访问，业务模块各自管理状态。

## 测试

[tests/](tests/) 覆盖依赖图、服务作用域、启动失败清理、异步关闭和调用结果配对。测试使用内存服务与任务：

```sh
cargo test --locked -p exoharness-core
```

相关设计：[架构](../../docs/ARCHITECTURE.md)。
