//! Installs/uninstalls so-context into Codex CLI's global config.toml.
//!
//! Config path: ~/.codex/config.toml
//!
//! Writes:
//!   - `[mcp_servers.so-context]`      — MCP stdio bridge
//!   - `[[hooks.PreToolUse]]`          — injects `_so_session_id` into so-context tool calls
//!                                       and blocks short native shell commands so the agent retries with `so_shell`
//!   - `[[hooks.PostCompact]]`         — resets file-visit cache after context compaction
//!   - `~/.codex/AGENTS.md` snippet    — prefer `so_shell` for one-shot shell commands
//!
//! Watch lifecycle is handled automatically by the daemon via the MCP connection:
//! projects are registered on `initialize` and unwatched on connection close.
//! No SessionStart/Stop hooks are needed.

use super::instructions;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use toml_edit::{Array, DocumentMut, Item, Table, value};

const SERVER_NAME: &str = "so-context";
const SO_CONTEXT_MCP_MATCHER: &str = "mcp__so-context__.*";
const NATIVE_SHELL_MATCHERS: &[&str] = &[
    "Bash",
    "bash",
    "Shell",
    "shell",
    "runTerminalCommand",
    "runInTerminal",
    "run_in_terminal",
    "terminal",
    "shell_command",
    "exec_command",
    "local_shell",
    "run_shell_command",
];

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".codex").join("config.toml")
}

pub fn install(binary: &str) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create dir {}", parent.display()))?;
    }

    let mut doc: DocumentMut = if path.exists() {
        let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
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

    fs::write(&path, doc.to_string()).with_context(|| format!("write {}", path.display()))?;
    instructions::install_codex_instructions(&instructions::home_dir())?;
    println!("Codex: wrote MCP + hooks to {}", path.display());
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let path = config_path();
    if !path.exists() {
        println!("Codex: config not found, nothing to remove");
        return Ok(());
    }

    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut doc: DocumentMut = text
        .parse::<DocumentMut>()
        .unwrap_or_else(|_| DocumentMut::new());

    // Remove MCP server entry.
    if let Some(mcp) = doc.get_mut("mcp_servers").and_then(|v| v.as_table_mut()) {
        mcp.remove(SERVER_NAME);
    }

    // Remove the PreToolUse hook group.
    remove_pre_tool_use_hook(&mut doc);

    // Remove the PostCompact hook group.
    remove_post_compact_hook(&mut doc);

    fs::write(&path, doc.to_string()).with_context(|| format!("write {}", path.display()))?;
    instructions::uninstall_codex_instructions(&instructions::home_dir())?;
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

    install_pre_tool_group(
        event_aot,
        SO_CONTEXT_MCP_MATCHER,
        make_pre_tool_handler(binary, "Tagging so-context call with session ID"),
    );
    for matcher in NATIVE_SHELL_MATCHERS {
        install_pre_tool_group(
            event_aot,
            matcher,
            make_pre_tool_handler(
                binary,
                "Short shell command detected; routing to mcp__so-context__so_shell",
            ),
        );
    }
}

fn install_pre_tool_group(event_aot: &mut toml_edit::ArrayOfTables, matcher: &str, handler: Table) {
    let group_idx = event_aot.iter().position(|group| {
        group
            .get("matcher")
            .and_then(|m| m.as_str())
            .map(|m| m == matcher)
            .unwrap_or(false)
    });

    if let Some(idx) = group_idx {
        let group = event_aot.iter_mut().nth(idx).unwrap();
        if let Some(inner) = group["hooks"].as_array_of_tables_mut() {
            let to_remove: Vec<usize> = inner
                .iter()
                .enumerate()
                .filter(|(_, h)| is_so_context_pre_tool_handler(h))
                .map(|(i, _)| i)
                .collect();
            for i in to_remove.into_iter().rev() {
                inner.remove(i);
            }
            inner.push(handler);
        }
    } else {
        let mut group = Table::new();
        group["matcher"] = value(matcher);

        let mut inner_aot = toml_edit::ArrayOfTables::new();
        inner_aot.push(handler);
        group["hooks"] = Item::ArrayOfTables(inner_aot);

        event_aot.push(group);
    }
}

fn make_pre_tool_handler(binary: &str, status_message: &str) -> Table {
    let mut handler = Table::new();
    handler["type"] = value("command");
    handler["command"] = value(format!("{binary} hook pre-tool"));
    handler["statusMessage"] = value(status_message);
    handler
}

fn command_invokes_so_context_hook(command: &str, suffix: &str) -> bool {
    let Some(binary) = command.strip_suffix(suffix) else {
        return false;
    };
    Path::new(binary.trim_end())
        .file_name()
        .and_then(|name| name.to_str())
        == Some("so-context")
}

