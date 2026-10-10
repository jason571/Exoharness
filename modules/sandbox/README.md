# Sandbox

`exoharness-sandbox` 提供 Linux OCI 代码执行后端与持久任务工作区。Python、Shell、JavaScript 和 TypeScript 在固定摘要的执行镜像中运行，执行进程与主服务分离。

## 执行与文件接口

- [service.rs](src/service.rs)：`Sandbox::open` 检查后端和镜像，`start` 创建执行任务，`receipt` 读取回执，`cancel_invocation` 取消调用，`stop` 关闭后端。
- [types.rs](src/types.rs)：`RunSpec`、`Runtime`、`Limits`、`Workspace` 和 `Receipt` 定义执行输入与结果。
- [job.rs](src/job.rs)：`JobHandle` 管理任务完成等待与取消。
- [task/](src/task/)：`TaskStore` 提供创建、读取、检索、编辑、补丁及执行租约，`TaskSnapshot` 保存工作区版本。
- [filesystem.rs](src/filesystem.rs)、[output.rs](src/output.rs)：校验文件路径并收集输出。

容器使用只读根文件系统、只读包挂载和可写任务目录，默认禁网，并配置 CPU、内存、PID、运行时间和输出限制。文件操作检查 owner、session 和工作区 revision；执行回执保存退出码、输出、截断和清理状态。

主服务账号需具备 Docker 使用权限，并预先准备执行镜像。主服务采用容器部署时，沙盒调用链的连接方式见 [部署说明](../../docs/DEPLOYMENT.md)。

## 测试

[tests/](tests/) 覆盖 OCI 执行、任务文件、精确编辑、执行租约和取消。设置 `EXOHARNESS_PRO_TEST_SANDBOX_IMAGE` 后运行：

```sh
cargo test --locked -p exoharness-sandbox
```

相关设计：[Coding 工作流](../../docs/CODING_WORKFLOW.md)、[安全](../../docs/SECURITY.md)。
