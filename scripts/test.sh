#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
if [[ "${1:-}" == "--unit-only" ]]; then
    cargo test --locked --offline -p exoharness-contracts -p exoharness-core -p exoharness-coding
    printf '%s\n' 'Unit-only gate: PostgreSQL integration was explicitly excluded.'
elif [[ $# -eq 0 ]]; then
    if [[ -z "${EXOHARNESS_PRO_TEST_DATABASE_URL:-}" ]]; then
        printf '%s\n' 'Set EXOHARNESS_PRO_TEST_DATABASE_URL to a disposable PostgreSQL database. No silent integration-test skip.' >&2
        exit 2
    fi
    cargo test --locked --offline --workspace
else
    printf '%s\n' 'Usage: bash scripts/test.sh [--unit-only]' >&2
    exit 2
fi
cargo fmt --all -- --check
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
python3 -m unittest discover -s scripts/tests -v
