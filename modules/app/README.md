# Application

`exoharness-app` 生成 `exoharness` 执行文件，是服务装配和启动入口。它读取运行配置，注册框架扩展，启动管理端、用户端和文件监听，并负责后台任务与服务关闭。

## 启动结构

- [main.rs](src/main.rs)：启动与 CLI 分派。
- [config.rs](src/config.rs)：读取监听、数据库命名空间、数据目录及沙盒镜像配置。
- [extensions.rs](src/extensions.rs)及各 `*_extension.rs`：装配存储、身份、模型、能力、沙盒、文件和聊天服务。
- [server.rs](src/server.rs)：启动监听、管理任务并执行优雅关闭。
- [identity_cli.rs](src/identity_cli.rs)：本地管理员密码恢复。

三个监听默认使用 IPv4 loopback 的 9797、9798、9790，端口和公开文件基址通过 `EXOHARNESS_*` 配置。`--version` 读取构建版本，`--check-configuration` 核验指定配置目录，`--set-local-admin-password` 进入密码恢复流程。

## 构建与运行

完整构建先打包前端，再编译执行文件。在仓库根目录执行：

```sh
bash scripts/package-release.sh --offline
bash release/scripts/start-native.sh start
```

运行前准备数据库、私有凭据和模型配置；启用脚本执行时还需 OCI 后端及执行镜像。具体步骤见 [README](../../README.md)和[部署说明](../../docs/DEPLOYMENT.md)。

## 测试

[tests/cli.rs](tests/cli.rs) 检查命令行行为：

```sh
cargo test --locked -p exoharness-app
```

服务装配、认证访问和任务交付按 [测试与验收](../../docs/TESTING.md)进行集成检查。
