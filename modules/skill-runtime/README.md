# Skill Runtime

`exoharness-skill-runtime` 检查指定 Skill revision 在执行镜像中的运行条件，并保存环境证据。它连接 Skill 包、能力登记与 Sandbox，供导入准备和执行准入使用。

## 主要接口

- [service.rs](src/service.rs)：`SkillRuntime::open` 组合服务，`status` 查询环境状态，`prepare` 发起核验，`close` 结束受管检查任务。
- [types.rs](src/types.rs)：`RuntimeStatus` 区分脚本存在、环境就绪及脚本执行就绪；`EnvironmentEvidence` 保存能力版本、包摘要、镜像摘要、运行类型和缺失依赖。
- [metadata.rs](src/metadata.rs)：解析并检查运行依赖声明。
- [probe.rs](src/probe.rs)、[probe.py](src/probe.py)：在沙盒中运行框架提供的依赖检查程序。
- [cache.rs](src/cache.rs)：按版本与镜像身份保存和读取核验证据。

准备阶段使用已有执行镜像，将依赖声明作为数据传给检查程序。检查结果记录可用运行时、版本和缺失项。包 revision 或镜像摘要变化后重新核验；脚本执行仍需独立授权。依赖安装与镜像制作按部署流程处理。

## 测试

[tests/preparation.rs](tests/preparation.rs) 验证环境检查、证据绑定和版本变化。需要独立 PostgreSQL、Docker 及 `EXOHARNESS_PRO_TEST_SANDBOX_IMAGE` 指定的镜像：

```sh
cargo test --locked -p exoharness-skill-runtime
```

相关说明：[能力接入](../../docs/CAPABILITIES.md)、[部署](../../docs/DEPLOYMENT.md)。
