# Configuration

[English](README.md) | 简体中文

`exoharness-configuration` 管理模型、供应商、助理、头像和配置版本。管理接口调用该模块写入配置，执行层绑定选定版本，用户端读取经过筛选的公开视图。

## 主要接口

- [models/](src/models/)：`Models` 提供目录读取、配置保存、启停和默认选择；`bind_enabled` 返回同时绑定模型、供应商和凭据的 `ModelBinding`。
- [profiles/](src/profiles/)：`Assistants` 管理助理创建、更新、启停、默认选择、能力范围和头像；`AssignmentLease` 提供分配使用的配置视图。
- [files.rs](src/files.rs)：管理配置 generation 和私有文件。
- [validation.rs](src/validation.rs)：检查配置结构。
- [lib.rs](src/lib.rs)：`check_directory` 核验配置目录。

写入操作校验预期 revision，配置版本与凭据在绑定时一起取得。管理视图和公开助理信息分别投影；模型执行取得的凭据仅供请求适配器使用。助理分配及删除引用检查由 Identity 与调用方协同完成。

## 测试

[tests/](tests/) 覆盖目录加载、版本冲突、启停、默认值、模型绑定和助理写入，使用独立临时目录：

```sh
cargo test --locked -p exoharness-configuration
```

相关说明：[模型配置](../../docs/MODEL_CONFIGURATION_DESIGN.md)、[助理配置](../../docs/ASSISTANT_CONFIGURATION_DESIGN.md)。
