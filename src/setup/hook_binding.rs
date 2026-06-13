use std::path::Path;

use serde_json::{Value, json};

use crate::host_adapter::HostKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookEvent {
    PreTool,
    PostCompact,
}

impl HookEvent {
    pub fn cli_name(self) -> &'static str {
        match self {
            Self::PreTool => "pre-tool",
            Self::PostCompact => "post-compact",
        }
    }
}

pub fn json_hook_args(event: HookEvent, host_kind: HostKind) -> Value {
    json!(["hook", event.cli_name(), "--host", host_kind.as_str()])
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn json_hook_value_invokes_event(hook: &Value, event: HookEvent) -> bool {
    let command_matches = hook
        .get("command")
        .and_then(|c| c.as_str())
        .map(command_points_to_so_context)
        .unwrap_or(false);

    command_matches && json_hook_args_match_event(hook, event)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn json_hook_value_matches_binary(hook: &Value, binary: &str, event: HookEvent) -> bool {
    hook.get("command").and_then(|c| c.as_str()) == Some(binary)
        && json_hook_args_match_event(hook, event)
}

pub fn json_hook_value_matches_binary_or_legacy(
    hook: &Value,
    binary: &str,
    event: HookEvent,
) -> bool {
    let command_matches = hook
        .get("command")
        .and_then(|c| c.as_str())
        .map(|command| command == binary || command_points_to_so_context(command))
        .unwrap_or(false);

    command_matches && json_hook_args_match_event(hook, event)
}

pub fn toml_hook_command(binary: &str, event: HookEvent, host_kind: HostKind) -> String {
    format!(
        "{binary} hook {} --host {}",
        event.cli_name(),
        host_kind.as_str()
    )
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn toml_hook_command_invokes_event(command: &str, event: HookEvent) -> bool {
    toml_hook_command_matches(command, event, command_points_to_so_context)
}

pub fn toml_hook_command_matches_binary_or_legacy(
    command: &str,
    binary: &str,
    event: HookEvent,
) -> bool {
    toml_hook_command_matches(command, event, |hook_binary| {
        hook_binary == binary || command_points_to_so_context(hook_binary)
    })
}

fn json_hook_args_match_event(hook: &Value, event: HookEvent) -> bool {
    hook.get("args")
        .and_then(|a| a.as_array())
        .map(|args| {
            args.first().and_then(|v| v.as_str()) == Some("hook")
                && args.get(1).and_then(|v| v.as_str()) == Some(event.cli_name())
        })
        .unwrap_or(false)
}

fn toml_hook_command_matches<F>(command: &str, event: HookEvent, binary_matches: F) -> bool
where
    F: FnOnce(&str) -> bool,
{
    let marker = format!(" hook {}", event.cli_name());
    let Some((binary, tail)) = command.split_once(&marker) else {
        return false;
    };
    let extra_args = tail.trim();
    let host_args_ok = extra_args.is_empty() || extra_args.starts_with("--host ");

    host_args_ok && binary_matches(binary.trim_end())
}

fn command_points_to_so_context(command: &str) -> bool {
    Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        == Some("so-context")
}

#[cfg(test)]
#[path = "hook_binding_tests.rs"]
mod tests;
