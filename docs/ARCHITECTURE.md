# Architecture

English | [简体中文](ARCHITECTURE.zh-CN.md)

ExoHarness has one application-composition entry point, one extension host and one Agent loop. The management, user and file endpoints share execution facts while retaining separate routes and authorization boundaries.

```text
Management / User             File service
       │                           │
      API ─────────────────── Artifacts
       │
 Agent + Tools ── ModelRuntime / MCP / Skills / Sandbox
       │                                  │
 Contracts + Core                  Coding workspace
       │
 PostgreSQL, configuration generations, Secret Store, private files

App owns configuration, extension registration, dependency injection,
listeners and shutdown; it does not host a second business loop.
```

## Module responsibilities

| Directory | Responsibility |
| --- | --- |
| `modules/contracts` | Native model/tool protocols, execution states, snapshots, invocation identity, validation and repeated-call policy |
| `modules/core` | Extension dependency graph, typed services, resource ownership, lifecycle and context projection |
| `modules/agent` | Model requests, pending calls, budgets, bounded repair, stage projection and stop decisions |
| `modules/tools` | Authorized catalog, capability discovery/activation, live schemas, execution adapters and workflow dependency resolution |
| `modules/api` | HTTP, identity adapters, management/user routes, SSE, Trace and file reads |
| `modules/app` | Service composition, three listeners, background tasks and graceful shutdown |
| `modules/registry` | Capability versions, permissions, health, discovery evidence and audit |
| `modules/configuration` | Model/assistant generations, validation, private config and avatars |
| `modules/model-runtime` | Streaming LLM protocols and speech synthesis/recognition adapters |
| `modules/identity` | Users, registration, login sessions, passwords, installation policy and assistant assignment |
| `modules/skills` / `skill-runtime` | Immutable packages, instruction/resource reads and pinned runtime checks |
| `modules/mcp` / `secrets` | Standard MCP, schemas, real progress, background handles and private credentials |
| `modules/sandbox` | Isolated OCI execution, persistent workspaces, limits, cancellation and receipts |
| `modules/coding` | Pure policies for paths, precise edits, patches and focused search |
| `modules/artifacts` | File persistence, provenance, digests, signatures and project publication |
| `modules/storage/postgres` | Persistent execution/registry/knowledge transactions; blocking boundary for synchronous operations |
| `modules/storage/sql-compat` | PostgreSQL-compatible access used by identity and related modules |
| `web-console` / `web` | Frontend source and built assets |

Contracts do not depend on HTTP, storage or vendors; Core does not depend on business modules. Agent calls models and dispatchers through narrow ports. HTTP adapters do not duplicate state machines, and business modules must not bypass module interfaces through a global API-state container.

## Extensions and lifecycle

An extension declares `ExtensionSpec { id, provides, requires }`. Before starting any extension, the host checks duplicate, missing, cyclic and ambiguous providers. Typed `Service::KEY` limits service publication and lookup; an extension can access only declared dependencies.

Resource ownership is registered at initialization and released in reverse dependency order. Partial startup failure also triggers cleanup; a failed release is not treated as successful removal. The host provides static registration and lifecycle management, not arbitrary dynamic code loading, hot replacement or full in-flight generation leases.

## One execution

1. After authentication, load the user Session and assigned assistant, then save the Turn, Run and pinned model/capability snapshot.
2. Agent discovers capabilities authorized for the current task, reads Skills as needed and activates live tool schemas.
3. Before invocation, check revision, authorization, readiness and native JSON, then prepare a stable invocation.
4. Claim invocation admission and commit state/events in a transaction. Only then call the external service.
5. Trusted adapters persist exact receipts. MCP progress flows directly into the event stream; background polling does not call the model.
6. Successful artifacts enter the shared file service. The model answers from delivery facts, while complete invocation data stays in the journal.

Run admission checks global and per-user capacity before the first model request. Defaults are 1,000 globally and 2 per user; a Session remains serial. Excess Runs stay `queued`, with a durable queue event, and make no model call. Waiting Runs use current limits after an admin change; running Runs are not interrupted.

MCP admission checks capability `limits.max_concurrency` and its assigned shared resource pool together in a short transaction using a persistent invocation ledger. Different MCP services may share one GPU pool; unassigned services retain their own limit. Waiting and cancelled pre-submit calls do not consume attempts. Submitted or unknown-outcome calls hold their reservation until a trusted terminal receipt arrives. Changing pool assignments does not release in-flight reservations.

## Core invariants

- Capability import, runtime preparation, permission grants and actual execution are checked independently.
- The execution journal persists facts; model context is rebuilt for the current step.
- Tool calls and results are projected as pairs. JSON, code and essential work data are not truncated.
- Persistent checkpoints can compact model input but cannot broaden authorization or prove an unknown call succeeded.
- Completed side effects are not resubmitted; a transport disconnect alone does not establish business failure.
- Model requests, argument repairs and tool retries have separate counters; continuing unchanged does not reset budgets.
- Late receipts may be saved after cancellation but cannot restart a finished Run.

See [Feature status](STATUS.md) for recovery and historical-format limitations, and [Database](DATABASE.md) for persistence design.
