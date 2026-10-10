# 工具运行时

[English](README.md) | 简体中文

`exoharness-tools` 将已登记能力转换为 Agent 可发现、可激活的工具，并负责调用前校验与执行派发。MCP、模型能力、Skill 读取和任务沙盒使用统一的目录与授权入口。

## 主要接口

- [catalog.rs](src/catalog.rs)：`Catalog` 保存当前授权范围的能力绑定；`search` 返回紧凑目录，`activate` 加载选定工具的 Schema。
- [service.rs](src/service.rs)：`ToolRuntime` 组合登记、配置、模型、MCP、Skill、沙盒及产物服务。
- [dispatch.rs](src/dispatch.rs)：`dispatch` 派发调用并返回 `DispatchResult`。
- [schema.rs](src/schema.rs)：`validate` 校验原生 JSON，`input_diagnostics` 返回字段诊断。
- [admission.rs](src/admission.rs)、[background.rs](src/background.rs)：处理授权准入、后台任务进度和取消。
- [coding.rs](src/coding.rs)、[workspace.rs](src/workspace.rs)、[publication.rs](src/publication.rs)：连接工作区操作、代码执行和文件发布。

发现目录与实际加载的 Schema 分开管理。调用前复核授权、能力版本、运行状态和参数类型；派发结果记录实际执行情况、文件和工作流元数据。后台轮询使用已取得的任务句柄，执行事实由调用账本保存。

## 测试

[tests/](tests/) 覆盖原生工具 Schema、目录激活、字段诊断、后台任务和产物提取：

```sh
cargo test --locked -p exoharness-tools
```

完整执行链的数据库与 OCI 用例位于 API 模块。相关设计：[能力接入](../../docs/CAPABILITIES.md)。
