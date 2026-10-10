# Code modules

English | [简体中文](README.zh-CN.md)

ExoHarness uses a Rust workspace for services, protocols and execution components. The table links to each crate's responsibilities and entry points. Module boundaries and dependency rules are described in [Architecture](../docs/ARCHITECTURE.md).

| Module | Responsibility |
| --- | --- |
| [Contracts](contracts/README.md) | Cross-module protocols, data types and pure validation |
| [Core](core/README.md) | Extension host, service dependencies and resource lifecycle |
| [Agent](agent/README.md) | Model/tool loop, context and budgets |
| [Tools](tools/README.md) | Capability discovery, schema activation and dispatch |
| [Registry](registry/README.md) | Authorized capability catalog |
| [Skills](skills/README.md) | Skill packages, versions and resources |
| [Skill Runtime](skill-runtime/README.md) | Runtime checks and revision evidence |
| [MCP](mcp/README.md) | MCP configuration, connections, discovery and progress |
| [Model Runtime](model-runtime/README.md) | LLM, TTS and ASR protocol adapters |
| [Sandbox](sandbox/README.md) | OCI execution and task workspaces |
| [Coding](coding/README.md) | Code search, precise edits and patch algorithms |
| [Configuration](configuration/README.md) | Models, assistants, avatars and configuration revisions |
| [Identity](identity/README.md) | Users, authentication, policies and assistant assignment |
| [Secrets](secrets/README.md) | Private credential storage |
| [Artifacts](artifacts/README.md) | File storage, signed access and project publication |
| [PostgreSQL Storage](storage/postgres/README.md) | Execution, registry, sessions and knowledge persistence |
| [Database Access](storage/sql-compat/README.md) | PostgreSQL SQL access adapter |
| [API](api/README.md) | HTTP, SSE, access control and file responses |
| [Application](app/README.md) | Service composition, listeners, startup and shutdown |

Production implementation lives in each module's `src/`; tests live in `tests/`. Commands in module READMEs run from the repository root. See [Testing and acceptance](../docs/TESTING.md) for database and OCI environment setup.
