# Identity

English | [简体中文](README.zh-CN.md)

`exoharness-identity` manages administrator and registered-user identities, login sessions, passwords, installation policy, capability sharing and assistant assignment. The API adds cookie, CSRF and route-access controls.

## Main interfaces

- [types.rs](src/types.rs): `AuthStore`, roles, user accounts, sessions and capability-policy types.
- [roles.rs](src/roles.rs): administrator-role authentication, password updates and session checks.
- [users.rs](src/users.rs), [user_directory.rs](src/user_directory.rs): registered-user login, profile/password management, admin directory and session revocation.
- [registration.rs](src/registration.rs), [mailer.rs](src/mailer.rs): email codes, verified registration, notification templates and mail delivery.
- [policy.rs](src/policy.rs): user capability policy, private-capability sharing and assistant assignment.
- [attempts.rs](src/attempts.rs), [schema.rs](src/schema.rs): login rate limits and identity-schema initialization.

Administrator roles and registered users use separate authentication flows. Password changes follow session-revocation policy; verification codes have explicit active/expired states. Installation eligibility, sharing grants and assistant assignments are stored as user policy for capability management and execution to read.

## Tests

[tests/](tests/) verifies administrator/user authentication, passwords, sessions and security policy. It requires a dedicated PostgreSQL database.

```sh
cargo test --locked -p exoharness-identity
```

See [Security](../../docs/SECURITY.md) and [API reference](../../docs/INTERFACES.md).
