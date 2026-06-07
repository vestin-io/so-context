#!/usr/bin/env sh
set -eu

SO_CONTEXT_BIN="${SO_CONTEXT_BIN:-$(cd "$(dirname "$0")/.." && pwd)/target/debug/so-context}"

fail() {
  printf '%s\n' "test_pre_tool_hook: $*" >&2
  exit 1
}

assert_contains() {
  haystack="$1"
  needle="$2"
  case "$haystack" in
    *"$needle"*) ;;
    *) fail "expected output to contain: $needle" ;;
  esac
}

run_hook() {
  payload="$1"
  printf '%s' "$payload" | "$SO_CONTEXT_BIN" hook pre-tool
}

session_output="$(run_hook '{
  "tool_name": "mcp__so-context__so_read",
  "session_id": "session-123",
  "agent_id": "agent-456",
  "tool_input": { "path": "src/main.rs" }
}')"
assert_contains "$session_output" '"_so_session_id":"session-123"'

read_output="$(run_hook '{
  "tool_name": "Read",
  "tool_input": { "path": "/tmp/example.rs" }
}')"
assert_contains "$read_output" '"permissionDecision":"deny"'
assert_contains "$read_output" 'mcp__so-context__so_read'
assert_contains "$read_output" '{\"mode\":\"full\"'
assert_contains "$read_output" '\"path\":\"/tmp/example.rs\"'

search_output="$(run_hook '{
  "tool_name": "Grep",
  "tool_input": {
    "pattern": "ConfigRepository",
    "path": "/tmp/project",
    "limit": 15
  }
}')"
assert_contains "$search_output" 'mcp__so-context__so_search'
assert_contains "$search_output" '\"query\":\"ConfigRepository\"'
assert_contains "$search_output" '\"limit\":15'
assert_contains "$search_output" '\"path\":\"/tmp/project\"'

regex_output="$(run_hook '{
  "tool_name": "SearchFiles",
  "tool_input": {
    "query": "ConfigRepository|ConfigStore",
    "path": "/tmp/project"
  }
}')"
[ -z "$regex_output" ] || fail "regex-style native search should pass through without hook output"

shell_output="$(run_hook '{
  "tool_name": "Bash",
  "tool_input": { "command": "git status" }
}')"
assert_contains "$shell_output" 'mcp__so-context__so_shell'
assert_contains "$shell_output" '[\"git\",\"status\"]'

printf '%s\n' "test_pre_tool_hook: OK"
