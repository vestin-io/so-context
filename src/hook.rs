//! `so-context hook` — PreToolUse/PostCompact hook handlers for agent CLIs.
//!
//! PreToolUse currently serves two purposes:
//! - inject `_so_session_id` into so-context MCP tool calls
//! - block selected native shell commands and direct the agent to retry with
//!   `mcp__so-context__so_shell`

use anyhow::Result;
use serde_json::{Value, json};

use crate::shell::{parse_simple_shell_command, rewrite_env_prefix};

const SO_CONTEXT_TOOL_PREFIX: &str = "mcp__so-context__";
const SHELL_TOOL_NAMES: &[&str] = &[
    "Bash",
    "bash",
    "Shell",
    "shell",
    "runTerminalCommand",
    "runInTerminal",
    "run_in_terminal",
    "terminal",
    "shell_command",
    "exec_command",
    "local_shell",
    "run_shell_command",
];
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

    if SHELL_TOOL_NAMES.contains(&tool_name) {
        return deny_native_shell_if_needed(input);
    }

    None
}

fn inject_session_id(input: &Value) -> Option<Value> {
    // Claude Code: agent_id is present when firing inside a sub-agent and
    // uniquely identifies that context window. Fall back to session_id for
    // the main thread or for Codex (which only provides session_id).
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
        "Prefer `mcp__so-context__so_shell` for this short shell command. Retry with `argv: {}`. Keep the native shell only for long-running, streaming, or interactive commands.",
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

fn is_env_assignment(arg: &str) -> bool {
    let Some((name, _value)) = arg.split_once('=') else {
        return false;
    };
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn should_prefer_so_shell(argv: &[String]) -> bool {
    let program_index = program_index(argv);

    let Some(program) = argv.get(program_index).map(|s| s.as_str()) else {
        return false;
    };
    let args = &argv[program_index + 1..];

    !should_keep_native_shell(base_program_name(program), args)
}

fn program_index(argv: &[String]) -> usize {
    if argv.first().map(|s| s.as_str()) == Some("env") {
        argv.iter()
            .skip(1)
            .take_while(|arg| is_env_assignment(arg))
            .count()
            + 1
    } else {
        0
    }
}

fn base_program_name(program: &str) -> &str {
    std::path::Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
}

fn should_keep_native_shell(program: &str, args: &[String]) -> bool {
    match program {
        "ssh" | "scp" | "sftp" | "mosh" | "top" | "htop" | "less" | "more" | "man" | "vim"
        | "nvim" | "nano" | "tmux" | "screen" | "watch" => true,
        "tail" => has_any_flag(args, &["-f", "--follow"]),
        "docker" => !should_prefer_docker(args),
        "docker-compose" => !should_prefer_docker_compose(args),
        "kubectl" => !should_prefer_kubectl(args),
        "cargo" => should_keep_native_cargo(args),
        "npm" | "pnpm" | "yarn" | "bun" | "npx" => should_keep_native_js_runner(args),
        "python" | "python3" => should_keep_native_python(args),
        "node" => should_keep_native_node(args),
        _ => false,
    }
}

fn should_prefer_docker(args: &[String]) -> bool {
    let Some(subcommand) = first_non_flag(args) else {
        return true;
    };

    match subcommand {
        "run" | "exec" | "attach" => false,
        "logs" => !has_any_flag(args, &["-f", "--follow"]),
        "compose" => should_prefer_docker_compose(after_subcommand(args, "compose")),
        _ => true,
    }
}

fn should_prefer_docker_compose(args: &[String]) -> bool {
    let Some(subcommand) = first_non_flag(args) else {
        return true;
    };

    match subcommand {
        "up" | "exec" | "run" | "attach" | "watch" => false,
        "logs" => !has_any_flag(args, &["-f", "--follow"]),
        _ => true,
    }
}

fn should_prefer_kubectl(args: &[String]) -> bool {
    let Some(subcommand) = first_non_flag(args) else {
        return true;
    };

    match subcommand {
        "exec" | "attach" | "port-forward" => false,
        "logs" => !has_any_flag(args, &["-f", "--follow"]),
        _ => true,
    }
}

fn should_keep_native_cargo(args: &[String]) -> bool {
    matches!(first_non_flag(args), Some("run" | "watch"))
}

fn should_keep_native_js_runner(args: &[String]) -> bool {
    let Some(subcommand) = first_non_flag(args) else {
        return false;
    };

    match subcommand {
        "dev" | "start" | "serve" | "watch" | "create" => true,
        "run" => matches!(
            first_non_flag(after_subcommand(args, "run")),
            Some("dev" | "start" | "serve" | "watch")
        ),
        _ => false,
    }
}

fn should_keep_native_python(args: &[String]) -> bool {
    if has_any_flag(args, &["-i"]) {
        return true;
    }

    match args.first().map(|arg| arg.as_str()) {
        Some("-m") => matches!(args.get(1).map(|arg| arg.as_str()), Some("http.server")),
        Some(script) if script.ends_with("manage.py") => {
            matches!(args.get(1).map(|arg| arg.as_str()), Some("runserver"))
        }
        _ => false,
    }
}

fn should_keep_native_node(args: &[String]) -> bool {
    has_any_flag(args, &["--watch"])
}

fn has_any_flag(args: &[String], flags: &[&str]) -> bool {
    args.iter().any(|arg| flags.iter().any(|flag| arg == flag))
}

fn first_non_flag(args: &[String]) -> Option<&str> {
    args.iter()
        .find(|arg| !arg.starts_with('-'))
        .map(|s| s.as_str())
}

fn after_subcommand<'a>(args: &'a [String], subcommand: &str) -> &'a [String] {
    if let Some(index) = args.iter().position(|arg| arg == subcommand) {
        &args[index + 1..]
    } else {
        &[]
    }
}

/// PostCompact hook handler.
///
/// Reads the Claude Code PostCompact JSON payload from stdin and sends a
/// `compact_reset` ctrl notification to the daemon so it clears file-visit
/// cache entries for the compacted session. This ensures the agent receives
/// full file content again rather than "use cached context" stubs.
///
/// Expected payload fields:
///   - `session_id`     — identifies the Claude Code session
///   - `connection_id`  — identifies the MCP connection within the session
///     (Claude Code injects this; fall back to session_id if absent)
#[tokio::main]
pub async fn run_post_compact_hook() -> Result<()> {
    let input: Value = serde_json::from_reader(std::io::stdin()).unwrap_or(Value::Null);

    let session_id = input
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    let connection_id = input
        .get("connection_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or(session_id); // fallback: use session_id as connection_id

    let (cid, sid) = match (connection_id, session_id) {
        (Some(c), Some(s)) => (c, s),
        _ => {
            // Missing IDs — nothing useful to reset, exit silently.
            return Ok(());
        }
    };

    if let Err(e) = crate::mcp::send_compact_reset(cid, sid).await {
        eprintln!("so-context compact hook: {e}");
    }

    Ok(())
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
