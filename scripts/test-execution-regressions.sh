#!/usr/bin/env bash
# Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
# SPDX-License-Identifier: AGPL-3.0-only
# Commercial licensing: contact flyangqn@gmail.com.
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
: "${EXOHARNESS_PRO_TEST_DATABASE_URL:?Set a dedicated disposable test database URL}"
: "${EXOHARNESS_PRO_TEST_SANDBOX_IMAGE:?Set an already prepared OCI test image}"
cargo test --locked --offline -p exoharness-contracts -p exoharness-agent
cargo test --locked --offline -p exoharness-mcp -p exoharness-model-runtime
cargo test --locked --offline -p exoharness-tools --lib -p exoharness-api --lib
cargo test --locked --offline -p exoharness-api --test chat
csp_fixture_dir="$(mktemp -d "${TMPDIR:-/tmp}/exoharness-preview-csp.XXXXXX")"
trap 'rm -rf -- "$csp_fixture_dir"' EXIT
export EXOHARNESS_PREVIEW_CSP_FIXTURE="$csp_fixture_dir/preview"
cargo test --locked --offline -p exoharness-api --test media
cargo test --locked --offline -p exoharness-api --test identity --test password_reset
cargo test --locked --offline -p exoharness-storage-postgres --test persistence
cargo test --locked --offline -p exoharness-api --test tool_chat -- --test-threads=1
node scripts/tests/browser_preview_links.mjs "$EXOHARNESS_PREVIEW_CSP_FIXTURE"
cargo test --locked --offline -p exoharness-sandbox -p exoharness-skill-runtime -- --test-threads=1
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/tests -v
npm --prefix web-console run typecheck
npm --prefix web-console run test:capability-install
node --experimental-strip-types web-console/tests/execution-notice.test.mjs
node --experimental-strip-types web-console/tests/progress-message.test.mjs
node scripts/tests/browser_chat_progress.mjs
node scripts/tests/browser_registration_code.mjs
node scripts/tests/browser_password_reset.mjs
node scripts/tests/chat-artifact-authority.mjs
node scripts/tests/chat-saved-outputs.mjs
node scripts/tests/browser_portal_i18n.mjs
node scripts/tests/browser_management_theme.mjs
node scripts/tests/browser_execution_notices.mjs
node scripts/tests/browser_partial_result.mjs
EXOHARNESS_TEST_CHROME="${EXOHARNESS_TEST_CHROME:-/usr/bin/google-chrome}" node scripts/tests/browser_trace_timeline.mjs
cargo fmt --all -- --check
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
