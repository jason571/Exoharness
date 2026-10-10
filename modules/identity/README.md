# Identity

`exoharness-identity` 管理管理员与注册用户的身份、登录会话、密码、安装政策、能力分享和助理分配。API 在此基础上实施 Cookie、CSRF 和路由访问控制。

## 主要接口

- [types.rs](src/types.rs)：`AuthStore`、角色、用户账号、会话和能力政策类型。
- [roles.rs](src/roles.rs)：管理角色认证、密码更新和会话校验。
- [users.rs](src/users.rs)、[user_directory.rs](src/user_directory.rs)：注册用户登录、资料与密码维护、管理列表及会话撤销。
- [registration.rs](src/registration.rs)、[mailer.rs](src/mailer.rs)：邮箱验证码、验证注册、通知模板和邮件发送。
- [policy.rs](src/policy.rs)：用户能力政策、私有能力分享与助理分配。
- [attempts.rs](src/attempts.rs)、[schema.rs](src/schema.rs)：登录频率控制和身份数据表初始化。

管理角色与注册用户使用各自的认证流程。密码变更按会话撤销规则处理；验证码有生效与失效状态。安装资格、共享授予和助理分配保存为用户政策，由能力接入与执行层读取。

## 测试

[tests/](tests/) 验证角色与用户认证、密码、会话和安全策略。需要独立 PostgreSQL：

```sh
cargo test --locked -p exoharness-identity
```

相关说明：[安全](../../docs/SECURITY.md)、[接口](../../docs/INTERFACES.md)。
