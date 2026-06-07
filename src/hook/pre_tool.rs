use anyhow::Result;
use serde_json::{Value, json};

use crate::shell::{
    NATIVE_SHELL_TOOL_NAMES, parse_simple_shell_command, rewrite_env_prefix, should_prefer_so_shell,
};

const SO_CONTEXT_TOOL_PREFIX: &str = "mcp__so-context__";
const PRE_TOOL_USE_EVENT: &str = "PreToolUse";
const NATIVE_READ_TOOL_NAMES: &[&str] = &["Read", "read", "View", "view", "read_file"];
const NATIVE_SEARCH_TOOL_NAMES: &[&str] = &[
    "Grep",
    "grep",
    "rg",
    "ripgrep",
    "SearchFiles",
    "search_files",
];
const SIMPLE_NATIVE_SEARCH_KEYS: &[&str] =
    &["query", "pattern", "path", "directory", "root", "limit"];

pub fn run_pre_tool_use_hook() -> Result<()> {
    let input: Value = serde_json::from_reader(std::io::stdin()).unwrap_or(Value::Null);

    if let Some(output) = run_pre_tool_use_hook_value(&input) {
        println!("{output}");
    }

    Ok(())
}

fn run_pre_tool_use_hook_value(input: &Value) -> Option<Value> {
    let tool_name = input
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if tool_name.starts_with(SO_CONTEXT_TOOL_PREFIX) {
        return inject_session_id(input);
    }

    if NATIVE_SHELL_TOOL_NAMES.contains(&tool_name) {
        return deny_native_shell_if_needed(input);
    }

    if NATIVE_READ_TOOL_NAMES.contains(&tool_name) {
        return deny_native_read_if_needed(input);
    }

    if NATIVE_SEARCH_TOOL_NAMES.contains(&tool_name) {
        return deny_native_search_if_needed(input);
    }

    None
}

fn inject_session_id(input: &Value) -> Option<Value> {
    let context_id = input
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            input
                .get("agent_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
        })?;

    let mut tool_input = input
        .get("tool_input")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    tool_input.insert(
        "_so_session_id".to_string(),
        Value::String(context_id.to_string()),
    );

    Some(json!({
        "hookSpecificOutput": {
            "hookEventName": PRE_TOOL_USE_EVENT,
            "permissionDecision": "allow",
            "updatedInput": tool_input,
        }
    }))
}

