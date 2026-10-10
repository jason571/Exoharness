# Contracts

`exoharness-contracts` 定义各模块共享的执行协议和数据类型。该模块只包含数据结构与校验逻辑，适用于模型适配器、工具派发器和持久化层。

## 协议组成

- [model.rs](src/model.rs)：`ChatRequest`、`Message`、`ToolCall`、`Completion` 和 `Usage`，统一模型输入、工具调用与用量记录。
- [execution.rs](src/execution.rs)：Session、Run、执行快照、事件及状态转换规则。
- [invocation.rs](src/invocation.rs)：外部调用状态及提交资格。
- [capability/](src/capability/)：能力登记、版本、权限和公开视图。
- [strict_json.rs](src/strict_json.rs)、[json_syntax_repair.rs](src/json_syntax_repair.rs)：严格 JSON 解析及带修改记录的有限语法修正。
- [retry.rs](src/retry.rs)：按工具名称与参数判断连续重复调用。

语法修正返回修改证据，实时 Schema 校验和执行授权由 Tools 完成。调用状态与 Run 状态分别记录，便于区分任务终态和外部操作结果。

## 测试

[tests/](tests/) 覆盖状态转换、重复调用策略、JSON 解析与修正。以下命令在仓库根目录执行：

```sh
cargo test --locked -p exoharness-contracts
```

相关设计：[架构](../../docs/ARCHITECTURE.md)、[上下文与 Token](../../docs/CONTEXT_AND_TOKENS.md)。
