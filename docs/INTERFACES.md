# API reference

English | [简体中文](INTERFACES.zh-CN.md)

Management and user portals use separate routers; the file endpoint provides restricted reads. Default ports are 9797 / 9798 / 9790 and can be changed at deployment. API paths do not include ports. This document describes current business APIs, not historical CLI or route inventories.

## Authentication and errors

The browser logs in to receive an HttpOnly cookie, then calls `GET /v1/auth/session` for session and CSRF information. Cookie-authenticated writes include `x-exoharness-csrf`. API clients may use a Bearer token from a restricted credential file. `actor`, owner and assistant IDs are not proof of identity.

Management writes require an administrator; user-data routes validate the current owner. Configuration, secrets and Trace management routes are not registered on the user portal. Authentication validates Host/Origin; reverse-proxy requirements are described in [Security](SECURITY.md).

```json
{"error":{"code":"state_conflict","message":"State changed; refresh and use the current revision."},"resumable":false}
```

Common status codes: 400 invalid input, 401 unauthenticated, 403 forbidden, 404 absent within the current scope, 409 revision/state conflict or unknown outcome, 413 upload too large and 503 backend not ready. Clients should use the error code rather than parse human text. Not every 409 means “resume execution.”

Configuration and capability writes include `expected_revision` where required; stale updates do not silently overwrite data. Exact DTOs are defined by each handler's Deserialize type. A separate OpenAPI specification is not provided.

## Sessions and execution

| Method | Path | Purpose |
| --- | --- | --- |
| GET / POST | `/v1/sessions` | List/create; listing supports `q` search |
| GET / PATCH / DELETE | `/v1/sessions/{session}` | History, title/pin update, archive/delete; history supports `turn_limit` and `before_ordinal` |
| POST | `/v1/sessions/{session}/turns` | Submit an execution; returns 202 and Run information |
| GET | `/v1/runs/{run}/events` | SSE events; `Last-Event-ID` is the last consumed sequence |
| POST | `/v1/runs/{run}/cancel` | Stop a Run; external cancellation depends on the service receipt |
| POST | `/v1/runs/{run}/resume` | Currently returns 409 `resume_unavailable`; it does not claim unsafe recovery |
| GET | `/v1/sessions/{session}/response-actions` | Read user actions on completed responses |
| GET / POST | `/v1/runs/{run}/response-actions` | Record copy, like, dislike and regeneration-related actions |
| GET | `/v1/chat/configuration` | Safe projection of current chat settings |
| GET | `/v1/agent-profile`, `/v1/agent-profiles/default` | Public projection of the assistant assigned to the user |

Minimal request sequence:

```text
POST /v1/sessions                       {"title":"Example task"}
POST /v1/sessions/{session}/turns       {"prompt":"Calculate and plot the result","attachments":[]}
GET  /v1/runs/{run}/events              Last-Event-ID: 0
GET  /v1/sessions/{session}
```

A Turn DTO may include `model`, `capability_ids` and `actor`. The server authorizes from the authenticated identity and pins the snapshot. Production chat uses the Session/Turn/Run routes above; `/v1/chat` and `/v1/execute/*` are restricted validation routes.

SSE sends each committed Chat payload as JSON `data` with an event ID, including text, calls, real progress and terminal state. Reconnection replays committed events; it does not rerun tools. The stream ends after `complete`, `failed`, `cancelled` or `recoverable`. Persistent-state reads and event replay are separate APIs; stream disconnection does not mean the task failed.

## Capabilities and imports

These management routes register capabilities:

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/v1/capabilities`, `/v1/capabilities/overview` | Registrations and permission/health/readiness state |
| GET | `/v1/assistant-capabilities` | Assistant grant catalog, including the native sandbox entry |
| GET / DELETE | `/v1/capabilities/{id}` | Detail/delete |
| GET | `/v1/capabilities/{id}/versions`, `/health`, `/runtime` | Revision, health and runtime status |
| PUT | `/v1/capabilities/{id}/enabled` | Enable/disable a revision |
| GET | `/v1/capabilities/{id}/permissions` | Permission decisions |
| PUT | `/v1/capabilities/{id}/permissions/{permission}` | Allow / Deny / Pending |
| GET / POST | `/v1/capabilities/{id}/preparation` | Runtime checks and explicit preparation; permissions remain separate |
| POST | `/v1/capabilities/{id}/validate` | MCP initialize/discovery or Skill integrity check |
| GET | `/v1/capabilities/{id}/tools` | Current validated native tool schemas |
| GET | `/v1/capabilities/{id}/secrets` | Credential status; values are not returned |
| PUT / DELETE | `/v1/capabilities/{id}/secrets/{name}` | Rotate/delete a secret reference |
| POST | `/v1/imports/manifest` | Paste full MCP JSON in `content` |
| POST | `/v1/imports/mcp-http` | Import an MCP by URL |
| POST | `/v1/imports/skill/preview` | Read-only package/revision preview |
| POST | `/v1/imports/skill`, `/v1/imports/skill/package` | Upload a Skill tree/ZIP |
| GET | `/v1/capabilities/{id}/skill-instructions`, `/skill-files` | Full instructions and package-resource metadata |

Example MCP content uses a placeholder address and no real secret:

```json
{"mcpServers":{"example":{"type":"streamableHttp","url":"https://mcp.example.com/mcp"}}}
```

Import parses and registers a service without starting it. Sensitive headers move to the Secret Store. ToolNode records exist, but standalone Tool execution preparation is not connected; do not confuse this with the working native sandbox path.

## Models, assistants and speech

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/v1/model-catalog` | Management model/provider types and defaults |
| POST | `/v1/model-configuration` | Unified model-save entry point |
| POST | `/v1/model-providers`, `/v1/models` | Save providers and models |
| DELETE | `/v1/model-providers/{name}`, `/v1/models/{name}` | Delete configuration |
| PUT | `/v1/model-providers/{name}/enabled`, `/v1/models/{name}/enabled` | Enable/disable |
| PUT | `/v1/models/{name}/default` | Set the default for a type |
| POST | `/v1/models/{name}/test` | Typed protocol test |
| GET / POST | `/v1/agent-profiles` | List/create assistants for management |
| GET / PUT / DELETE | `/v1/agent-profiles/{id}` | Read/update/delete an assistant |
| PUT | `/v1/agent-profiles/{id}/enabled`, `/default` | Enable/disable and set default |
| POST / GET | `/v1/agent-assets`, `/v1/agent-assets/{id}` | Upload/read authorized avatar assets |
| GET | `/v1/speech-input/configuration` | ASR readiness for user/management clients |
| POST | `/v1/speech-input` | Upload a recording for transcription; audio does not enter normal LLM context |

