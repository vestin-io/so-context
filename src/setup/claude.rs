//! Installs/uninstalls so-context into Claude Code's global settings.json.
//!
//! Config path: ~/.claude/settings.json
//!
//! Writes:
//!   - `mcpServers.so-context`          — MCP stdio bridge
//!   - `hooks.PreToolUse[].hooks[]`     — injects `_so_session_id` into so-context tool calls
//!     and reroutes selected native shell calls through the so-context shell CLI
//!   - `hooks.PostCompact[].hooks[]`    — resets file-visit cache after context compaction
//!   - `~/.claude/CLAUDE.md` snippet    — prefer `so_read`/`so_shell` over native read/shell tools
//!
//! Watch lifecycle is handled automatically by the daemon via the MCP connection:
//! projects are registered on `initialize` and unwatched on connection close.
//! No SessionStart/SessionEnd hooks are needed.

use super::hook_binding::{HookEvent, json_hook_args, json_hook_value_matches_binary_or_legacy};
use super::instructions;
use anyhow::{Context, Result};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::{Path, PathBuf};

use crate::host_adapter::{HostKind, capability_profile};
use crate::routing_session::{NATIVE_READ_TOOL_NAMES, NATIVE_SEARCH_TOOL_NAMES};
use crate::shell::NATIVE_SHELL_TOOL_NAMES;

const SERVER_NAME: &str = "so-context";
const LEGACY_NATIVE_READ_MATCHERS: &[&str] = NATIVE_READ_TOOL_NAMES;
const LEGACY_NATIVE_SEARCH_MATCHERS: &[&str] = NATIVE_SEARCH_TOOL_NAMES;
const NATIVE_SHELL_MATCHERS: &[&str] = NATIVE_SHELL_TOOL_NAMES;

pub(crate) fn install_into_home(home: &Path, binary: &str) -> Result<()> {
    let profile = capability_profile(HostKind::Claude);
    let path = config_path_for(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }

    let mut root: Value = if path.exists() {
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&text).unwrap_or(Value::Object(Map::new()))
    } else {
        Value::Object(Map::new())
    };

    let obj = root.as_object_mut().unwrap();

    // --- MCP server ---
    obj.entry("mcpServers")
        .or_insert(json!({}))
        .as_object_mut()
        .unwrap()
        .insert(
            SERVER_NAME.to_string(),
            json!({ "command": binary, "args": ["mcp"] }),
        );

    // --- PreToolUse hook: inject _so_session_id ---
    install_pre_tool_use_hook(obj, binary);

    // --- PostCompact hook: reset file-visit cache ---
    install_post_compact_hook(obj, binary);

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    instructions::install_claude_instructions(home)?;
    println!(
        "{}: wrote MCP + hooks to {}",
        profile.display_name,
        path.display()
    );
    Ok(())
}

pub(crate) fn uninstall_from_home(home: &Path, binary: &str) -> Result<()> {
    let profile = capability_profile(HostKind::Claude);
    let path = config_path_for(home);
    if !path.exists() {
        println!(
            "{}: config not found, nothing to remove",
            profile.display_name
        );
        return Ok(());
    }

    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut root: Value = serde_json::from_str(&text).unwrap_or(Value::Object(Map::new()));
    let obj = root.as_object_mut().unwrap();

    // Remove MCP server entry.
    if let Some(mcp) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        mcp.remove(SERVER_NAME);
    }

    // Remove the PreToolUse hook group.
    remove_pre_tool_use_hook(obj, binary);

    // Remove the PostCompact hook group.
    remove_post_compact_hook(obj, binary);

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    instructions::uninstall_claude_instructions(home)?;
    println!(
        "{}: removed MCP + hooks from {}",
        profile.display_name,
        path.display()
    );
    Ok(())
}

