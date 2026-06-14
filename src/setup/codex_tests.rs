use super::{
    install_post_compact_hook, install_pre_tool_use_hook, remove_post_compact_hook,
    remove_pre_tool_use_hook,
};
use toml_edit::DocumentMut;

#[test]
fn remove_pre_tool_use_hook_keeps_user_hooks_in_same_matcher_group() {
    let mut doc: DocumentMut = r#"
[hooks]
[[hooks.PreToolUse]]
matcher = "Bash"
[[hooks.PreToolUse.hooks]]
type = "command"
command = "/tmp/so-context hook pre-tool"
[[hooks.PreToolUse.hooks]]
type = "command"
command = "/usr/local/bin/custom-hook"
"#
    .parse()
    .unwrap();

    remove_pre_tool_use_hook(&mut doc, "/tmp/so-context");

    let hooks = doc["hooks"]["PreToolUse"][0]["hooks"]
        .as_array_of_tables()
        .unwrap();
    let hook = hooks.iter().next().unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(
        hook.get("command").and_then(|v| v.as_str()),
        Some("/usr/local/bin/custom-hook")
    );
}

#[test]
fn install_pre_tool_use_hook_replaces_only_so_context_entry() {
    let mut doc: DocumentMut = r#"
[hooks]
[[hooks.PreToolUse]]
matcher = "Bash"
[[hooks.PreToolUse.hooks]]
type = "command"
command = "/tmp/so-context hook pre-tool"
[[hooks.PreToolUse.hooks]]
type = "command"
command = "/usr/local/bin/custom-hook"
"#
    .parse()
    .unwrap();

    install_pre_tool_use_hook(&mut doc, "/opt/bin/so-context");

    let hooks = doc["hooks"]["PreToolUse"][0]["hooks"]
        .as_array_of_tables()
        .unwrap();
    assert_eq!(hooks.len(), 2);
    assert!(hooks.iter().any(|hook| {
        hook.get("command").and_then(|v| v.as_str()) == Some("/usr/local/bin/custom-hook")
    }));
    assert!(hooks.iter().any(|hook| {
        hook.get("command").and_then(|v| v.as_str())
            == Some("/opt/bin/so-context hook pre-tool --host codex")
    }));
}

#[test]
fn install_pre_tool_use_hook_adds_native_shell_matcher_group() {
    let mut doc: DocumentMut = r#"
[hooks]
"#
    .parse()
    .unwrap();

    install_pre_tool_use_hook(&mut doc, "/opt/bin/so-context");

    let groups = doc["hooks"]["PreToolUse"].as_array_of_tables().unwrap();
    let shell_group = groups
        .iter()
        .find(|group| group.get("matcher").and_then(|v| v.as_str()) == Some("Bash"))
        .expect("expected Bash matcher group");
    let hooks = shell_group
        .get("hooks")
        .and_then(|v| v.as_array_of_tables())
        .unwrap();
    let hook = hooks.iter().next().unwrap();
    assert_eq!(
        hook.get("statusMessage").and_then(|v| v.as_str()),
        Some("Short shell command detected; routing through so-context shell CLI")
    );
}

#[test]
fn install_pre_tool_use_hook_uses_regex_matcher_for_self_tools() {
    let mut doc: DocumentMut = r#"
[hooks]
"#
    .parse()
    .unwrap();

    install_pre_tool_use_hook(&mut doc, "/opt/bin/so-context");

    let groups = doc["hooks"]["PreToolUse"].as_array_of_tables().unwrap();
    assert!(groups.iter().any(|group| {
        group.get("matcher").and_then(|v| v.as_str()) == Some("mcp__so-context__.*")
    }));
}