fn is_legacy_so_context_hook(hook: &Table, hook_name: &str) -> bool {
    let command_matches = hook
        .get("command")
        .and_then(|c| c.as_str())
        .map(|command| {
            Path::new(command)
                .file_name()
                .and_then(|name| name.to_str())
                == Some("so-context")
        })
        .unwrap_or(false);
    let args_match = hook
        .get("args")
        .and_then(|a| a.as_array())
        .map(|args| {
            args.len() == 2
                && args.get(0).and_then(|v| v.as_str()) == Some("hook")
                && args.get(1).and_then(|v| v.as_str()) == Some(hook_name)
        })
        .unwrap_or(false);

    command_matches && args_match
}

fn is_so_context_pre_tool_handler(hook: &Table) -> bool {
    let command_matches = hook
        .get("command")
        .and_then(|c| c.as_str())
        .map(|command| command_invokes_so_context_hook(command, " hook pre-tool"))
        .unwrap_or(false);

    command_matches || is_legacy_so_context_hook(hook, "pre-tool")
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

    let mut groups_to_remove = Vec::new();
    for (idx, group) in aot.iter_mut().enumerate() {
        let matches_group = group
            .get("matcher")
            .and_then(|m| m.as_str())
            .map(|m| m == SO_CONTEXT_MCP_MATCHER || NATIVE_SHELL_MATCHERS.contains(&m))
            .unwrap_or(false);
        if !matches_group {
            continue;
        }

        if let Some(inner) = group["hooks"].as_array_of_tables_mut() {
            let to_remove: Vec<usize> = inner
                .iter()
                .enumerate()
                .filter(|(_, hook)| is_so_context_pre_tool_handler(hook))
                .map(|(i, _)| i)
                .collect();
            for hook_idx in to_remove.into_iter().rev() {
                inner.remove(hook_idx);
            }
            if inner.is_empty() {
                groups_to_remove.push(idx);
            }
        }
    }

    for idx in groups_to_remove.into_iter().rev() {
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

    let mut target_group_idx: Option<usize> = None;
    let mut groups_to_remove = Vec::new();

    for (idx, group) in event_aot.iter_mut().enumerate() {
        let Some(inner) = group["hooks"].as_array_of_tables_mut() else {
            continue;
        };
        let had_so_context = inner.iter().any(is_so_context_post_compact_handler);
        if !had_so_context {
            continue;
        }
        if target_group_idx.is_none() {
            target_group_idx = Some(idx);
        }

        let to_remove: Vec<usize> = inner
            .iter()
            .enumerate()
            .filter(|(_, hook)| is_so_context_post_compact_handler(hook))
            .map(|(i, _)| i)
            .collect();
        for hook_idx in to_remove.into_iter().rev() {
            inner.remove(hook_idx);
        }
        if inner.is_empty() && Some(idx) != target_group_idx {
            groups_to_remove.push(idx);
        }
    }

    match target_group_idx {
        Some(idx) => {
            if let Some(inner) = event_aot
                .iter_mut()
                .nth(idx)
                .and_then(|group| group["hooks"].as_array_of_tables_mut())
            {
                inner.push(make_post_compact_handler(binary));
            }
        }
        None => {
            let mut group = Table::new();
            // PostCompact has no matcher — it fires unconditionally.
            let mut inner_aot = toml_edit::ArrayOfTables::new();
            inner_aot.push(make_post_compact_handler(binary));
            group["hooks"] = Item::ArrayOfTables(inner_aot);
            event_aot.push(group);
        }
    }

    for idx in groups_to_remove.into_iter().rev() {
        event_aot.remove(idx);
    }
}

fn make_post_compact_handler(binary: &str) -> Table {
    let mut handler = Table::new();
    handler["type"] = value("command");
    handler["command"] = value(format!("{binary} hook post-compact"));
    handler["statusMessage"] = value("Resetting so-context file cache after compaction");
    handler
}

fn is_so_context_post_compact_handler(hook: &Table) -> bool {
    let command_matches = hook
        .get("command")
        .and_then(|c| c.as_str())
        .map(|command| command_invokes_so_context_hook(command, " hook post-compact"))
        .unwrap_or(false);

    command_matches || is_legacy_so_context_hook(hook, "post-compact")
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

    let mut groups_to_remove = Vec::new();
    for (idx, group) in aot.iter_mut().enumerate() {
        if let Some(inner) = group["hooks"].as_array_of_tables_mut() {
            let to_remove: Vec<usize> = inner
                .iter()
                .enumerate()
                .filter(|(_, hook)| is_so_context_post_compact_handler(hook))
                .map(|(i, _)| i)
                .collect();
            for hook_idx in to_remove.into_iter().rev() {
                inner.remove(hook_idx);
            }
            if inner.is_empty() {
                groups_to_remove.push(idx);
            }
        }
    }

    for idx in groups_to_remove.into_iter().rev() {
        aot.remove(idx);
    }
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
