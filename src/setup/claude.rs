//! Installs so-context into Claude Code's global settings.json.
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

    // --- Hooks ---
    // SessionStart: ensure-watch $PWD
    install_hook(
        obj,
        "SessionStart",
        binary,
        &["ensure-watch"],
        "$PWD",
    );
    // SessionEnd: unwatch $PWD
    install_hook(
        obj,
        "SessionEnd",
        binary,
        &["unwatch"],
        "$PWD",
    );

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    println!("Claude: wrote MCP + hooks to {}", path.display());
    Ok(())
}

/// Adds (or replaces) a so-context hook handler under `hooks.<event>`.
///
/// The hook command is: `<binary> <subcommand...> <path_arg>`
/// e.g. `so-context ensure-watch $PWD`
///
/// We store the hook in a matcher group with `matcher: "*"` so it fires on
/// every occurrence of the event, and we de-duplicate by checking for an
/// existing entry with the same command + args before inserting.
fn install_hook(
    root: &mut Map<String, Value>,
    event: &str,
    binary: &str,
    subcommand: &[&str],
    path_arg: &str,
) {
    let mut args: Vec<Value> = subcommand.iter().map(|s| json!(s)).collect();
    args.push(json!(path_arg));

    let new_hook = json!({
        "type": "command",
        "command": binary,
        "args": args
    });

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

    // Find or create a matcher group for our hooks.
    // We use a dedicated group identified by having a "hooks" array containing
    // a command matching our binary so we don't clobber user groups.
    let group = find_or_create_so_context_group(event_arr, binary);

    let inner = group
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert(json!([]))
        .as_array_mut()
        .unwrap();

    // Replace existing so-context entry for this subcommand, or append.
    let subcommand_key = subcommand.first().copied().unwrap_or("");
    let existing = inner.iter_mut().find(|h| {
        h.get("args")
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            == Some(subcommand_key)
    });

    match existing {
        Some(entry) => *entry = new_hook,
        None => inner.push(new_hook),
    }
}

/// Returns a mutable reference to the so-context matcher group inside an event
/// array, creating it if it doesn't exist.
fn find_or_create_so_context_group<'a>(
    event_arr: &'a mut Vec<Value>,
    binary: &str,
) -> &'a mut Value {
    // Look for an existing group that contains at least one hook with our binary.
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

    // Create a new group.
    event_arr.push(json!({ "matcher": "*", "hooks": [] }));
    event_arr.last_mut().unwrap()
}
