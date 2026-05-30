//! Installs/uninstalls so-context into Codex CLI's global config.toml.
//!
//! Config path: ~/.codex/config.toml
//!
//! Writes:
//!   - `[mcp_servers.so-context]`      — MCP stdio bridge
//!   - `[[hooks.PreToolUse]]`          — injects `_so_session_id` into so-context tool calls
//!   - `[[hooks.PostCompact]]`         — resets file-visit cache after context compaction
//!
//! Watch lifecycle is handled automatically by the daemon via the MCP connection:
//! projects are registered on `initialize` and unwatched on connection close.
//! No SessionStart/Stop hooks are needed.

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

    // --- PreToolUse hook: inject _so_session_id ---
    install_pre_tool_use_hook(&mut doc, binary);

    // --- PostCompact hook: reset file-visit cache ---
    install_post_compact_hook(&mut doc, binary);

    fs::write(&path, doc.to_string())
        .with_context(|| format!("write {}", path.display()))?;
    println!("Codex: wrote MCP + hooks to {}", path.display());
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let path = config_path();
    if !path.exists() {
        println!("Codex: config not found, nothing to remove");
        return Ok(());
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let mut doc: DocumentMut = text.parse::<DocumentMut>().unwrap_or_else(|_| DocumentMut::new());

    // Remove MCP server entry.
    if let Some(mcp) = doc.get_mut("mcp_servers").and_then(|v| v.as_table_mut()) {
        mcp.remove(SERVER_NAME);
    }

    // Remove the PreToolUse hook group.
    remove_pre_tool_use_hook(&mut doc);

    // Remove the PostCompact hook group.
    remove_post_compact_hook(&mut doc);

    fs::write(&path, doc.to_string())
        .with_context(|| format!("write {}", path.display()))?;
    println!("Codex: removed MCP + hooks from {}", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// PreToolUse hook
// ---------------------------------------------------------------------------

fn install_pre_tool_use_hook(doc: &mut DocumentMut, binary: &str) {
    if doc.get("hooks").is_none() {
        doc["hooks"] = Item::Table(Table::new());
    }
    let hooks_table = doc["hooks"].as_table_mut().unwrap();

    if hooks_table.get("PreToolUse").is_none() {
        hooks_table["PreToolUse"] = Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
    }

    let event_aot = match hooks_table["PreToolUse"].as_array_of_tables_mut() {
        Some(a) => a,
        None => return,
    };

    // Find existing so-context PreToolUse group by matcher.
    let group_idx = event_aot.iter().position(|group| {
        group
            .get("matcher")
            .and_then(|m| m.as_str())
            .map(|m| m == "mcp__so-context__.*")
            .unwrap_or(false)
    });

    if let Some(idx) = group_idx {
        let group = event_aot.iter_mut().nth(idx).unwrap();
        if let Some(inner) = group["hooks"].as_array_of_tables_mut() {
            let to_remove: Vec<usize> = inner
                .iter()
                .enumerate()
                .filter(|(_, h)| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c == binary)
                        .unwrap_or(false)
                        && h.get("args")
                            .and_then(|a| a.as_array())
                            .map(|a| a.iter().any(|v| v.as_str() == Some("pre-tool")))
                            .unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect();
            for i in to_remove.into_iter().rev() {
                inner.remove(i);
            }
            inner.push(make_pre_tool_handler(binary));
        }
    } else {
        let mut group = Table::new();
        group["matcher"] = value("mcp__so-context__.*");

        let mut inner_aot = toml_edit::ArrayOfTables::new();
        inner_aot.push(make_pre_tool_handler(binary));
        group["hooks"] = Item::ArrayOfTables(inner_aot);

        event_aot.push(group);
    }
}

fn make_pre_tool_handler(binary: &str) -> Table {
    let mut handler = Table::new();
    handler["type"] = value("command");
    handler["command"] = value(binary);
    let mut args = Array::new();
    args.push("hook");
    args.push("pre-tool");
    handler["args"] = value(args);
    handler["statusMessage"] = value("Tagging so-context call with session ID");
    handler
}

fn remove_pre_tool_use_hook(doc: &mut DocumentMut) {
    let hooks_table = match doc.get_mut("hooks").and_then(|v| v.as_table_mut()) {
        Some(t) => t,
        None => return,
    };

    let aot = match hooks_table
        .get_mut("PreToolUse")
        .and_then(|v| v.as_array_of_tables_mut())
    {
        Some(a) => a,
        None => return,
    };

    let to_remove: Vec<usize> = aot
        .iter()
        .enumerate()
        .filter(|(_, group)| {
            group
                .get("matcher")
                .and_then(|m| m.as_str())
                .map(|m| m == "mcp__so-context__.*")
                .unwrap_or(false)
        })
        .map(|(i, _)| i)
        .collect();

    for idx in to_remove.into_iter().rev() {
        aot.remove(idx);
    }
}

// ---------------------------------------------------------------------------
// PostCompact hook
// ---------------------------------------------------------------------------

fn install_post_compact_hook(doc: &mut DocumentMut, binary: &str) {
    if doc.get("hooks").is_none() {
        doc["hooks"] = Item::Table(Table::new());
    }
    let hooks_table = doc["hooks"].as_table_mut().unwrap();

    if hooks_table.get("PostCompact").is_none() {
        hooks_table["PostCompact"] = Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
    }

    let event_aot = match hooks_table["PostCompact"].as_array_of_tables_mut() {
        Some(a) => a,
        None => return,
    };

    // Find existing group containing our binary+post-compact handler.
    let group_idx = event_aot.iter().position(|group| {
        group
            .get("hooks")
            .and_then(|h| h.as_array_of_tables())
            .map(|inner| {
                inner.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c == binary)
                        .unwrap_or(false)
                        && h.get("args")
                            .and_then(|a| a.as_array())
                            .map(|a| a.iter().any(|v| v.as_str() == Some("post-compact")))
                            .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });

    if let Some(idx) = group_idx {
        let group = event_aot.iter_mut().nth(idx).unwrap();
        if let Some(inner) = group["hooks"].as_array_of_tables_mut() {
            let to_remove: Vec<usize> = inner
                .iter()
                .enumerate()
                .filter(|(_, h)| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c == binary)
                        .unwrap_or(false)
                        && h.get("args")
                            .and_then(|a| a.as_array())
                            .map(|a| a.iter().any(|v| v.as_str() == Some("post-compact")))
                            .unwrap_or(false)
                })
                .map(|(i, _)| i)
                .collect();
            for i in to_remove.into_iter().rev() {
                inner.remove(i);
            }
            inner.push(make_post_compact_handler(binary));
        }
    } else {
        let mut group = Table::new();
        // PostCompact has no matcher — it fires unconditionally.
        let mut inner_aot = toml_edit::ArrayOfTables::new();
        inner_aot.push(make_post_compact_handler(binary));
        group["hooks"] = Item::ArrayOfTables(inner_aot);
        event_aot.push(group);
    }
}

fn make_post_compact_handler(binary: &str) -> Table {
    let mut handler = Table::new();
    handler["type"] = value("command");
    handler["command"] = value(binary);
    let mut args = Array::new();
    args.push("hook");
    args.push("post-compact");
    handler["args"] = value(args);
    handler["statusMessage"] = value("Resetting so-context file cache after compaction");
    handler
}

fn remove_post_compact_hook(doc: &mut DocumentMut) {
    let hooks_table = match doc.get_mut("hooks").and_then(|v| v.as_table_mut()) {
        Some(t) => t,
        None => return,
    };

    let aot = match hooks_table
        .get_mut("PostCompact")
        .and_then(|v| v.as_array_of_tables_mut())
    {
        Some(a) => a,
        None => return,
    };

    // Remove any group containing a handler with "post-compact" in its args.
    let to_remove: Vec<usize> = aot
        .iter()
        .enumerate()
        .filter(|(_, group)| {
            group
                .get("hooks")
                .and_then(|h| h.as_array_of_tables())
                .map(|inner| {
                    inner.iter().any(|h| {
                        h.get("args")
                            .and_then(|a| a.as_array())
                            .map(|a| a.iter().any(|v| v.as_str() == Some("post-compact")))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .map(|(i, _)| i)
        .collect();

    for idx in to_remove.into_iter().rev() {
        aot.remove(idx);
    }
}
