# Skill Runtime

English | [简体中文](README.zh-CN.md)

`exoharness-skill-runtime` checks whether a specific Skill revision can run in an execution image and records environment evidence. It connects Skill packages, capability registration, and Sandbox for import preparation and execution admission.

## Main interfaces

- [service.rs](src/service.rs): `SkillRuntime::open` composes the service; `status` reads environment state; `prepare` starts verification; and `close` ends a managed check job.
- [types.rs](src/types.rs): `RuntimeStatus` distinguishes script presence, environment readiness, and script-execution readiness. `EnvironmentEvidence` records capability revision, package digest, image digest, runtime type, and missing dependencies.
- [metadata.rs](src/metadata.rs): parses and validates runtime dependency declarations.
- [probe.rs](src/probe.rs), [probe.py](src/probe.py): run the framework-provided dependency checker inside the sandbox.
- [cache.rs](src/cache.rs): stores and reads verification evidence keyed by revision and image identity.

Preparation uses an existing execution image and passes dependency declarations to the checker as data. Results record available runtimes, versions, and missing requirements. A package-revision or image-digest change triggers a new check; running Skill scripts still requires separate authorization. Dependency installation and image creation follow deployment procedures.

## Tests

[tests/preparation.rs](tests/preparation.rs) verifies environment checks, evidence binding, and revision changes. It requires dedicated PostgreSQL, Docker, and the image selected by `EXOHARNESS_PRO_TEST_SANDBOX_IMAGE`:

```sh
cargo test --locked -p exoharness-skill-runtime
```

Related documentation: [Capabilities](../../docs/CAPABILITIES.md), [Deployment](../../docs/DEPLOYMENT.md).
