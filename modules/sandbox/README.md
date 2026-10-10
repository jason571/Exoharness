# Sandbox

English | [简体中文](README.zh-CN.md)

`exoharness-sandbox` provides a Linux OCI code-execution backend and persistent task workspaces. Python, Shell, JavaScript, and TypeScript run in an execution image pinned by digest, in processes separate from the main service.

## Execution and file interfaces

- [service.rs](src/service.rs): `Sandbox::open` checks the backend and image; `start` creates an execution job; `receipt` reads its receipt; `cancel_invocation` cancels a call; and `stop` closes the backend.
- [types.rs](src/types.rs): `RunSpec`, `Runtime`, `Limits`, `Workspace`, and `Receipt` define execution inputs and results.
- [job.rs](src/job.rs): `JobHandle` manages waiting for completion and cancellation.
- [task/](src/task/): `TaskStore` supports creation, reading, search, editing, patches, and execution leases. `TaskSnapshot` records a workspace revision.
- [filesystem.rs](src/filesystem.rs), [output.rs](src/output.rs): validate file paths and collect outputs.

Containers use a read-only root filesystem, read-only package mounts, and a writable task directory. Networking is disabled by default, with CPU, memory, PID, runtime, and output limits. File operations validate owner, session, and workspace revision; execution receipts record exit code, output, truncation, and cleanup status.

The main-service account needs Docker permission, and the execution image must be prepared in advance. See [Deployment](../../docs/DEPLOYMENT.md) for sandbox connectivity when the main service itself runs in a container.

## Tests

[tests/](tests/) covers OCI execution, task files, precise edits, execution leases, and cancellation. Set `EXOHARNESS_PRO_TEST_SANDBOX_IMAGE`, then run:

```sh
cargo test --locked -p exoharness-sandbox
```

Related design: [Coding workflow](../../docs/CODING_WORKFLOW.md), [Security](../../docs/SECURITY.md).