See [Model configuration](MODEL_CONFIGURATION_DESIGN.md) for protocol support.

## Users, knowledge and management

| Method | Path | Scope |
| --- | --- | --- |
| POST | `/v1/auth/login`, `/v1/auth/logout` | Login/logout on the corresponding portal |
| GET | `/v1/auth/session`, `/v1/auth/status` | Current identity/authentication state |
| POST | `/v1/auth/registration-code`, `/v1/auth/register` | User verification code/registration |
| GET | `/v1/account` | User account |
| PUT | `/v1/account/profile`, `/v1/account/password` | Update profile/password |
| GET | `/v1/user-capabilities`, `/v1/user-capabilities/install-jobs` | User capability view; currently `installation_ready=false` |
| GET | `/v1/management/users`, `/v1/management/users/{id}` | Admin user list/detail |
| PUT | `/v1/management/users/{id}/assistant`, `/capability-policy` | Assign assistant/user policy |
| PUT | `/v1/auth/passwords/{role}` | Set an administrator-role password |
| GET / PUT | `/v1/auth/registration-email` | Configure registration email template |
| GET / PUT | `/v1/scheduler-configuration` | Limits, shared pools and MCP assignments; PUT requires current `revision` |
| GET | `/v1/management/overview`, `/v1/audit` | Management overview/audit |
| GET | `/v1/trace/sessions`, `/v1/trace/runs`, `/v1/trace/runs/{run}` | Cross-user Trace lists and details |
| GET | `/v1/trace/runs/{run}/requests/{logical}/{attempt}` | Request manifest; `part` reads evidence belonging to that request |
| GET / POST | `/v1/knowledge/categories`, `/v1/knowledge/documents`, `/v1/memories` | Query/create within the current user's scope |
| PUT / DELETE | `/v1/knowledge/categories/{id}`, `/v1/memories/{id}` | Update/delete |
| GET / PUT / DELETE | `/v1/knowledge/documents/{id}` | Document detail/update/delete |
| POST | `/v1/knowledge/documents/import` | Upload a knowledge file |
| GET | `/v1/knowledge/search`, `/v1/knowledge/documents/{id}/blocks[/{block}]` | Search, structural block list and versioned read |
| GET / POST | `/v1/knowledge/libraries` | Manage shared libraries |
| GET / PUT / DELETE | `/v1/knowledge/libraries/{library}` | Library detail/update/delete |
| PUT | `/v1/knowledge/libraries/{library}/grants` | Grant library access |

Shared libraries have corresponding `/v1/knowledge/libraries/{library}/categories...` and `/documents...` routes. Brackets denote an optional path segment; use an actual path in requests. See [Knowledge import](KNOWLEDGE_DOCUMENT_IMPORT.md) for fields, formats and read scope.

Trace lists support `limit`/`offset`; Run lists require `session_id`. Details support `event_after`/`event_limit` to page the timeline. A request `part` must belong to the Run/request and is not an arbitrary file-read route.

Scheduler GET returns `revision`, `max_active_runs_per_user` (1–128, default 2), `max_global_agent_runs` (1–1024, default 1,000), `resource_pools`, `mcp_resource_assignments`, millisecond `updated_at` and read-only `mcp_capabilities`. PUT submits the first six fields, not the read-only catalog; revision conflicts return 409 `state_conflict`. A pool is `{id,name,max_concurrency}` and an assignment is `{capability_id:pool_id}`; assignments must reference a registered MCP and existing pool. Saving the configuration does not change permissions.

Queue events arrive over SSE as `resource_queue` with `state: waiting | acquired` and `message`. Run waits omit `tool_call_id`; MCP waits include it. Waiting does not call a model or increment execution attempts. Overview scheduler fields report actual active/waiting counts, user/global limits and pool usage.

## File service

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/health` | File-service health |
| GET | `/artifacts/{id}/{filename}` | Signed download/Range |
| GET | `/artifacts/{id}/{filename}/metadata`, `/preview` | Metadata/restricted preview |
| GET | `/projects/{id}/{signature}/{*path}` | Multi-file static preview within a published manifest |
| GET | `/workspace-files/{id}/{filename}`, `/preview` | Compatibility file routes |

The file service creates resource paths and signed queries. Clients use the returned address; access is limited to registered files and authorized project manifests.

Routes are implemented under `modules/api/src/{chat,conversations,registry,skills,mcp,configuration,identity,knowledge,media}/`, with Trace in `trace.rs`. `/v1/local/*` and `/local-validation` are development-validation interfaces; infrastructure status fields there do not mean production chat is disconnected.