fn config_path_for(home: &Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

// ---------------------------------------------------------------------------
// PreToolUse hook
// ---------------------------------------------------------------------------

fn install_pre_tool_use_hook(root: &mut Map<String, Value>, binary: &str) {
    remove_pre_tool_use_hook(root, binary);
    let profile = capability_profile(HostKind::Claude);
    let self_tool_matcher = profile.self_tool_naming.hook_matcher();
    let hooks_obj = root
        .entry("hooks")
        .or_insert(json!({}))
        .as_object_mut()
        .unwrap();

    let event_arr = hooks_obj
        .entry("PreToolUse")
        .or_insert(json!([]))
        .as_array_mut()
        .unwrap();

    install_pre_tool_group(
        event_arr,
        &self_tool_matcher,
        make_pre_tool_handler(binary, "Tagging so-context call with session ID"),
    );
    for matcher in NATIVE_SHELL_MATCHERS {
        install_pre_tool_group(
            event_arr,
            matcher,
            make_pre_tool_handler(
                binary,
                "Short shell command detected; routing through so-context shell CLI",
            ),
        );
    }
}

fn remove_pre_tool_use_hook(root: &mut Map<String, Value>, binary: &str) {
    let profile = capability_profile(HostKind::Claude);
    let self_tool_matcher = profile.self_tool_naming.hook_matcher();
    let arr = match root
        .get_mut("hooks")
        .and_then(|h| h.as_object_mut())
        .and_then(|h| h.get_mut("PreToolUse"))
        .and_then(|v| v.as_array_mut())
    {
        Some(a) => a,
        None => return,
    };

    let mut empty_groups = Vec::new();
    for (idx, group) in arr.iter_mut().enumerate() {
        let is_so_context_matcher = group
            .get("matcher")
            .and_then(|m| m.as_str())
            .map(|m| {
                m == self_tool_matcher
                    || LEGACY_NATIVE_READ_MATCHERS.contains(&m)
                    || LEGACY_NATIVE_SEARCH_MATCHERS.contains(&m)
                    || NATIVE_SHELL_MATCHERS.contains(&m)
            })
            .unwrap_or(false);
        if !is_so_context_matcher {
            continue;
        }

        if let Some(hooks) = group.get_mut("hooks").and_then(|h| h.as_array_mut()) {
            hooks.retain(|hook| !is_so_context_hook_value(hook, binary, "pre-tool"));
            if hooks.is_empty() {
                empty_groups.push(idx);
            }
        }
    }

    for idx in empty_groups.into_iter().rev() {
        arr.remove(idx);
    }
}

fn install_pre_tool_group(event_arr: &mut Vec<Value>, matcher: &str, new_hook: Value) {
    let pos = event_arr.iter().position(|g| {
        g.get("matcher")
            .and_then(|m| m.as_str())
            .map(|m| m == matcher)
            .unwrap_or(false)
    });

    if let Some(idx) = pos {
        if let Some(inner) = event_arr[idx]
            .as_object_mut()
            .and_then(|g| g.get_mut("hooks"))
            .and_then(|h| h.as_array_mut())
        {
            let binary = new_hook
                .get("command")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            inner.retain(|h| !is_so_context_hook_value(h, &binary, "pre-tool"));
            inner.push(new_hook);
        }
    } else {
        event_arr.push(json!({
            "matcher": matcher,
            "hooks": [new_hook]
        }));
    }
}

fn is_so_context_hook_value(hook: &Value, binary: &str, hook_name: &str) -> bool {
    match hook_name {
        "pre-tool" => json_hook_value_matches_binary_or_legacy(hook, binary, HookEvent::PreTool),
        "post-compact" => {
            json_hook_value_matches_binary_or_legacy(hook, binary, HookEvent::PostCompact)
        }
        _ => false,
    }
}

fn make_pre_tool_handler(binary: &str, status_message: &str) -> Value {
    let profile = capability_profile(HostKind::Claude);

    json!({
        "type": "command",
        "command": binary,
        "args": json_hook_args(HookEvent::PreTool, profile.kind),
        "statusMessage": status_message
    })
}

// ---------------------------------------------------------------------------
// PostCompact hook
// ---------------------------------------------------------------------------

fn install_post_compact_hook(root: &mut Map<String, Value>, binary: &str) {
    let profile = capability_profile(HostKind::Claude);
    let new_hook = json!({
        "type": "command",
        "command": binary,
        "args": json_hook_args(HookEvent::PostCompact, profile.kind),
        "statusMessage": "Resetting so-context file cache after compaction"
    });

    let hooks_obj = root
        .entry("hooks")
        .or_insert(json!({}))
        .as_object_mut()
        .unwrap();

    let event_arr = hooks_obj
        .entry("PostCompact")
        .or_insert(json!([]))
        .as_array_mut()
        .unwrap();

    let mut target_group_idx: Option<usize> = None;
    let mut groups_to_remove = Vec::new();

    for (idx, group) in event_arr.iter_mut().enumerate() {
        let Some(inner) = group
            .as_object_mut()
            .and_then(|g| g.get_mut("hooks"))
            .and_then(|h| h.as_array_mut())
        else {
            continue;
        };

        let had_so_context = inner
            .iter()
            .any(|hook| is_so_context_hook_value(hook, binary, "post-compact"));
        if !had_so_context {
            continue;
        }
        if target_group_idx.is_none() {
            target_group_idx = Some(idx);
        }

        inner.retain(|hook| !is_so_context_hook_value(hook, binary, "post-compact"));
        if inner.is_empty() && Some(idx) != target_group_idx {
            groups_to_remove.push(idx);
        }
    }

    match target_group_idx {
        Some(idx) => {
            if let Some(inner) = event_arr[idx]
                .as_object_mut()
                .and_then(|g| g.get_mut("hooks"))
                .and_then(|h| h.as_array_mut())
            {
                inner.push(new_hook);
            }
        }
        None => event_arr.push(json!({ "hooks": [new_hook] })),
    }

    for idx in groups_to_remove.into_iter().rev() {
        event_arr.remove(idx);
    }
}

fn remove_post_compact_hook(root: &mut Map<String, Value>, binary: &str) {
    let arr = match root
        .get_mut("hooks")
        .and_then(|h| h.as_object_mut())
        .and_then(|h| h.get_mut("PostCompact"))
        .and_then(|v| v.as_array_mut())
    {
        Some(a) => a,
        None => return,
    };

    let mut empty_groups = Vec::new();
    for (idx, group) in arr.iter_mut().enumerate() {
        if let Some(hooks) = group.get_mut("hooks").and_then(|h| h.as_array_mut()) {
            hooks.retain(|hook| !is_so_context_hook_value(hook, binary, "post-compact"));
            if hooks.is_empty() {
                empty_groups.push(idx);
            }
        }
    }

    for idx in empty_groups.into_iter().rev() {
        arr.remove(idx);
    }
}

#[cfg(test)]
#[path = "claude_tests.rs"]
mod tests;
