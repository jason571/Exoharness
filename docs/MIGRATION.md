# Data and configuration migration

English | [简体中文](MIGRATION.zh-CN.md)

Migration preserves users, configuration, capability packages and generated files. Historical records are classified as read-only or eligible to continue under the current execution protocol, and versions are checked during migration.

## Migration scope

- PostgreSQL: identities, capability registrations, sessions, events and file metadata.
- Private configuration: model/assistant generations, credentials and avatars.
- Files: Skill packages, task workspaces, source documents, artifacts and signing keys.
- Deployment: ports, database namespace, reachable external-service addresses and sandbox-image digests.

A migration snapshot includes the database, capability packages, generated files, model configuration and separately stored credentials. Map old environment variables to new settings one at a time; enable the new service only after paths, ports and permissions have been checked.

## Sequence

1. In a private directory, back up database and files from the same time window and record versions and SHA-256 values.
2. Give the new environment an independent database or verified namespace; do not let old and new processes write the same Run concurrently.
3. Validate configuration, account hashes, assistant assignments, capability revisions/permissions and file references.
4. Explicitly check compatibility of historical Run snapshots and event JSON. Keep unconverted records read-only; do not infer a state and rerun them.
5. Verify identities, reads, representative files and a new task before switching the write owner.

Source utilities `scripts/import-server-config.py` and `scripts/import-server-users.py` perform read-only export/import for an explicitly selected source; inspect `--help` first. They are not part of the standalone release's automatic startup. Do not copy production data into a shared development environment without establishing its source and destination.

## Current boundaries

Database tables and control formats have compatibility checks. When old records lack current control metadata, the service returns `migration_required` rather than silently assigning defaults. Full conversion of old Runs and safe HTTP Resume are not yet implemented.

After importing accounts, models and assistants, verify live permissions, service connectivity and sandbox readiness. Matching image tags do not prove matching image digests. Before reverting to an old service, check data compatibility; two versions must not drive the same task.
