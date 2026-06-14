use std::path::Path;

use serde_json::{Map, Value};

#[derive(Debug, Clone, Default)]
pub struct ShellRedirectContext {
    pub client: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

pub fn rewrite_native_shell_tool_input(
    tool_input: &Map<String, Value>,
    context: &ShellRedirectContext,
) -> Option<Map<String, Value>> {
    let (command_key, command) = native_command_field(tool_input)?;
    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    let argv = super::parse_simple_shell_command(command)?;
    let inspected_argv = super::rewrite_env_prefix(argv);
    if is_already_so_context_shell_command(&inspected_argv) {
        return None;
    }
    if !super::should_prefer_so_shell(&inspected_argv) {
        return None;
    }

    let rewritten_command = build_so_context_shell_command(command, context)?;
    let mut rewritten_input = tool_input.clone();
    rewritten_input.insert(command_key.to_string(), Value::String(rewritten_command));
    Some(rewritten_input)
}

fn native_command_field(tool_input: &Map<String, Value>) -> Option<(&'static str, &str)> {
    tool_input
        .get("command")
        .and_then(Value::as_str)
        .map(|value| ("command", value))
        .or_else(|| {
            tool_input
                .get("cmd")
                .and_then(Value::as_str)
                .map(|value| ("cmd", value))
        })
}

fn build_so_context_shell_command(command: &str, context: &ShellRedirectContext) -> Option<String> {
    let binary = std::env::current_exe().ok()?;
    let mut parts = Vec::with_capacity(9);
    if let Some(client) = context.client.as_deref() {
        parts.push(format!("SO_CONTEXT_CLIENT={}", shell_quote(client)));
    }
    if let Some(session_id) = context.session_id.as_deref() {
        parts.push(format!("SO_CONTEXT_SESSION_ID={}", shell_quote(session_id)));
    }
    if let Some(agent_id) = context.agent_id.as_deref() {
        parts.push(format!("SO_CONTEXT_AGENT={}", shell_quote(agent_id)));
    }
    parts.push("SO_CONTEXT_SESSION_SOURCE=hook".to_string());
    parts.push(shell_quote(binary.to_string_lossy().as_ref()));
    parts.push("shell".to_string());
    parts.push("-c".to_string());
    parts.push(shell_quote(command));
    Some(parts.join(" "))
}

fn is_already_so_context_shell_command(argv: &[String]) -> bool {
    let Some((program, subcommand)) = program_and_subcommand(argv) else {
        return false;
    };
    if subcommand != "shell" {
        return false;
    }

    let program_name = base_program_name(program);
    if program_name == "so-context" {
        return true;
    }

    std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(|name| name == program_name)
        .unwrap_or(false)
}

fn program_and_subcommand(argv: &[String]) -> Option<(&str, &str)> {
    let program_index = if argv.first().map(String::as_str) == Some("env") {
        argv.iter()
            .skip(1)
            .take_while(|arg| is_env_assignment(arg))
            .count()
            + 1
    } else {
        0
    };

    Some((
        argv.get(program_index)?.as_str(),
        argv.get(program_index + 1)?.as_str(),
    ))
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

fn base_program_name(program: &str) -> &str {
    Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
}

fn shell_quote(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if arg
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '='))
    {
        return arg.to_string();
    }

    format!("'{}'", arg.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{ShellRedirectContext, rewrite_native_shell_tool_input};

    #[test]
    fn rewrites_command_field_through_shell_cli() {
        let input = json!({ "command": "git status", "timeout_ms": 1000 })
            .as_object()
            .cloned()
            .unwrap();

        let rewritten = rewrite_native_shell_tool_input(
            &input,
            &ShellRedirectContext {
                client: Some("codex".into()),
                session_id: Some("session-123".into()),
                ..ShellRedirectContext::default()
            },
        )
        .expect("expected rewritten input");

        let command = rewritten
            .get("command")
            .and_then(Value::as_str)
            .expect("command string");
        assert!(command.contains("SO_CONTEXT_CLIENT=codex"));
        assert!(command.contains("SO_CONTEXT_SESSION_ID=session-123"));
        assert!(command.contains(" shell -c "));
        assert!(command.ends_with("'git status'"));
        assert_eq!(rewritten.get("timeout_ms"), input.get("timeout_ms"));
    }

    #[test]
    fn rewrites_cmd_field_without_losing_other_args() {
        let input = json!({ "cmd": "pwd", "tty": true })
            .as_object()
            .cloned()
            .unwrap();

        let rewritten = rewrite_native_shell_tool_input(&input, &ShellRedirectContext::default())
            .expect("expected rewritten input");

        assert!(rewritten.get("cmd").and_then(Value::as_str).is_some());
        assert_eq!(rewritten.get("tty"), input.get("tty"));
    }

    #[test]
    fn skips_commands_that_should_stay_native() {
        let input = json!({ "command": "tail -f log.txt" })
            .as_object()
            .cloned()
            .unwrap();

        assert!(
            rewrite_native_shell_tool_input(&input, &ShellRedirectContext::default()).is_none()
        );
    }

    #[test]
    fn skips_commands_already_rewritten_with_env_prefix() {
        let current_binary = std::env::current_exe()
            .expect("current exe")
            .to_string_lossy()
            .into_owned();
        let input = json!({
            "command": format!(
                "SO_CONTEXT_CLIENT=opencode SO_CONTEXT_SESSION_ID=session-123 SO_CONTEXT_SESSION_SOURCE=hook {} shell -c 'git status'",
                current_binary
            )
        })
        .as_object()
        .cloned()
        .unwrap();

        assert!(
            rewrite_native_shell_tool_input(&input, &ShellRedirectContext::default()).is_none()
        );
    }
}