#[test]
fn remove_post_compact_hook_keeps_other_hooks() {
    let mut doc: DocumentMut = r#"
[hooks]
[[hooks.PostCompact]]
[[hooks.PostCompact.hooks]]
type = "command"
command = "/tmp/so-context hook post-compact"
[[hooks.PostCompact.hooks]]
type = "command"
command = "/usr/local/bin/custom-post"
"#
    .parse()
    .unwrap();

    remove_post_compact_hook(&mut doc, "/tmp/so-context");

    let hooks = doc["hooks"]["PostCompact"][0]["hooks"]
        .as_array_of_tables()
        .unwrap();
    let hook = hooks.iter().next().unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(
        hook.get("command").and_then(|v| v.as_str()),
        Some("/usr/local/bin/custom-post")
    );
}

#[test]
fn install_post_compact_hook_replaces_only_so_context_entry() {
    let mut doc: DocumentMut = r#"
[hooks]
[[hooks.PostCompact]]
[[hooks.PostCompact.hooks]]
type = "command"
command = "/tmp/so-context hook post-compact"
[[hooks.PostCompact.hooks]]
type = "command"
command = "/usr/local/bin/custom-post"
"#
    .parse()
    .unwrap();

    install_post_compact_hook(&mut doc, "/opt/bin/so-context");

    let hooks = doc["hooks"]["PostCompact"][0]["hooks"]
        .as_array_of_tables()
        .unwrap();
    assert_eq!(hooks.len(), 2);
    assert!(hooks.iter().any(|hook| {
        hook.get("command").and_then(|v| v.as_str()) == Some("/usr/local/bin/custom-post")
    }));
    assert!(hooks.iter().any(|hook| {
        hook.get("command").and_then(|v| v.as_str())
            == Some("/opt/bin/so-context hook post-compact --host codex")
    }));
}

#[test]
fn remove_pre_tool_use_hook_supports_host_tagged_commands() {
    let mut doc: DocumentMut = r#"
[hooks]
[[hooks.PreToolUse]]
matcher = "Bash"
[[hooks.PreToolUse.hooks]]
type = "command"
command = "/tmp/so-context hook pre-tool --host codex"
[[hooks.PreToolUse.hooks]]
type = "command"
command = "/usr/local/bin/custom-hook"
"#
    .parse()
    .unwrap();

    remove_pre_tool_use_hook(&mut doc, "/tmp/so-context");

    let hooks = doc["hooks"]["PreToolUse"][0]["hooks"]
        .as_array_of_tables()
        .unwrap();
    let hook = hooks.iter().next().unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(
        hook.get("command").and_then(|v| v.as_str()),
        Some("/usr/local/bin/custom-hook")
    );
}

#[test]
fn remove_pre_tool_use_hook_supports_custom_binary_names() {
    let mut doc: DocumentMut = r#"
[hooks]
[[hooks.PreToolUse]]
matcher = "Bash"
[[hooks.PreToolUse.hooks]]
type = "command"
command = "/tmp/ctx-custom hook pre-tool --host codex"
[[hooks.PreToolUse.hooks]]
type = "command"
command = "/usr/local/bin/custom-hook"
"#
    .parse()
    .unwrap();

    remove_pre_tool_use_hook(&mut doc, "/tmp/ctx-custom");

    let hooks = doc["hooks"]["PreToolUse"][0]["hooks"]
        .as_array_of_tables()
        .unwrap();
    let hook = hooks.iter().next().unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(
        hook.get("command").and_then(|v| v.as_str()),
        Some("/usr/local/bin/custom-hook")
    );
}

#[test]
fn remove_post_compact_hook_supports_custom_binary_names() {
    let mut doc: DocumentMut = r#"
[hooks]
[[hooks.PostCompact]]
[[hooks.PostCompact.hooks]]
type = "command"
command = "/tmp/ctx-custom hook post-compact --host codex"
[[hooks.PostCompact.hooks]]
type = "command"
command = "/usr/local/bin/custom-post"
"#
    .parse()
    .unwrap();

    remove_post_compact_hook(&mut doc, "/tmp/ctx-custom");

    let hooks = doc["hooks"]["PostCompact"][0]["hooks"]
        .as_array_of_tables()
        .unwrap();
    let hook = hooks.iter().next().unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(
        hook.get("command").and_then(|v| v.as_str()),
        Some("/usr/local/bin/custom-post")
    );
}