fn deny_native_shell_if_needed(input: &Value) -> Option<Value> {
    let command = if let Some(command) = input
        .pointer("/tool_input/command")
        .and_then(|value| value.as_str())
    {
        command
    } else if let Some(command) = input
        .pointer("/tool_input/cmd")
        .and_then(|value| value.as_str())
    {
        command
    } else {
        return None;
    };

    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    let argv = parse_simple_shell_command(command)?;
    let inspected_argv = rewrite_env_prefix(argv);
    if !should_prefer_so_shell(&inspected_argv) {
        return None;
    }

    let reason = format!(
        "I routed this short shell command through our context-aware `mcp__so-context__so_shell` tool to improve shared project context for the next steps. This is expected, not an error. Retry with `argv: {}`. Keep the native shell only for long-running, streaming, or interactive commands.",
        serde_json::to_string(&inspected_argv).unwrap_or_else(|_| "[]".to_string())
    );

    Some(json!({
        "hookSpecificOutput": {
            "hookEventName": PRE_TOOL_USE_EVENT,
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }))
}

fn deny_native_read_if_needed(input: &Value) -> Option<Value> {
    let path = input
        .pointer("/tool_input/path")
        .and_then(|value| value.as_str())
        .or_else(|| {
            input
                .pointer("/tool_input/file_path")
                .and_then(|value| value.as_str())
        })?
        .trim();

    if path.is_empty() {
        return None;
    }

    let retry = serde_json::json!({
        "path": path,
        "mode": "full",
    });
    let reason = format!(
        "I routed this native file read through `mcp__so-context__so_read` so the file content stays attributable and reusable in shared project context. This is expected, not an error. Retry with arguments: {}. Use `mode: \"outline\"` when you only need structure instead of full file text.",
        retry
    );

    Some(json!({
        "hookSpecificOutput": {
            "hookEventName": PRE_TOOL_USE_EVENT,
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }))
}

fn deny_native_search_if_needed(input: &Value) -> Option<Value> {
    let retry = preferred_so_search_args(input)?;

    let reason = format!(
        "I routed this native search through `mcp__so-context__so_search` so the hits stay attributable and reusable in shared project context. This is expected, not an error. Retry with arguments: {}. Keep native grep-style tools only when you need raw grep semantics or the project is not indexed.",
        Value::Object(retry)
    );

    Some(json!({
        "hookSpecificOutput": {
            "hookEventName": PRE_TOOL_USE_EVENT,
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }))
}

fn preferred_so_search_args(input: &Value) -> Option<serde_json::Map<String, Value>> {
    let tool_input = input.get("tool_input")?.as_object()?;
    let query = tool_input
        .get("query")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| {
            if !uses_only_allowed_search_keys(tool_input) || !is_simple_literal_search_query(value)
            {
                return None;
            }
            Some(value.to_string())
        })
        .or_else(|| {
            if tool_input.contains_key("regex") || tool_input.contains_key("regexp") {
                return None;
            }

            let pattern = tool_input
                .get("pattern")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            if !uses_only_allowed_search_keys(tool_input)
                || !is_simple_literal_search_query(pattern)
            {
                return None;
            }

            Some(pattern.to_string())
        })?;

    let path = tool_input
        .get("path")
        .and_then(|value| value.as_str())
        .or_else(|| tool_input.get("directory").and_then(|value| value.as_str()))
        .or_else(|| tool_input.get("root").and_then(|value| value.as_str()))
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let mut retry = serde_json::Map::new();
    retry.insert("query".to_string(), Value::String(query));
    if let Some(path) = path {
        retry.insert("path".to_string(), Value::String(path.to_string()));
    }
    if let Some(limit) = tool_input
        .get("limit")
        .and_then(|value| value.as_u64())
        .filter(|value| *value > 0)
    {
        retry.insert("limit".to_string(), Value::Number(limit.into()));
    }

    Some(retry)
}

fn uses_only_allowed_search_keys(tool_input: &serde_json::Map<String, Value>) -> bool {
    tool_input
        .keys()
        .all(|key| SIMPLE_NATIVE_SEARCH_KEYS.contains(&key.as_str()))
}

fn is_simple_literal_search_query(query: &str) -> bool {
    !query.is_empty()
        && !query.chars().any(|ch| {
            matches!(
                ch,
                '\\' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
            )
        })
}

#[cfg(test)]
mod tests {
    use super::run_pre_tool_use_hook_value;
    use serde_json::{Value, json};

    fn hook(input: Value) -> Option<Value> {
        run_pre_tool_use_hook_value(&input)
    }

    #[test]
    fn injects_session_id_for_so_context_tools() {
        let output = hook(json!({
            "tool_name": "mcp__so-context__so_read",
            "session_id": "session-123",
            "tool_input": { "path": "src/main.rs" }
        }))
        .expect("expected hook output");

        assert_eq!(
            output.pointer("/hookSpecificOutput/updatedInput/_so_session_id"),
            Some(&Value::String("session-123".into()))
        );
    }

    #[test]
    fn prefers_session_id_over_agent_id_for_so_context_tools() {
        let output = hook(json!({
            "tool_name": "mcp__so-context__so_read",
            "session_id": "session-123",
            "agent_id": "agent-456",
            "tool_input": { "path": "src/main.rs" }
        }))
        .expect("expected hook output");

        assert_eq!(
            output.pointer("/hookSpecificOutput/updatedInput/_so_session_id"),
            Some(&Value::String("session-123".into()))
        );
    }

    #[test]
    fn blocks_short_bash_commands_and_suggests_so_shell() {
        let output = hook(json!({
            "tool_name": "Bash",
            "tool_input": { "command": "git status" }
        }))
        .expect("expected deny output");

        assert_eq!(
            output.pointer("/hookSpecificOutput/permissionDecision"),
            Some(&Value::String("deny".into()))
        );
        let reason = output
            .pointer("/hookSpecificOutput/permissionDecisionReason")
            .and_then(|value| value.as_str())
            .unwrap();
        assert!(reason.contains("mcp__so-context__so_shell"));
        assert!(reason.contains("improve shared project context"));
        assert!(reason.contains("[\"git\",\"status\"]"));
    }

    #[test]
    fn blocks_short_exec_command_aliases_too() {
        let output = hook(json!({
            "tool_name": "exec_command",
            "tool_input": { "cmd": "pwd" }
        }))
        .expect("expected deny output");

        assert_eq!(
            output.pointer("/hookSpecificOutput/permissionDecision"),
            Some(&Value::String("deny".into()))
        );
        let reason = output
            .pointer("/hookSpecificOutput/permissionDecisionReason")
            .and_then(|value| value.as_str())
            .unwrap();
        assert!(reason.contains("[\"pwd\"]"));
    }

    #[test]
    fn blocks_env_prefixed_commands() {
        let output = hook(json!({
            "tool_name": "Bash",
            "tool_input": { "command": "FOO=bar git status" }
        }))
        .expect("expected deny output");

        let reason = output
            .pointer("/hookSpecificOutput/permissionDecisionReason")
            .and_then(|value| value.as_str())
            .unwrap();
        assert!(reason.contains("[\"env\",\"FOO=bar\",\"git\",\"status\"]"));
    }

    #[test]
    fn blocks_native_read_and_suggests_so_read() {
        let output = hook(json!({
            "tool_name": "Read",
            "tool_input": { "path": "/tmp/example.rs" }
        }))
        .expect("expected deny output");

        assert_eq!(
            output.pointer("/hookSpecificOutput/permissionDecision"),
            Some(&Value::String("deny".into()))
        );
        let reason = output
            .pointer("/hookSpecificOutput/permissionDecisionReason")
            .and_then(|value| value.as_str())
            .unwrap();
        assert!(reason.contains("mcp__so-context__so_read"));
        assert!(reason.contains("\"path\":\"/tmp/example.rs\""));
        assert!(reason.contains("\"mode\":\"full\""));
    }

    #[test]
    fn blocks_native_search_and_suggests_so_search() {
        let output = hook(json!({
            "tool_name": "Grep",
            "tool_input": {
                "pattern": "ConfigRepository",
                "path": "/tmp/project"
            }
        }))
        .expect("expected deny output");

        assert_eq!(
            output.pointer("/hookSpecificOutput/permissionDecision"),
            Some(&Value::String("deny".into()))
        );
        let reason = output
            .pointer("/hookSpecificOutput/permissionDecisionReason")
            .and_then(|value| value.as_str())
            .unwrap();
        assert!(reason.contains("mcp__so-context__so_search"));
        assert!(reason.contains("\"query\":\"ConfigRepository\""));
        assert!(reason.contains("\"path\":\"/tmp/project\""));
    }

    #[test]
    fn allows_regex_style_native_search_to_pass_through() {
        let output = hook(json!({
            "tool_name": "Grep",
            "tool_input": {
                "pattern": "ConfigRepository|ConfigStore",
                "path": "/tmp/project"
            }
        }));

        assert!(output.is_none());
    }

    #[test]
    fn allows_native_search_with_extra_semantics_to_pass_through() {
        let output = hook(json!({
            "tool_name": "SearchFiles",
            "tool_input": {
                "query": "ConfigRepository",
                "path": "/tmp/project",
                "case_sensitive": true
            }
        }));

        assert!(output.is_none());
    }

    #[test]
    fn allows_regex_style_query_native_search_to_pass_through() {
        let output = hook(json!({
            "tool_name": "SearchFiles",
            "tool_input": {
                "query": "ConfigRepository|ConfigStore",
                "path": "/tmp/project"
            }
        }));

        assert!(output.is_none());
    }

    #[test]
    fn allows_long_running_tail_follow() {
        let output = hook(json!({
            "tool_name": "Bash",
            "tool_input": { "command": "tail -f log.txt" }
        }));
        assert!(output.is_none());
    }

    #[test]
    fn allows_complex_shell_syntax_to_pass_through() {
        let output = hook(json!({
            "tool_name": "Bash",
            "tool_input": { "command": "git status | head" }
        }));
        assert!(output.is_none());
    }

    #[test]
    fn allows_cargo_run_to_pass_through() {
        let output = hook(json!({
            "tool_name": "Bash",
            "tool_input": { "command": "cargo run" }
        }));
        assert!(output.is_none());
    }
}
