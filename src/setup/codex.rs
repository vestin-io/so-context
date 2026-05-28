//! Installs so-context into Codex CLI's global config.toml.
//!
//! Config path: ~/.codex/config.toml
//!
//! Writes:
//!   - `[mcp_servers.so-context]`  — MCP stdio bridge
//!   - `[[hooks.SessionStart]]`    — ensure-watch on session start
//!   - `[[hooks.Stop]]`            — unwatch on session end (Stop = turn ends)
//!
//! Codex uses the same hook shape as Claude Code:
//!   [[hooks.<Event>]]
//!   matcher = "..."
//!   [[hooks.<Event>.hooks]]
//!   type = "command"
//!   command = "<binary> <args>"

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use toml_edit::{Array, DocumentMut, Item, Table, value};

const SERVER_NAME: &str = "so-context";

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".codex").join("config.toml")
}

pub fn install(binary: &str) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }

    let mut doc: DocumentMut = if path.exists() {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
        text.parse::<DocumentMut>()
            .unwrap_or_else(|_| DocumentMut::new())
    } else {
        DocumentMut::new()
    };

    // --- MCP server ---
    if doc.get("mcp_servers").is_none() {
        doc["mcp_servers"] = Item::Table(Table::new());
    }
    {
        let mcp_servers = doc["mcp_servers"]
            .as_table_mut()
            .context("[mcp_servers] is not a table")?;

        let mut server_table = Table::new();
        server_table["command"] = value(binary);
        let mut args = Array::new();
        args.push("mcp");
        server_table["args"] = value(args);
        mcp_servers[SERVER_NAME] = Item::Table(server_table);
    }

    // --- Hooks ---
    // Codex uses [[hooks.Event]] arrays of matcher-group tables, each with a
    // nested [[hooks.Event.hooks]] array of handlers.
    //
    // We write the hooks as a flat command string: `<binary> <subcommand> $PWD`
    // Codex runs hooks with the session cwd, so $PWD resolves correctly.

    install_hook(&mut doc, "SessionStart", binary, "ensure-watch", "$PWD");
    install_hook(&mut doc, "Stop", binary, "unwatch", "$PWD");

    fs::write(&path, doc.to_string())
        .with_context(|| format!("write {}", path.display()))?;
    println!("Codex: wrote MCP + hooks to {}", path.display());
    Ok(())
}

/// Installs (or replaces) a so-context hook in a Codex config.toml document.
///
/// Writes:
/// ```toml
/// [[hooks.<event>]]
/// matcher = "*"
///
/// [[hooks.<event>.hooks]]
/// type = "command"
/// command = "<binary> <subcommand> <path_arg>"
/// ```
fn install_hook(
    doc: &mut DocumentMut,
    event: &str,
    binary: &str,
    subcommand: &str,
    path_arg: &str,
) {
    let command_str = format!("{binary} {subcommand} {path_arg}");

    // Ensure [hooks] table exists.
    if doc.get("hooks").is_none() {
        doc["hooks"] = Item::Table(Table::new());
    }
    let hooks_table = doc["hooks"].as_table_mut().unwrap();

    // Each event is an array of tables: [[hooks.<event>]].
    // We either find our existing group or append a new one.
    if hooks_table.get(event).is_none() {
        hooks_table[event] = Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
    }

    let event_aot = match hooks_table[event].as_array_of_tables_mut() {
        Some(a) => a,
        None => return, // unexpected shape, skip
    };

    // Find an existing group whose inner hooks array contains our binary.
    let group_idx = event_aot.iter().position(|group| {
        group
            .get("hooks")
            .and_then(|h| h.as_array_of_tables())
            .map(|inner| {
                inner.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c.starts_with(binary))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });

    if let Some(idx) = group_idx {
        // Replace the existing handler in-place.
        let group = event_aot.iter_mut().nth(idx).unwrap();
        if let Some(inner) = group["hooks"].as_array_of_tables_mut() {
            // Find the specific subcommand entry.
            let entry_idx = inner.iter().position(|h| {
                h.get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| c.contains(subcommand))
                    .unwrap_or(false)
            });
            if let Some(ei) = entry_idx {
                if let Some(h) = inner.iter_mut().nth(ei) {
                    h["command"] = value(command_str);
                }
            } else {
                let mut handler = Table::new();
                handler["type"] = value("command");
                handler["command"] = value(command_str);
                inner.push(handler);
            }
        }
    } else {
        // Append a new matcher group.
        let mut group = Table::new();
        group["matcher"] = value("*");

        let mut handler = Table::new();
        handler["type"] = value("command");
        handler["command"] = value(command_str);

        let mut inner_aot = toml_edit::ArrayOfTables::new();
        inner_aot.push(handler);
        group["hooks"] = Item::ArrayOfTables(inner_aot);

        event_aot.push(group);
    }
}
