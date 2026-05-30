//! Installs/uninstalls so-context into Claude Code's global settings.json.
//!
//! Config path: ~/.claude/settings.json
//!
//! Writes:
//!   - `mcpServers.so-context`          — MCP stdio bridge
//!   - `hooks.PreToolUse[].hooks[]`     — injects `_so_session_id` into so-context tool calls
//!   - `hooks.PostCompact[].hooks[]`    — resets file-visit cache after context compaction
//!
//! Watch lifecycle is handled automatically by the daemon via the MCP connection:
//! projects are registered on `initialize` and unwatched on connection close.
//! No SessionStart/SessionEnd hooks are needed.

use anyhow::{Context, Result};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::PathBuf;

const SERVER_NAME: &str = "so-context";

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".claude").join("settings.json")
}

pub fn install(binary: &str) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }

    let mut root: Value = if path.exists() {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
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
    println!("Claude: wrote MCP + hooks to {}", path.display());
    Ok(())
}

pub fn uninstall(binary: &str) -> Result<()> {
    let path = config_path();
    if !path.exists() {
        println!("Claude: config not found, nothing to remove");
        return Ok(());
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
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
    println!("Claude: removed MCP + hooks from {}", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// PreToolUse hook
// ---------------------------------------------------------------------------

fn install_pre_tool_use_hook(root: &mut Map<String, Value>, binary: &str) {
    let new_hook = json!({
        "type": "command",
        "command": binary,
        "args": ["hook"],
        "statusMessage": "Tagging so-context call with session ID"
    });

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

    let pos = event_arr.iter().position(|g| {
        g.get("matcher")
            .and_then(|m| m.as_str())
            .map(|m| m == "mcp__so-context__.*")
            .unwrap_or(false)
    });

    if let Some(idx) = pos {
        if let Some(inner) = event_arr[idx]
            .as_object_mut()
            .and_then(|g| g.get_mut("hooks"))
            .and_then(|h| h.as_array_mut())
        {
            inner.retain(|h| {
                h.get("args")
                    .and_then(|a| a.as_array())
                    .map(|a| a.iter().all(|v| v.as_str() != Some("hook")))
                    .unwrap_or(true)
            });
            inner.push(new_hook);
        }
    } else {
        event_arr.push(json!({
            "matcher": "mcp__so-context__.*",
            "hooks": [new_hook]
        }));
    }
}

fn remove_pre_tool_use_hook(root: &mut Map<String, Value>, binary: &str) {
    let arr = match root
        .get_mut("hooks")
        .and_then(|h| h.as_object_mut())
        .and_then(|h| h.get_mut("PreToolUse"))
        .and_then(|v| v.as_array_mut())
    {
        Some(a) => a,
        None => return,
    };

    arr.retain(|group| {
        let is_so_context_matcher = group
            .get("matcher")
            .and_then(|m| m.as_str())
            .map(|m| m == "mcp__so-context__.*")
            .unwrap_or(false);
        let has_our_binary = group
            .get("hooks")
            .and_then(|h| h.as_array())
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c == binary)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        !(is_so_context_matcher && has_our_binary)
    });
}

// ---------------------------------------------------------------------------
// PostCompact hook
// ---------------------------------------------------------------------------

fn install_post_compact_hook(root: &mut Map<String, Value>, binary: &str) {
    let new_hook = json!({
        "type": "command",
        "command": binary,
        "args": ["compact"],
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

    // There's only one group for PostCompact (no matcher filter needed).
    // Find existing group that contains our binary and update it; otherwise append.
    let pos = event_arr.iter().position(|g| {
        g.get("hooks")
            .and_then(|h| h.as_array())
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c == binary)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });

    if let Some(idx) = pos {
        // Replace to pick up any args change.
        if let Some(inner) = event_arr[idx]
            .as_object_mut()
            .and_then(|g| g.get_mut("hooks"))
            .and_then(|h| h.as_array_mut())
        {
            inner.retain(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| c != binary)
                    .unwrap_or(true)
            });
            inner.push(new_hook);
        }
    } else {
        event_arr.push(json!({ "hooks": [new_hook] }));
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

    arr.retain(|group| {
        let has_our_binary = group
            .get("hooks")
            .and_then(|h| h.as_array())
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c == binary)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        !has_our_binary
    });
}
