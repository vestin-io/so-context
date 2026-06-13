use super::{run_pre_tool_use_hook_value, run_pre_tool_use_hook_value_for_test};
use crate::host_adapter::HostKind;
use serde_json::{Value, json};

fn hook(input: Value) -> Option<Value> {
    run_pre_tool_use_hook_value(&input)
}

fn hook_for_host(host_kind: HostKind, input: Value) -> Option<Value> {
    run_pre_tool_use_hook_value_for_test(host_kind, &input)
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
fn preserves_excerpt_arguments_for_native_read_reroute() {
    let output = hook(json!({
        "tool_name": "Read",
        "tool_input": {
            "path": "/tmp/example.rs",
            "start_line": 12,
            "end_line": 20,
            "line_numbers": true
        }
    }))
    .expect("expected deny output");

    let reason = output
        .pointer("/hookSpecificOutput/permissionDecisionReason")
        .and_then(|value| value.as_str())
        .unwrap();
    assert!(reason.contains("\"path\":\"/tmp/example.rs\""));
    assert!(reason.contains("\"start_line\":12"));
    assert!(reason.contains("\"end_line\":20"));
    assert!(reason.contains("\"line_numbers\":true"));
    assert!(!reason.contains("\"mode\":\"full\""));
}

#[test]
fn blocks_native_read_file_path_alias_and_suggests_so_read() {
    let output = hook(json!({
        "tool_name": "read_file",
        "tool_input": { "file_path": "/tmp/example.rs" }
    }))
    .expect("expected deny output");

    let reason = output
        .pointer("/hookSpecificOutput/permissionDecisionReason")
        .and_then(|value| value.as_str())
        .unwrap();
    assert!(reason.contains("mcp__so-context__so_read"));
    assert!(reason.contains("\"path\":\"/tmp/example.rs\""));
    assert!(!reason.contains("\"file_path\""));
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
fn blocks_native_search_directory_alias_and_suggests_so_search() {
    let output = hook(json!({
        "tool_name": "grep",
        "tool_input": {
            "pattern": "ConfigRepository",
            "directory": "/tmp/project"
        }
    }))
    .expect("expected deny output");

    let reason = output
        .pointer("/hookSpecificOutput/permissionDecisionReason")
        .and_then(|value| value.as_str())
        .unwrap();
    assert!(reason.contains("mcp__so-context__so_search"));
    assert!(reason.contains("\"query\":\"ConfigRepository\""));
    assert!(reason.contains("\"path\":\"/tmp/project\""));
    assert!(!reason.contains("\"directory\""));
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

#[test]
fn opencode_host_injects_session_id_for_flattened_self_tool_names() {
    let output = hook_for_host(
        HostKind::OpenCode,
        json!({
            "tool_name": "so-context_so_read",
            "session": { "id": "session-nested" },
            "tool_input": { "path": "src/main.rs" }
        }),
    )
    .expect("expected hook output");

    assert_eq!(
        output.pointer("/hookSpecificOutput/updatedInput/_so_session_id"),
        Some(&Value::String("session-nested".into()))
    );
}

#[test]
fn opencode_host_uses_flattened_retry_tool_names() {
    let output = hook_for_host(
        HostKind::OpenCode,
        json!({
            "tool_name": "Read",
            "tool_input": { "path": "/tmp/example.rs" }
        }),
    )
    .expect("expected deny output");

    let reason = output
        .pointer("/hookSpecificOutput/permissionDecisionReason")
        .and_then(|value| value.as_str())
        .unwrap();
    assert!(reason.contains("`so-context_so_read`"));
    assert!(!reason.contains("mcp__so-context__so_read"));
}
