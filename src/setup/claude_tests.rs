use super::{install_pre_tool_use_hook, remove_post_compact_hook, remove_pre_tool_use_hook};
use serde_json::json;

#[test]
fn remove_pre_tool_use_hook_keeps_user_hooks_in_same_matcher_group() {
    let mut root = json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "Bash",
                "hooks": [
                    { "type": "command", "command": "/tmp/so-context", "args": ["hook", "pre-tool"] },
                    { "type": "command", "command": "/usr/local/bin/custom-hook", "args": ["do", "thing"] }
                ]
            }]
        }
    });

    remove_pre_tool_use_hook(root.as_object_mut().unwrap(), "/tmp/so-context");

    let hooks = root["hooks"]["PreToolUse"][0]["hooks"].as_array().unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(
        hooks[0]["command"].as_str(),
        Some("/usr/local/bin/custom-hook")
    );
}

#[test]
fn install_pre_tool_use_hook_replaces_only_so_context_entry() {
    let mut root = json!({
        "hooks": {
            "PreToolUse": [{
                "matcher": "Bash",
                "hooks": [
                    { "type": "command", "command": "/tmp/so-context", "args": ["hook", "pre-tool"] },
                    { "type": "command", "command": "/usr/local/bin/custom-hook", "args": ["do", "thing"] }
                ]
            }]
        }
    });

    install_pre_tool_use_hook(root.as_object_mut().unwrap(), "/opt/bin/so-context");

    let hooks = root["hooks"]["PreToolUse"][0]["hooks"].as_array().unwrap();
    assert_eq!(hooks.len(), 2);
    assert!(
        hooks
            .iter()
            .any(|hook| hook["command"].as_str() == Some("/usr/local/bin/custom-hook"))
    );
    assert!(hooks.iter().any(|hook| {
        hook["command"].as_str() == Some("/opt/bin/so-context")
            && hook["args"].as_array().is_some_and(|args| {
                args.len() == 2
                    && args[0].as_str() == Some("hook")
                    && args[1].as_str() == Some("pre-tool")
            })
    }));
}

#[test]
fn install_pre_tool_use_hook_adds_native_read_matcher_group() {
    let mut root = json!({
        "hooks": {}
    });

    install_pre_tool_use_hook(root.as_object_mut().unwrap(), "/opt/bin/so-context");

    let groups = root["hooks"]["PreToolUse"].as_array().unwrap();
    let read_group = groups
        .iter()
        .find(|group| group["matcher"].as_str() == Some("Read"))
        .expect("expected Read matcher group");
    let hooks = read_group["hooks"].as_array().unwrap();
    assert!(hooks.iter().any(|hook| {
        hook["statusMessage"].as_str()
            == Some("Native file read detected; routing to mcp__so-context__so_read")
    }));
}

#[test]
fn remove_post_compact_hook_keeps_other_hooks() {
    let mut root = json!({
        "hooks": {
            "PostCompact": [{
                "hooks": [
                    { "type": "command", "command": "/tmp/so-context", "args": ["hook", "post-compact"] },
                    { "type": "command", "command": "/usr/local/bin/custom-post", "args": ["do", "thing"] }
                ]
            }]
        }
    });

    remove_post_compact_hook(root.as_object_mut().unwrap(), "/tmp/so-context");

    let hooks = root["hooks"]["PostCompact"][0]["hooks"].as_array().unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(
        hooks[0]["command"].as_str(),
        Some("/usr/local/bin/custom-post")
    );
}
