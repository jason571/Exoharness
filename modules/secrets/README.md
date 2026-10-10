# Secrets

`exoharness-secrets` 提供本机文件凭据存储。MCP 导入等操作将敏感值保存为私有文件，登记配置使用引用，调用适配器按引用取得实际值。

## 主要接口

- [store.rs](src/store.rs)：`SecretStore::open` 打开私有目录，`put` 创建引用，`get` 读取值，`configured` 查询配置状态，`rotate` 更新值，`delete` 删除引用。
- [value.rs](src/value.rs)：`SecretValue` 封装敏感内容，`expose` 供受信任适配器在发送请求时使用。
- [filesystem.rs](src/filesystem.rs)：校验目录、文件权限及安全写入条件。

凭据文件使用 `0600`，目录使用 `0700`。更新采用受控文件写入，配置与页面仅公开引用或配置状态。本模块采用文件系统权限保护存储内容；磁盘加密与备份保护由部署环境承担。

## 测试

[tests/store.rs](tests/store.rs) 使用临时目录覆盖写入、读取、轮换、删除及文件权限：

```sh
cargo test --locked -p exoharness-secrets
```

相关说明：[安全](../../docs/SECURITY.md)。
