# Database and persistence

English | [简体中文](DATABASE.zh-CN.md)

PostgreSQL stores relational state. Managed directories hold model/assistant configuration, packages, secrets and artifact files. The database records paths, identities, revisions and SHA-256 values; Base64 file content is not used as persistent work data or model context.

## Data groups

| Data | Main tables/storage | Purpose |
| --- | --- | --- |
| Sessions and execution | `execution_sessions`, `execution_turns`, `execution_runs` | Owner, input, state, revision, time and execution snapshots |
| Assistant snapshots | `execution_session_agent_profiles`, `execution_run_agent_profiles` | Pin assistant identity and revision for each execution |
| Invocation ledger | `execution_invocations`, `execution_background_observations` | Full arguments/results, stable identity, submission and background state |
| Scheduling | `execution_scheduler_configuration`, `execution_resource_reservations` | Versioned concurrency settings and in-flight shared-pool reservations |
| Events and context | `execution_events`, `execution_context_checkpoints`, `execution_context_transforms` | Ordered events, original context and projection evidence |
| Models and usage | `execution_model_calls`, `execution_model_requests`, `execution_request_parts`, `execution_run_budgets` | Requests, actual usage, budgets and on-demand read evidence |
| Artifacts | `execution_run_artifacts`, `execution_artifact_files`, `execution_artifact_configuration` | Source Run, digest, file registration and delivery settings |
| Conversation actions | `execution_response_actions`, `execution_session_metadata`, and related tables | Feedback, pinning, titles and session metadata |
| Capability registry | `capabilities`, `capability_versions`, `capability_permissions`, `capability_health` | Definitions, immutable versions, permissions and health |
| Discovery and audit | `capability_discovery`, `registry_audit` | Live tool schemas, service evidence and change history |
| Identity | `auth_users`, `auth_user_sessions`, `auth_passwords`, `auth_sessions`, and related tables | Users, password hashes, sessions, email verification, assistant assignments and policies |
| Knowledge and memory | `knowledge_documents`, `knowledge_categories`, `knowledge_libraries`, `knowledge_library_grants`, `personal_memories` | User materials, categories, shared-library grants and personal memory |
| Private files | Configuration generations, package directories, Secret Store, Sandbox, Artifacts | Model/assistant config, credentials, original documents, task source and output |

Execution DDL retains some historical tables. Current scheduler settings use the singleton `execution_scheduler_configuration`; shared-pool reservations reference invocations. The presence of another legacy table does not mean its execution path is connected; see [Feature status](STATUS.md).

## Configuration and migrations

`EXOHARNESS_DATABASE_URL` provides the connection address; `EXOHARNESS_DATABASE_PASSWORD_FILE` can supply a separate password file. `EXOHARNESS_DATABASE_NAMESPACE` selects the execution namespace (default `execution`); the App currently uses `registry` and `auth` for registration and identity. Knowledge tables are migrated with execution storage; do not invent a separate connection setting.

DDL is under `modules/storage/postgres/migrations/`; identity DDL is in `modules/identity/src/schema.rs`. Incremental migrations cover execution, registration, discovery evidence, knowledge, document structure and titles. Migrations run explicitly, use transaction locks and do not erase existing data when repeated.

PostgreSQL storage migration locks are scoped to the current schema. Concurrent
migrations of the same namespace serialize; independent namespaces do not share
that advisory lock. Replace a running version before starting its successor's
migrations in the same namespace.

Model/assistant configuration uses immutable generations and a current pointer in `modules/configuration`; runtime refreshes configuration while each execution snapshot pins its revision. Credentials are not written into public configuration projections or Run snapshots.

## Transactions and concurrency

A Session is the owner-validation and Turn-creation lock boundary; two active Runs cannot be driven simultaneously in the same Session. Run revisions correspond to committed event sequence numbers. State, Turn, control metadata and events commit in one transaction; failure at any step rolls it back.

Invocation preparation and admission are separate. An intent includes the pinned capability revision, native arguments, digest and logical key. Repeating the same intent is idempotent; a conflicting intent is rejected. MCP admission takes a short advisory lock to check concurrency and queue order. It does not hold a database lock during a network call or create another queue database.

Scheduler updates, Run admission and shared-pool admission use a common transaction lock in the execution namespace; configuration writes use revision compare-and-swap. Active Run counts include `running` and `waiting_authorization`; `queued` Runs do not occupy an execution slot. MCP pool usage is calculated from submitted ledger entries, including `outcome_unknown`. Cancelling a Run alone does not release an external reservation. `0005_scheduler.sql` is an additive migration and does not delete historical records.

`context_json._exoharness_pro` stores host control metadata. Updating that key preserves other context fields. Full arguments and results are stored in the invocation ledger rather than duplicated in control metadata.

The synchronous PostgreSQL driver uses a connection pool; API and Agent work crosses a `spawn_blocking` boundary. PostgreSQL currently uses NoTls, suitable only for localhost or a protected private network—not an exposed public database.

## Compatibility and backups

Historical Run control data must be explicitly migrated to the current format before resuming. Migration validates invocation state and arguments, and only one version may hold execution ownership for a Run. See [Migration](MIGRATION.md).

Back up PostgreSQL, private configuration and keys, package directories, task files and artifacts from the same time window. A database-only backup cannot restore files or model credentials. Losing a signing key makes existing links unverifiable. A restore drill must check permissions, revisions, file digests and downloads as well as database connectivity.
