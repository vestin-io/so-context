#!/usr/bin/env sh
set -eu

CONFIG_PATH="${CODEX_CONFIG_PATH:-$HOME/.codex/config.toml}"
GUIDE_PATH="${CODEX_GUIDE_PATH:-$HOME/.codex/SO-CONTEXT.md}"
AGENTS_PATH="${CODEX_AGENTS_PATH:-$HOME/.codex/AGENTS.md}"
SO_CONTEXT_BIN="${SO_CONTEXT_BIN:-$(cd "$(dirname "$0")/.." && pwd)/target/debug/so-context}"

fail() {
  printf '%s\n' "check_codex_setup: $*" >&2
  exit 1
}

need_file() {
  [ -f "$1" ] || fail "missing file: $1"
}

assert_contains() {
  file="$1"
  pattern="$2"
  if ! grep -Fq "$pattern" "$file"; then
    fail "expected to find '$pattern' in $file"
  fi
}

assert_state_enabled() {
  key="$1"
  if ! awk -v key="$key" '
    $0 == key { in_block = 1; next }
    in_block && /^\[/ { exit 1 }
    in_block && $0 == "enabled = true" { found = 1; exit 0 }
    END { exit found ? 0 : 1 }
  ' "$CONFIG_PATH"; then
    fail "expected enabled = true for state block: $key"
  fi
}

need_file "$CONFIG_PATH"
need_file "$GUIDE_PATH"
need_file "$AGENTS_PATH"

assert_contains "$AGENTS_PATH" "@$GUIDE_PATH"
assert_contains "$GUIDE_PATH" "# so-context — Read, Search, And Shell Guidance"
assert_contains "$GUIDE_PATH" 'Prefer `mcp__so-context__so_read`'
assert_contains "$GUIDE_PATH" 'Prefer `mcp__so-context__so_search`'
assert_contains "$GUIDE_PATH" 'Prefer `mcp__so-context__so_shell`'

assert_contains "$CONFIG_PATH" "[mcp_servers.so-context]"
assert_contains "$CONFIG_PATH" "command = \"$SO_CONTEXT_BIN\""
assert_contains "$CONFIG_PATH" 'matcher = "mcp__so-context__.*"'
assert_contains "$CONFIG_PATH" 'matcher = "Read"'
assert_contains "$CONFIG_PATH" 'matcher = "Grep"'
assert_contains "$CONFIG_PATH" 'matcher = "Bash"'
assert_contains "$CONFIG_PATH" 'statusMessage = "Native file read detected; routing to mcp__so-context__so_read"'
assert_contains "$CONFIG_PATH" 'statusMessage = "Native search detected; routing to mcp__so-context__so_search"'
assert_contains "$CONFIG_PATH" 'statusMessage = "Short shell command detected; routing to mcp__so-context__so_shell"'
assert_contains "$CONFIG_PATH" "command = \"$SO_CONTEXT_BIN hook post-compact\""

for index in 0 1 2 3 4 5 6 7 8 9 10 11 12; do
  state_key="[hooks.state.\"$CONFIG_PATH:pre_tool_use:${index}:0\"]"
  assert_contains "$CONFIG_PATH" "$state_key"
  assert_state_enabled "$state_key"
done

printf '%s\n' "check_codex_setup: OK"
