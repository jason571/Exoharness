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
cargo test --locked --offline -p exoharness-tools --lib -p exoharness-api --lib
cargo test --locked --offline -p exoharness-storage-postgres --test persistence
cargo test --locked --offline -p exoharness-api --test tool_chat -- --test-threads=1
cargo test --locked --offline -p exoharness-sandbox -p exoharness-skill-runtime -- --test-threads=1
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/tests -v
npm --prefix web-console run typecheck
npm --prefix web-console run test:capability-install
node scripts/tests/chat-artifact-authority.mjs
node scripts/tests/chat-saved-outputs.mjs
node scripts/tests/browser_portal_i18n.mjs
node scripts/tests/browser_management_theme.mjs
cargo fmt --all -- --check
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
