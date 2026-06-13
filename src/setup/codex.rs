//! Installs/uninstalls so-context into Codex CLI's global config.toml.
//!
//! Config path: ~/.codex/config.toml
//!
//! Writes:
//!   - `[mcp_servers.so-context]`      — MCP stdio bridge
//!   - `[[hooks.PreToolUse]]`          — injects `_so_session_id` into so-context tool calls
//!     and reroutes selected native read/shell calls through so-context tools
//!   - `[[hooks.PostCompact]]`         — resets file-visit cache after context compaction
//!   - `~/.codex/AGENTS.md` snippet    — prefer `so_read`/`so_shell` over native read/shell tools
//!
//! Watch lifecycle is handled automatically by the daemon via the MCP connection:
//! projects are registered on `initialize` and unwatched on connection close.
//! No SessionStart/Stop hooks are needed.

use super::hook_binding::{
    HookEvent, toml_hook_command, toml_hook_command_matches_binary_or_legacy,
};
use super::instructions;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::host_adapter::{HostKind, capability_profile};
use crate::routing_session::{NATIVE_READ_TOOL_NAMES, NATIVE_SEARCH_TOOL_NAMES};
use crate::shell::NATIVE_SHELL_TOOL_NAMES;

const SERVER_NAME: &str = "so-context";
const NATIVE_READ_MATCHERS: &[&str] = NATIVE_READ_TOOL_NAMES;
const NATIVE_SEARCH_MATCHERS: &[&str] = NATIVE_SEARCH_TOOL_NAMES;
const NATIVE_SHELL_MATCHERS: &[&str] = NATIVE_SHELL_TOOL_NAMES;

pub(crate) fn install_into_home(home: &Path, binary: &str) -> Result<()> {
    let profile = capability_profile(HostKind::Codex);
    let path = config_path_for(home);
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
    instructions::install_codex_instructions(home)?;
    println!(
        "{}: wrote MCP + hooks to {}",
        profile.display_name,
        path.display()
    );
    Ok(())
}

pub(crate) fn uninstall_from_home(home: &Path, binary: &str) -> Result<()> {
    let profile = capability_profile(HostKind::Codex);
    let path = config_path_for(home);
    if !path.exists() {
        println!(
            "{}: config not found, nothing to remove",
            profile.display_name
        );
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
    remove_pre_tool_use_hook(&mut doc, binary);

    // Remove the PostCompact hook group.
    remove_post_compact_hook(&mut doc, binary);

    fs::write(&path, doc.to_string()).with_context(|| format!("write {}", path.display()))?;
    instructions::uninstall_codex_instructions(home)?;
    println!(
        "{}: removed MCP + hooks from {}",
        profile.display_name,
        path.display()
    );
    Ok(())
}

fn config_path_for(home: &Path) -> PathBuf {
    home.join(".codex").join("config.toml")
}

// ---------------------------------------------------------------------------
// PreToolUse hook
// ---------------------------------------------------------------------------

fn install_pre_tool_use_hook(doc: &mut DocumentMut, binary: &str) {
    let profile = capability_profile(HostKind::Codex);
    let self_tool_matcher = profile.self_tool_naming.hook_matcher();
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
        &self_tool_matcher,
        make_pre_tool_handler(binary, "Tagging so-context call with session ID"),
        binary,
    );
    for matcher in NATIVE_READ_MATCHERS {
        install_pre_tool_group(
            event_aot,
            matcher,
            make_pre_tool_handler(
                binary,
                "Native file read detected; routing to mcp__so-context__so_read",
            ),
            binary,
        );
    }
    for matcher in NATIVE_SEARCH_MATCHERS {
        install_pre_tool_group(
            event_aot,
            matcher,
            make_pre_tool_handler(
                binary,
                "Native search detected; routing to mcp__so-context__so_search",
            ),
            binary,
        );
    }
    for matcher in NATIVE_SHELL_MATCHERS {
        install_pre_tool_group(
            event_aot,
            matcher,
            make_pre_tool_handler(
                binary,
                "Short shell command detected; routing to mcp__so-context__so_shell",
            ),
            binary,
        );
    }
}

fn install_pre_tool_group(
    event_aot: &mut toml_edit::ArrayOfTables,
    matcher: &str,
    handler: Table,
    binary: &str,
) {
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
                .filter(|(_, h)| is_so_context_pre_tool_handler(h, binary))
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
    let profile = capability_profile(HostKind::Codex);
    let mut handler = Table::new();
    handler["type"] = value("command");
    handler["command"] = value(toml_hook_command(binary, HookEvent::PreTool, profile.kind));
    handler["statusMessage"] = value(status_message);
    handler
}

fn is_so_context_pre_tool_handler(hook: &Table, binary: &str) -> bool {
    hook.get("command")
        .and_then(|c| c.as_str())
        .map(|command| {
            toml_hook_command_matches_binary_or_legacy(command, binary, HookEvent::PreTool)
        })
        .unwrap_or(false)
}

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
        let had_so_context = inner
            .iter()
            .any(|hook| is_so_context_post_compact_handler(hook, binary));
        if !had_so_context {
            continue;
        }
        if target_group_idx.is_none() {
            target_group_idx = Some(idx);
        }

        let to_remove: Vec<usize> = inner
            .iter()
            .enumerate()
            .filter(|(_, hook)| is_so_context_post_compact_handler(hook, binary))
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
    let profile = capability_profile(HostKind::Codex);
    let mut handler = Table::new();
    handler["type"] = value("command");
    handler["command"] = value(toml_hook_command(
        binary,
        HookEvent::PostCompact,
        profile.kind,
    ));
    handler["statusMessage"] = value("Resetting so-context file cache after compaction");
    handler
}

fn is_so_context_post_compact_handler(hook: &Table, binary: &str) -> bool {
    hook.get("command")
        .and_then(|c| c.as_str())
        .map(|command| {
            toml_hook_command_matches_binary_or_legacy(command, binary, HookEvent::PostCompact)
        })
        .unwrap_or(false)
}

fn remove_pre_tool_use_hook(doc: &mut DocumentMut, binary: &str) {
    let profile = capability_profile(HostKind::Codex);
    let self_tool_matcher = profile.self_tool_naming.hook_matcher();
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
            .map(|m| {
                m == self_tool_matcher
                    || NATIVE_READ_MATCHERS.contains(&m)
                    || NATIVE_SEARCH_MATCHERS.contains(&m)
                    || NATIVE_SHELL_MATCHERS.contains(&m)
            })
            .unwrap_or(false);
        if !matches_group {
            continue;
        }

        if let Some(inner) = group["hooks"].as_array_of_tables_mut() {
            let to_remove: Vec<usize> = inner
                .iter()
                .enumerate()
                .filter(|(_, hook)| is_so_context_pre_tool_handler(hook, binary))
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

fn remove_post_compact_hook(doc: &mut DocumentMut, binary: &str) {
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
                .filter(|(_, hook)| is_so_context_post_compact_handler(hook, binary))
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
