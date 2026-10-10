# Registry

`exoharness-registry` 提供能力目录的服务入口。它基于 PostgreSQL 登记记录生成授权范围内的能力视图，供 Skill、MCP、Tools 和管理接口使用。

## 接口与数据

[src/lib.rs](src/lib.rs) 导出 `Registry`，实现 Core 的 `Service` 接口，服务标识为 `capability-registry`。

`Registry::views(access, limit)` 接收 `RegistryAccess` 与条数限制，返回 `RegisteredCapabilityView` 列表。能力记录的读写、版本及权限操作由 [PostgreSQL 登记实现](../storage/postgres/src/registry/)承担。

登记记录描述能力身份、版本和授权。环境就绪证据由 Skill Runtime 提供，MCP 连接与发现证据由 MCP 适配器提供，实际执行回执由执行层保存。这些信息各有来源，在目录中组合展示。

## 测试

登记、授权范围和记录读取的集成用例位于 [storage/postgres/tests/registry.rs](../storage/postgres/tests/registry.rs)。设置独立测试数据库后，在仓库根目录执行：

```sh
cargo test --locked -p exoharness-storage-postgres --test registry
```

测试环境见 [测试与验收](../../docs/TESTING.md)；数据结构见 [数据库设计](../../docs/DATABASE.md)。
