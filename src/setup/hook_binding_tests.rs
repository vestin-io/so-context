use serde_json::json;

use super::{
    HookEvent, json_hook_args, json_hook_value_invokes_event, json_hook_value_matches_binary,
    json_hook_value_matches_binary_or_legacy, toml_hook_command, toml_hook_command_invokes_event,
    toml_hook_command_matches_binary_or_legacy,
};
use crate::host_adapter::HostKind;

#[test]
fn json_hook_args_include_host_flag() {
    assert_eq!(
        json_hook_args(HookEvent::PreTool, HostKind::Claude),
        json!(["hook", "pre-tool", "--host", "claude"])
    );
}

#[test]
fn json_hook_value_matches_legacy_and_host_tagged_shapes() {
    assert!(json_hook_value_invokes_event(
        &json!({
            "command": "/tmp/so-context",
            "args": ["hook", "pre-tool"]
        }),
        HookEvent::PreTool
    ));

    assert!(json_hook_value_invokes_event(
        &json!({
            "command": "/tmp/so-context",
            "args": ["hook", "pre-tool", "--host", "claude"]
        }),
        HookEvent::PreTool
    ));
}

#[test]
fn json_hook_value_matches_binary_for_exact_command() {
    assert!(json_hook_value_matches_binary(
        &json!({
            "command": "/tmp/so-context",
            "args": ["hook", "post-compact", "--host", "claude"]
        }),
        "/tmp/so-context",
        HookEvent::PostCompact
    ));
}

#[test]
fn json_hook_value_matches_custom_binary_when_explicitly_requested() {
    assert!(json_hook_value_matches_binary_or_legacy(
        &json!({
            "command": "/tmp/ctx-custom",
            "args": ["hook", "post-compact", "--host", "claude"]
        }),
        "/tmp/ctx-custom",
        HookEvent::PostCompact
    ));
}

#[test]
fn toml_hook_command_supports_tagged_and_legacy_detection() {
    let command = toml_hook_command(
        "/opt/bin/so-context",
        HookEvent::PostCompact,
        HostKind::Codex,
    );
    assert!(toml_hook_command_invokes_event(
        &command,
        HookEvent::PostCompact
    ));
    assert!(toml_hook_command_invokes_event(
        "/tmp/so-context hook post-compact",
        HookEvent::PostCompact
    ));
}

#[test]
fn toml_hook_command_matches_custom_binary_when_explicitly_requested() {
    assert!(toml_hook_command_matches_binary_or_legacy(
        "/tmp/ctx-custom hook pre-tool --host codex",
        "/tmp/ctx-custom",
        HookEvent::PreTool
    ));
}
