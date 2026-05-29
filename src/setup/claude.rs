//! Installs/uninstalls so-context into Claude Code's global settings.json.
//!
//! Config path: ~/.claude/settings.json
//!
//! Writes:
//!   - `mcpServers.so-context`          — MCP stdio bridge
//!   - `hooks.SessionStart[].hooks[]`   — runs `so-context ensure-watch` on session start
//!   - `hooks.SessionEnd[].hooks[]`     — runs `so-context unwatch` on session end

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

    // --- Hooks ---
    install_hook(obj, "SessionStart", binary, "ensure-watch");
    install_hook(obj, "SessionEnd", binary, "unwatch");

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

    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut root: Value = serde_json::from_str(&text).unwrap_or(Value::Object(Map::new()));
    let obj = root.as_object_mut().unwrap();

    // Remove MCP server entry.
    if let Some(mcp) = obj.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        mcp.remove(SERVER_NAME);
    }

    // Remove hook groups containing our binary from each event.
    for event in &["SessionStart", "SessionEnd"] {
        remove_hook_group(obj, event, binary);
    }

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    println!("Claude: removed MCP + hooks from {}", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn install_hook(root: &mut Map<String, Value>, event: &str, binary: &str, subcommand: &str) {
    let command = format!(
        r#"SESSION_ID=$(jq -r '.session_id // "unknown"' 2>/dev/null || echo "unknown"); {binary} {subcommand} "$CLAUDE_PROJECT_DIR" --agent claude --session-id "$SESSION_ID""#
    );

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

/// Removes matcher groups from `hooks.<event>` whose hook commands reference `binary`.
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
                hooks
                    .iter()
                    .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(binary))
            })
            .unwrap_or(false)
    });

    if let Some(idx) = pos {
        return &mut event_arr[idx];
    }

    event_arr.push(json!({ "matcher": "*", "hooks": [] }));
    event_arr.last_mut().unwrap()
}
