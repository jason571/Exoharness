# Agent

`exoharness-agent` 实现模型与工具之间的执行循环。它根据当前状态发起模型请求、处理待执行调用、保存回执，并决定完成、取消或暂停。模型供应商、HTTP 和数据库通过适配接口接入。

## 入口与状态

- [engine.rs](src/engine.rs)：`run` 执行主循环。
- [ports.rs](src/ports.rs)：`Driver` 提供模型请求、工具调用、状态保存与取消检查；`Receipt` 区分执行前拒绝、执行失败、完成和结果未知。
- [state.rs](src/state.rs)：`EngineState` 保存执行游标、用量和计数，`EnginePolicy` 配置预算，`RunOutcome` 与 `StopReason` 描述停止结果。
- [context.rs](src/context.rs)、[projection.rs](src/projection.rs)、[catalog_context.rs](src/catalog_context.rs)：构建当前请求的历史及能力投影。
- [workflow.rs](src/workflow.rs)、[checkpoint_reads.rs](src/checkpoint_reads.rs)：处理持久工作流引用和读取记录。
- [input_correction.rs](src/input_correction.rs)：处理执行前的有限参数修正。

Driver 使用固定配置版本，并保存精确参数和回执。已提交但结果未知的调用进入核实流程；已完成结果通过回执复用。上下文按调用身份保持配对，同一 Skill revision 的根说明去重，持久 checkpoint 引用完整工作数据。

## 测试

[tests/](tests/) 使用 Mock Driver 覆盖模型/工具循环、预算、取消、重复调用、上下文投影和工作流续接：

```sh
cargo test --locked -p exoharness-agent
```

相关设计：[上下文与 Token](../../docs/CONTEXT_AND_TOKENS.md)、[架构](../../docs/ARCHITECTURE.md)。
