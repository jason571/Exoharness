# Skills, MCP and sandbox integration

English | [简体中文](CAPABILITIES.zh-CN.md)

Capability lifecycle separates registration, authorization, preparation, validation, execution and delivery. Import stores content and a revision; connection tests, runtime checks and user grants have separate states.

## Skills

The management console can import a single file, directory tree or ZIP. The package root contains `SKILL.md`; instructions, reference files and scripts are stored in an immutable package revision. Reading instructions provides reference data: it cannot grant permissions, replace system rules or claim that a tool was executed.

Runtime checks are pinned to capability ID, revision, package digest and runtime-image SHA. Checkers inspect interpreters such as Python/Node and supported dependency declarations inside an isolated environment. They do not run uploaded scripts, installation hooks or arbitrary pip/npm commands. A package update or image change invalidates the previous evidence.

The native Sandbox can mount an authorized Skill package and run a saved entrypoint. User/assistant permissions, package revision and sandbox readiness are all checked. The generic managed-Tool field `script_execution_ready` and native Sandbox dispatch are different interfaces; one field alone does not prove that every script is bound and executable. Missing dependencies require controlled preparation; import does not install packages from the network.

Deletion removes the Skill registration and its package contents. Historical execution evidence, call references, audit data and generated files follow their respective retention policies.

## MCP

The standard SDK performs Streamable HTTP initialization, capability negotiation and paginated tool discovery. The management console accepts pasted `mcpServers` JSON and URL-based imports. Sensitive header values are removed from the manifest and restored at call time without changing their authentication semantics.

Discovery evidence is bound to a capability revision and includes tool schemas, a digest and `serverInfo`. A revision or permission change invalidates old tool-cache evidence. Calls use the live input schema and native object/array parameters. `isError`, `structuredContent` and `resource_link` are retained as protocol facts.

stdio configurations can currently be registered, but the host does not launch them. Managed stdio execution is not complete; support for remote HTTP must not be generalized to claim that every MCP transport is ready.

## Cross-capability references and release consistency

Attachment IDs, Artifact IDs, task IDs and signed URLs are different reference types. Follow-up calls pass the original value from the actual tool result according to the live schema, preserving signatures and provenance. If a reference type changes, update and validate both the capability interface and Skill instructions.

When updating a capability:

1. Deploy the service code or runtime image and verify its actual mount path and version.
2. Update the registered Skill package and verify its current revision and the instructions actually read.
3. Rediscover MCP tools and confirm the registered live schemas match the service.
4. Use retained inputs and artifacts to verify cross-capability calls, types, identities, access and output.
5. Repeat a submission with the same task reference to confirm result reuse; record cancellation and error cases as needed.

Deployment code, registration revision and discovery evidence are checked separately, retaining original execution journals.

## Authorization and readiness

Execution requires a registered capability, matching revision, enabled state, permitted declared permissions, user policy, assistant scope and an available backend. Selecting a Skill does not authorize all MCP or TTS services it might use; each execution capability requires its own grant.

Service business errors are execution facts. The framework must not infer resolution, duration, story or URLs from a business name. Inter-service dependencies are declared through registered connections; the framework resolves and authorizes them.

## Progress, background work and cancellation

Synchronous calls use standard MCP `progressToken` and progress notifications. Service-reported progress and stages are forwarded as UI events. Valid progress renews the inactivity wait; other resource budgets are checked independently.

An asynchronous workflow may explicitly use the ExoHarness `_meta.exoharness.background_task` extension:

```json
{
  "version": 1,
  "id": "opaque-operation-id",
  "status": "working",
  "poll": {"tool": "inspect_operation", "arguments": {"id": "opaque-operation-id"}},
  "cancel": {"tool": "stop_operation", "arguments": {"id": "opaque-operation-id"}},
  "poll_interval_ms": 2500,
  "progress": {"progress": 42, "total": 100, "message": "Service-reported stage"}
}
```

Tool names and arguments are declared by the MCP service. Calls remain bound to authorized capabilities and revisions. States are `working`, `completed`, `failed` or `cancelled`; terminal states include the actual result. This extension is distinct from the native MCP Tasks protocol.

The backend polls a stable task handle and records status; the model receives only the required terminal result. Cancellation requires confirmation from the external service. If it cannot be confirmed, the outcome remains unknown and duplicate submission is blocked.

## Artifacts

An MCP service can declare artifacts with `resource_link` or `structuredContent.artifacts[]`; a single-file `structuredContent.result` is supported when it includes `filename`, `content_type` and `url` or `download_url`. Presentation uses the original tool receipt and does not depend on a model mentioning a Markdown link. Error receipts and files marked `artifact_visibility=internal` are excluded from final cards; duplicate declarations are deduplicated by URL and filename.

Files saved by Harness use permanent signed links. Same-origin MCP files may be saved through the current controlled delivery path; other MCP files retain the service-issued address and expiry. Services configure their own lifetime. Cross-origin addresses do not grant the host permission to fetch arbitrary URLs. File deletion and signing-key rotation can revoke existing access.

## Concurrency

`limits.max_concurrency` applies at the MCP submission boundary; supported values are currently 1–1024. Admission waits before the actual business submission. The persistent ledger coordinates across sessions and processes; waiting does not count as another attempt. A background call holds its reservation until a trusted terminal receipt; cancelling a Run alone does not prove that a GPU job ended.

Administrators create shared pools in the execution-scheduler settings under Login & Notifications and associate MCP services that use the same resource. Each call follows both its MCP limit and its pool limit. For example, several capabilities associated with a GPU pool of concurrency 1 are serialized across services. A resource-pool assignment limits resource use; it does not grant execution permission.

Global Agent and per-user active-Run limits are separate from MCP limits. Defaults are 1,000 globally and 2 per user. Excess Runs wait before their first model request. Up to 4,096 running or waiting workers can be held by the task service. The global limit is an admission policy, not a promise of throughput; actual capacity depends on model quota, database connections and machine resources. GPU work still obeys each MCP and shared-pool limit.

## Sandbox

Python, Shell, JavaScript and TypeScript run in fixed OCI images, isolated per task. The backend configures a read-only root filesystem and packages, writable workspace, network policy, non-root identity and resource limits. TypeScript requires a Node.js version with native type stripping in the image.

Code execution requires a ready sandbox; administrators prepare images and dependencies. The main-service environment and task sandbox are configured separately. Generic computation and plotting use dedicated execution tools; Skill-specific runtime dependencies are handled according to each package's declaration.

Implementation: `modules/{skills,skill-runtime,mcp,secrets,sandbox,tools}`. HTTP routes are listed in [API reference](INTERFACES.md); the complete Coding workflow is in [Coding](CODING_WORKFLOW.md).
