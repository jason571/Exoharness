#!/usr/bin/env bash
set -euo pipefail
project_script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$project_script_dir/build-native.py" "$@"
