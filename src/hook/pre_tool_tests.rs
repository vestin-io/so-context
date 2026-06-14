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
fn rewrites_short_bash_commands_through_shell_cli() {
    let output = hook(json!({
        "tool_name": "Bash",
        "tool_input": { "command": "git status" }
    }))
    .expect("expected rewrite output");

    let rewritten = output
        .pointer("/hookSpecificOutput/updatedInput/command")
        .and_then(|value| value.as_str())
        .unwrap();
    assert!(rewritten.contains("SO_CONTEXT_SESSION_SOURCE=hook"));
    assert!(rewritten.contains(" shell -c "));
    assert!(rewritten.ends_with("'git status'"));
}

#[test]
fn rewrites_short_exec_command_aliases_too() {
    let output = hook(json!({
        "tool_name": "exec_command",
        "tool_input": { "cmd": "pwd" }
    }))
    .expect("expected rewrite output");

    let rewritten = output
        .pointer("/hookSpecificOutput/updatedInput/cmd")
        .and_then(|value| value.as_str())
        .unwrap();
    assert!(rewritten.contains(" shell -c "));
    assert!(rewritten.ends_with("pwd"));
}

#[test]
fn rewrites_env_prefixed_commands() {
    let output = hook(json!({
        "tool_name": "Bash",
        "tool_input": { "command": "FOO=bar git status" }
    }))
    .expect("expected rewrite output");

    let rewritten = output
        .pointer("/hookSpecificOutput/updatedInput/command")
        .and_then(|value| value.as_str())
        .unwrap();
    assert!(rewritten.ends_with("'FOO=bar git status'"));
}

#[test]
fn passes_native_read_through() {
    let output = hook(json!({
        "tool_name": "Read",
        "tool_input": { "path": "/tmp/example.rs" }
    }));

    assert!(output.is_none());
}

#[test]
fn passes_native_search_through() {
    let output = hook(json!({
        "tool_name": "Grep",
        "tool_input": {
            "pattern": "ConfigRepository",
            "path": "/tmp/project"
        }
    }));

    assert!(output.is_none());
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
fn opencode_host_rewrites_shell_with_host_identity() {
    let output = hook_for_host(
        HostKind::OpenCode,
        json!({
            "tool_name": "exec_command",
            "session": { "id": "session-nested" },
            "tool_input": { "cmd": "pwd", "tty": false }
        }),
    )
    .expect("expected rewrite output");

    let rewritten = output
        .pointer("/hookSpecificOutput/updatedInput/cmd")
        .and_then(|value| value.as_str())
        .unwrap();
    assert!(rewritten.contains("SO_CONTEXT_CLIENT=opencode"));
    assert!(rewritten.contains("SO_CONTEXT_SESSION_ID=session-nested"));
}

#[test]
fn opencode_host_shapes_so_shell_input_via_shared_patch_contract() {
    let output = hook_for_host(
        HostKind::OpenCode,
        json!({
            "tool_name": "so-context_so_shell",
            "session": { "id": "session-nested" },
            "tool_input": {
                "argv": ["git", "-C", "my repo", "status"],
                "cwd": "/tmp/project",
                "full": false,
                "full_reason": "tee_missing_or_expired"
            }
        }),
    )
    .expect("expected patch output");

    assert_eq!(
        output.pointer("/hookSpecificOutput/updatedInput/command"),
        Some(&Value::String("git -C 'my repo' status".into()))
    );
    assert_eq!(
        output.pointer("/hookSpecificOutput/updatedInput/full_reason"),
        None
    );
    assert_eq!(
        output.pointer("/hookSpecificOutput/inputKeyOrder/0"),
        Some(&Value::String("command".into()))
    );
    assert_eq!(
        output.pointer("/hookSpecificOutput/inputKeyOrder/1"),
        Some(&Value::String("argv".into()))
    );
    assert_eq!(
        output.pointer("/hookSpecificOutput/inputKeyOrder/4"),
        Some(&Value::String("_so_session_id".into()))
    );
}
