# Database Access

`exoharness-database` 是 PostgreSQL 的 SQL 访问适配层，向 Identity 等模块提供连接、参数、行映射和事务接口。

## 主要接口

- [pool.rs](src/pool.rs)：`Database::connect` 和 `from_env` 建立连接池，`connection` 获取连接，namespace 定义查询作用域。
- [connection.rs](src/connection.rs)：`Connection` 和 `Executor` 提供执行、单行查询、语句准备及事务入口。
- [params.rs](src/params.rs)：`Params` 映射输入参数，并处理调用方使用的占位符形式。
- [rows.rs](src/rows.rs)：`Row`、`Statement`、`MappedRows` 和可选结果转换。
- [transaction.rs](src/transaction.rs)：`Transaction` 管理事务执行、提交与释放。

连接从独立配置和密码文件取得访问凭据。该层负责 SQL 接口适配和连接生命周期，业务表结构由 Identity 或 PostgreSQL Storage 管理。同步操作由上层安排到 blocking 工作线程。

## 测试

[tests/unit/parameters.rs](tests/unit/parameters.rs) 验证参数与占位符转换：

```sh
cargo test --locked -p exoharness-database --lib
```

实际事务与身份表调用通过 [Identity 测试](../../identity/tests/)覆盖。数据库配置见 [数据库设计](../../../docs/DATABASE.md)。
