# Testing and Acceptance

[English](TESTING.md) | [简体中文](TESTING.zh-CN.md)

Tests cover tool submission, arguments and receipts, artifact availability, and safe stopping on failure. Acceptance evidence is tracked separately for unit, protocol, integration, browser, real-task, and deployment checks.

## Baseline gates

```sh
# Contract/Core/Coding, static, and script gates; excludes PostgreSQL integration
export EXOHARNESS_PRO_TEST_SANDBOX_IMAGE='<prepared-sandbox-image>'
bash scripts/test.sh --unit-only
# Full source gate; requires a dedicated disposable test database
EXOHARNESS_PRO_TEST_DATABASE_URL='<private-test-connection>' bash scripts/test.sh
# Use when the local test database has already been prepared
python3 scripts/test-local.py
```

The default gate runs workspace Rust tests, formatting, all-target Clippy, and Python script tests. Cargo uses the lockfile and offline cache. Database integration tests use a dedicated disposable database. `--unit-only` runs unit, static, and Python checks; tests that verify isolated document parsing require Docker and a prepared sandbox image and fail explicitly when the environment is unavailable.

OCI tests require a prepared image and Docker execution permission. Set `EXOHARNESS_PRO_TEST_SANDBOX_IMAGE` to select the test image. Reports distinguish passed, failed, skipped, and environment-blocked cases.

## Coverage levels

| Level | Coverage |
| --- | --- |
| Unit | Dependency graphs, state, schemas, JSON, call/result pairing, Skill deduplication, budgets, and patches |
| PostgreSQL | CAS, transaction rollback, ownership, invocation claims, restart recovery, queue concurrency, and unknown outcomes |
| Scheduler | Default/persisted settings, revision conflicts, global and per-user limits, process races, pool reassignment, and unknown-call reservations |
| Model/MCP protocols | Streaming tool arguments, completion markers, live schemas, progress, background handles, and cancellation |
| OCI | Network denial, paths, resource limits, OOM, timeouts, cancellation, persistent files, and execution receipts |
| Browser | Login/settings, uploads, SSE, real media, Trace, mobile layout, and multi-file previews |
| Real-model tasks | Correct delivery, steps, input/output/cache usage, failures, and quality for each task type |
| Deployment | Release digest, ABI, health, authenticated domains, existing data, and actual file downloads |

Rust tests are in `modules/*/tests/`. Private implementation tests are included through explicit cfg paths rather than expanding public interfaces for testing. Script and browser tests are in `scripts/tests/`. Tests that require a running service must be run serially as documented; do not run scripts that restart services while user tasks are active.

## Critical regressions

- Ordinary conversation must not trigger tools. Authorized Skills/MCPs are discoverable; unauthorized capabilities are not contacted.
- With multiple services and a 68-tool catalog, selection is service-scoped rather than a full traversal. Exact-name batches cover missing names, duplicates, and authorization boundaries. A changed query restarts pagination; continuation cursors are bound to query scope and version.
- After selection, the old catalog is retired from model context without changing the journal. Unselected or failed selections, new queries, Skill instructions, and business JSON remain intact; tool calls and results stay paired.
- Unknown activation names require authorized discovery and a corrected whole selection within the Run's two-correction budget. Rejection preserves the loaded set and resolved names; unavailable or ambiguous tools are never guessed or granted.
- Every MCP call has its corresponding result. A valid checkpoint can be compacted without changing the full journal; invalid checkpoint declarations fall back safely.
- Repeating the same tool with identical arguments is limited to three consecutive attempts. Search, reads, and background polling must not create an unbounded model loop.
- With an MCP limit of one, a second call queues before submission. Cancelling while queued does not count as an attempt; unknown outcomes continue to hold capacity.
- If cancellation races with progress updates, rejecting late progress must not discard the execution adapter. Persist a trusted cancellation receipt before releasing capacity.
- Verify real TTS/ASR output; image, song, and video rendering, downloads and Range requests; and relative web assets.
- A failed coding check should trigger only a local repair; stale revisions must not be written. DOCX/PDF and table structures must not be corrupted by truncation.
- Success, error, cancellation, disconnect, and late receipts retain consistent state and never resubmit an invocation with an unknown side effect.

## Cost acceptance

For comparable inputs, record successful delivery, model requests, capability calls, argument repairs, input/output/cached usage, waiting time, and execution time per Run. Distinguish session totals from per-turn costs and mark unknown usage separately.

Use a mock LLM for protocol regression and real-model tasks for quality and cost acceptance. GPU and paid calls must run in an authorized environment with cancellation and budget controls. Public reports should use redacted aggregate data.

## Interface language regression

```sh
npm --prefix web-console run build
node scripts/tests/browser_i18n.mjs
```

These tests use the actual page structure and compiled assets. They cover language switching across 15 pages, preference persistence after refresh, dynamic status updates, form values, click handlers, and mobile navigation. Chat content, MCP messages, raw Trace data, and user conversation titles remain unchanged.

## Scheduler and management-console acceptance (2026-10-09)

| Check | Result |
| --- | --- |
| Workspace Rust regression | All non-ignored cases passed; a final 45-case conversation/tool integration run passed |
| Scheduler storage | 7 cases passed, covering cross-process races, independent capability limits, shared pools, cancellation, unknown receipts, and reassignment |
| Formatting and static analysis | `cargo fmt --all -- --check` and Clippy for all modules and targets passed |
| Release and scripts | 45 cases passed, including DOCX/PDF parsing with the cached OCI image |
| Browser | Passed across 10 management pages, both languages, 6 viewport widths, collapsed navigation, themes, header layout, Trace, and configuration-error handling |
| Local and production processes | Authenticated checks verified embedded assets, consistent page widths, configuration reads, unchanged-value saves, stale-revision rejection, and overview data |

Run and MCP queue behavior was verified with a mock LLM and persistent ledger: while queued, model-request, tool-submission, and attempt counts did not increase. The cancellation-race test committed the terminal database state before receiving late progress and confirmed that the adapter still completed cancellation and persisted its receipt.

Before the production update, back up runtime files and the execution schema. After updating, verify the process SHA-256, all three listeners, and authenticated endpoints. Acceptance preserved the existing concurrency limit and capability grants and did not submit a new GPU-generation task.
