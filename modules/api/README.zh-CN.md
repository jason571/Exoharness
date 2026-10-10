# API

[English](README.md) | 简体中文

`exoharness-api` 将框架服务适配为 HTTP、SSE 和文件接口。管理端负责配置与审计，用户端负责个人会话和任务，文件端负责产物访问；身份与数据所有权在服务端检查。

## 路由组织

- [routes.rs](src/routes.rs)、[application.rs](src/application.rs)：组合路由及应用服务。
- [auth.rs](src/auth.rs)、[identity/](src/identity/)：认证、CSRF、注册用户、管理角色与用户政策。
- [chat/](src/chat/)、[conversations/](src/conversations/)、[events.rs](src/events.rs)：Turn 提交、执行控制、会话管理和持久事件流。
- [registry/](src/registry/)、[skills/](src/skills/)、[mcp/](src/mcp/)：能力目录、导入、验证、环境准备及凭据更新。
- [configuration/](src/configuration/)、[knowledge/](src/knowledge/)：模型、助理、头像、资料和记忆接口。
- [trace.rs](src/trace.rs)、[management.rs](src/management.rs)：追踪和运行总览。
- [media/](src/media/)：下载、Range、媒体元信息和项目预览。

公共组合入口由 [lib.rs](src/lib.rs) 导出，各处理器依赖 `ApiState` 中的类型化服务。同步数据库操作经 blocking 边界访问，任务工作线程管理请求结束后的执行。

事件游标支持重放和前端观察恢复。HTTP Run Resume 接口当前返回 `resume_unavailable`；checkpoint 保存在执行日志中，具体恢复支持范围见[功能状态](../../docs/STATUS.md)。

## 测试

[tests/](tests/) 覆盖路由、身份、配置、任务、文件和协议适配。集成用例需要独立 PostgreSQL；运行与文档解析用例还需要准备沙盒镜像：

```sh
cargo test --locked -p exoharness-api
```

接口与恢复支持范围分别见 [接口说明](../../docs/INTERFACES.md)和[功能状态](../../docs/STATUS.md)。
