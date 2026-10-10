# Security boundaries

English | [简体中文](SECURITY.zh-CN.md)

Agent output and uploaded files are untrusted input. The server enforces identity, permissions, revisions, paths, network policy and execution limits.

## User and assistant isolation

Users may access only their own Sessions, Runs, materials and memories; administrators can inspect cross-user traces. Management routes are not registered on the user portal. Execution requires the assistant scope, user policy, capability grant and current readiness. A pinned snapshot does not override a later authorization revocation.

Identity comes from server-side authentication. Browsers use persistent cookie sessions and API clients use Bearer credentials. Password hashing, login throttling and CSRF are checked separately. Administrators and regular users have distinct scopes.

## Credentials

Sensitive MCP manifest headers are moved into the Secret Store; APIs return only whether a value is configured. Unix secret files use mode 0600 and their directories use 0700. Configurations and logs do not echo secrets. Header values retain their original semantics; the framework does not add `Bearer` arbitrarily.

Public repositories exclude credentials, environment files, database passwords, verification codes, signed URLs, account exports and real Trace data. Rotation retains old versions referenced by active executions. Automatic collection of orphaned secrets and full generation leases remain incomplete.

## Code and files

Skill import parses and registers a package without executing code or installing dependencies. Preparation runs fixed checkers only. Python/Shell/JavaScript/TypeScript execution must use an OCI sandbox; if it is unavailable, the task stops without falling back to the host.

Task workspaces, read-only package mounts, image digests and authorization are pinned. The backend validates paths and symlink boundaries and applies network-disabled defaults, a non-root identity, restricted mounts and resource budgets. The main service container and task sandbox are managed separately; only a trusted execution backend may use the Docker Socket.

Document parsing does not execute macros, external resources or unknown scripts. Previews expose only published manifests, with an isolated CSP and no same-origin access to the main site.

## External calls

Inputs are checked against the live schema before submission. Stable invocation IDs provide local idempotency and audit records; external idempotency depends on the service protocol. Background polling and cancellation retain the original authorization scope. If a receipt cannot confirm the outcome, the call remains unknown and is not submitted again.

Fetched files are checked against configured sources. Remote service and signed-file addresses come from trusted configuration; deployment network policy controls public and private reachability. Network checks for user-installed capabilities depend on completion of the installation workflow.

## Public deployment

The application binds to loopback by default, with HTTPS terminated by a reverse proxy. Current management and user authentication accept corresponding local Host/HTTP Origin values; a proxy must map Host and Origin correctly and preserve the existing checks.

For public deployment, validate same-origin proxy mapping, secure cookie attributes and the authentication policy. A general public-Origin configuration is not yet available. PostgreSQL currently uses NoTls; restrict its connection to localhost or a protected private network.

Media signatures grant file access. Links without expiry use the same credential protection; use expiration and an appropriate delivery policy when link propagation must be limited.

## Vulnerability reports

Reports should include a minimal redacted reproduction, version and impact scope. Send credentials, user files and full Trace data through a private channel. Configure a private security-reporting channel for the public repository before release.
