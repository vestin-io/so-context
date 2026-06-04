use anyhow::Result;
use serde_json::{Value, json};

use crate::shell::{
    NATIVE_SHELL_TOOL_NAMES, parse_simple_shell_command, rewrite_env_prefix, should_prefer_so_shell,
};

const SO_CONTEXT_TOOL_PREFIX: &str = "mcp__so-context__";
const PRE_TOOL_USE_EVENT: &str = "PreToolUse";

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

    None
}

fn inject_session_id(input: &Value) -> Option<Value> {
    let context_id = input
        .get("agent_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            input
                .get("session_id")
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
        "This short shell command was automatically routed to `mcp__so-context__so_shell`. This is expected, not an error. Retry with `argv: {}`. Keep the native shell only for long-running, streaming, or interactive commands.",
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
