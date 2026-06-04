//! Installs/uninstalls so-context into Claude Code's global settings.json.
//!
//! Config path: ~/.claude/settings.json
//!
//! Writes:
//!   - `mcpServers.so-context`          — MCP stdio bridge
//!   - `hooks.PreToolUse[].hooks[]`     — injects `_so_session_id` into so-context tool calls
//!                                        and blocks short native shell commands so the agent retries with `so_shell`
//!   - `hooks.PostCompact[].hooks[]`    — resets file-visit cache after context compaction
//!   - `~/.claude/CLAUDE.md` snippet    — prefer `so_shell` for one-shot shell commands
//!
//! Watch lifecycle is handled automatically by the daemon via the MCP connection:
//! projects are registered on `initialize` and unwatched on connection close.
//! No SessionStart/SessionEnd hooks are needed.

use super::instructions;
use anyhow::{Context, Result};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

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

    // --- PreToolUse hook: inject _so_session_id ---
    install_pre_tool_use_hook(obj, binary);

    // --- PostCompact hook: reset file-visit cache ---
    install_post_compact_hook(obj, binary);

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    instructions::install_claude_instructions(&instructions::home_dir())?;
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

    // Remove the PreToolUse hook group.
    remove_pre_tool_use_hook(obj, binary);

    // Remove the PostCompact hook group.
    remove_post_compact_hook(obj, binary);

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    instructions::uninstall_claude_instructions(&instructions::home_dir())?;
    println!("Claude: removed MCP + hooks from {}", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// PreToolUse hook
// ---------------------------------------------------------------------------

fn install_pre_tool_use_hook(root: &mut Map<String, Value>, binary: &str) {
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
        SO_CONTEXT_MCP_MATCHER,
        make_pre_tool_handler(binary, "Tagging so-context call with session ID"),
    );
    for matcher in NATIVE_SHELL_MATCHERS {
        install_pre_tool_group(
            event_arr,
            matcher,
            make_pre_tool_handler(
                binary,
                "Short shell command detected; routing to mcp__so-context__so_shell",
            ),
        );
    }
}

fn remove_pre_tool_use_hook(root: &mut Map<String, Value>, _binary: &str) {
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
            .map(|m| m == SO_CONTEXT_MCP_MATCHER || NATIVE_SHELL_MATCHERS.contains(&m))
            .unwrap_or(false);
        if !is_so_context_matcher {
            continue;
        }

        if let Some(hooks) = group.get_mut("hooks").and_then(|h| h.as_array_mut()) {
            hooks.retain(|hook| !is_so_context_hook_value(hook, "pre-tool"));
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
            inner.retain(|h| !is_so_context_hook_value(h, "pre-tool"));
            inner.push(new_hook);
        }
    } else {
        event_arr.push(json!({
            "matcher": matcher,
            "hooks": [new_hook]
        }));
    }
}

fn is_so_context_hook_value(hook: &Value, hook_name: &str) -> bool {
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

fn make_pre_tool_handler(binary: &str, status_message: &str) -> Value {
    json!({
        "type": "command",
        "command": binary,
        "args": ["hook", "pre-tool"],
        "statusMessage": status_message
    })
}

// ---------------------------------------------------------------------------
// PostCompact hook
// ---------------------------------------------------------------------------

fn install_post_compact_hook(root: &mut Map<String, Value>, binary: &str) {
    let new_hook = json!({
        "type": "command",
        "command": binary,
        "args": ["hook", "post-compact"],
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
                        && h.get("args")
                            .and_then(|a| a.as_array())
                            .map(|a| a.iter().any(|v| v.as_str() == Some("post-compact")))
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
                !(h.get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| c == binary)
                    .unwrap_or(false)
                    && h.get("args")
                        .and_then(|a| a.as_array())
                        .map(|a| a.iter().any(|v| v.as_str() == Some("post-compact")))
                        .unwrap_or(false))
            });
            inner.push(new_hook);
        }
    } else {
        event_arr.push(json!({ "hooks": [new_hook] }));
    }
}

fn remove_post_compact_hook(root: &mut Map<String, Value>, _binary: &str) {
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
            hooks.retain(|hook| !is_so_context_hook_value(hook, "post-compact"));
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
