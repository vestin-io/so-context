#!/usr/bin/env sh
set -eu

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

"$ROOT_DIR/scripts/check_codex_setup.sh"
"$ROOT_DIR/scripts/test_pre_tool_hook.sh"

printf '%s\n' "run_codex_hook_smoke: OK"
