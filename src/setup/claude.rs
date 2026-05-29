//! Installs/uninstalls so-context into Claude Code's global settings.json.
//!
//! Config path: ~/.claude/settings.json
//!
//! Writes:
//!   - `mcpServers.so-context`          — MCP stdio bridge
//!   - `hooks.SessionStart[].hooks[]`   — runs `so-context watch` on session start
//!   - `hooks.SessionEnd[].hooks[]`     — runs `so-context unwatch` on session end
//!   - `hooks.PreToolUse[].hooks[]`     — injects `_so_session_id` into so-context tool calls

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

    // --- Lifecycle hooks ---
    install_hook(obj, "SessionStart", binary, "watch");
    install_hook(obj, "SessionEnd", binary, "unwatch");

    // --- PreToolUse hook: inject _so_session_id ---
    install_pre_tool_use_hook(obj, binary);

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

    // Remove lifecycle hook groups containing our binary from each event.
    for event in &["SessionStart", "SessionEnd"] {
        remove_hook_group(obj, event, binary);
    }

    // Remove the PreToolUse hook group referencing our binary.
    remove_pre_tool_use_hook(obj, binary);

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
// Lifecycle hooks (SessionStart / SessionEnd)
// ---------------------------------------------------------------------------

fn install_hook(root: &mut Map<String, Value>, event: &str, binary: &str, subcommand: &str) {
    let command = format!(r#""{binary}" {subcommand} "$CLAUDE_PROJECT_DIR""#);

    let new_hook = json!({ "type": "command", "command": command });

    let hooks_obj = root
        .entry("hooks")
        .or_insert(json!({}))
        .as_object_mut()
        .unwrap();

    let event_arr = hooks_obj
        .entry(event)
        .or_insert(json!([]))
        .as_array_mut()
        .unwrap();

    let group = find_or_create_so_context_group(event_arr, binary);

    let inner = group
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert(json!([]))
        .as_array_mut()
        .unwrap();

    let existing = inner.iter_mut().find(|h| {
        h.get("command")
            .and_then(|c| c.as_str())
            .map(|c| c.contains(subcommand))
            .unwrap_or(false)
    });

    match existing {
        Some(entry) => *entry = new_hook,
        None => inner.push(new_hook),
    }
}

fn remove_hook_group(root: &mut Map<String, Value>, event: &str, binary: &str) {
    let arr = match root
        .get_mut("hooks")
        .and_then(|h| h.as_object_mut())
        .and_then(|h| h.get_mut(event))
        .and_then(|v| v.as_array_mut())
    {
        Some(a) => a,
        None => return,
    };

    arr.retain(|group| {
        !group
            .get("hooks")
            .and_then(|h| h.as_array())
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c.contains(binary))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });
}

fn find_or_create_so_context_group<'a>(
    event_arr: &'a mut Vec<Value>,
    binary: &str,
) -> &'a mut Value {
    let pos = event_arr.iter().position(|g| {
        g.get("hooks")
            .and_then(|h| h.as_array())
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command").and_then(|c| c.as_str()) == Some(binary)
                })
            })
            .unwrap_or(false)
    });

    if let Some(idx) = pos {
        return &mut event_arr[idx];
    }

    event_arr.push(json!({ "matcher": "*", "hooks": [] }));
    event_arr.last_mut().unwrap()
}
