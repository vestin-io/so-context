//! Installs/uninstalls so-context into OpenCode's global config.
//!
//! Writes:
//!   - MCP server entry in `~/.config/opencode/opencode.json`
//!   - Plugin file at `~/.config/opencode/plugins/so-context.ts`

use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::host_adapter::{HostKind, capability_profile};
use crate::routing_session::{NATIVE_READ_TOOL_NAMES, NATIVE_SEARCH_TOOL_NAMES};
use crate::shell::NATIVE_SHELL_TOOL_NAMES;

const SERVER_NAME: &str = "so-context";

pub(crate) fn install_into_home(home: &Path, binary: &str) -> Result<()> {
    install_mcp_at(&config_path_for(home), binary)?;
    install_plugin_at(&plugin_path_for(home), binary)?;
    Ok(())
}

pub(crate) fn uninstall_from_home(home: &Path) -> Result<()> {
    uninstall_mcp_at(&config_path_for(home))?;
    uninstall_plugin_at(&plugin_path_for(home))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP server entry
// ---------------------------------------------------------------------------

fn install_mcp_at(path: &Path, binary: &str) -> Result<()> {
    let profile = capability_profile(HostKind::OpenCode);
    fs::create_dir_all(path.parent().unwrap())
        .with_context(|| format!("create dir {}", path.parent().unwrap().display()))?;

    let mut root: Value = if path.exists() {
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&text).unwrap_or(Value::Object(Map::new()))
    } else {
        Value::Object(Map::new())
    };

    let mcp = root
        .as_object_mut()
        .unwrap()
        .entry("mcp")
        .or_insert(Value::Object(Map::new()))
        .as_object_mut()
        .unwrap();

    mcp.insert(
        SERVER_NAME.to_string(),
        serde_json::json!({
            "type": "local",
            "command": [binary, "mcp"],
            "enabled": true
        }),
    );

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    println!(
        "{}: wrote MCP entry to {}",
        profile.display_name,
        path.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Uninstall
// ---------------------------------------------------------------------------

fn uninstall_mcp_at(path: &Path) -> Result<()> {
    let profile = capability_profile(HostKind::OpenCode);
    if !path.exists() {
        println!(
            "{}: config not found, nothing to remove",
            profile.display_name
        );
        return Ok(());
    }

    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut root: Value = serde_json::from_str(&text).unwrap_or(Value::Object(Map::new()));

    if let Some(mcp) = root
        .as_object_mut()
        .and_then(|o| o.get_mut("mcp"))
        .and_then(|v| v.as_object_mut())
    {
        mcp.remove(SERVER_NAME);
    }

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    println!(
        "{}: removed MCP entry from {}",
        profile.display_name,
        path.display()
    );
    Ok(())
}

fn uninstall_plugin_at(path: &Path) -> Result<()> {
    let profile = capability_profile(HostKind::OpenCode);
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("remove {}", path.display()))?;
        println!(
            "{}: removed plugin {}",
            profile.display_name,
            path.display()
        );
    } else {
        println!(
            "{}: plugin not found, nothing to remove",
            profile.display_name
        );
    }
    Ok(())
}

fn install_plugin_at(path: &Path, binary: &str) -> Result<()> {
    let profile = capability_profile(HostKind::OpenCode);
    fs::create_dir_all(path.parent().unwrap())
        .with_context(|| format!("create dir {}", path.parent().unwrap().display()))?;

    let source = render_plugin_source(binary)?;
    fs::write(path, source).with_context(|| format!("write {}", path.display()))?;
    println!(
        "{}: wrote plugin to {}",
        profile.display_name,
        path.display()
    );
    Ok(())
}

fn render_plugin_source(binary: &str) -> Result<String> {
    let profile = capability_profile(HostKind::OpenCode);
    let escaped_binary = binary.replace('\\', "\\\\").replace('"', "\\\"");
    let tool_prefix = profile.self_tool_naming.tool_prefix;
    let session_id_paths = serde_json::to_string(profile.identity_paths.session_id_paths)?;
    let native_read_tool_names = serde_json::to_string(&NATIVE_READ_TOOL_NAMES)?;
    let native_search_tool_names = serde_json::to_string(&NATIVE_SEARCH_TOOL_NAMES)?;
    let native_shell_tool_names = serde_json::to_string(&NATIVE_SHELL_TOOL_NAMES)?;
    Ok(include_str!("so-context.ts")
        .replace("__SO_CONTEXT_BINARY__", &escaped_binary)
        .replace("__SO_CONTEXT_TOOL_PREFIX__", tool_prefix)
        .replace("__SO_CONTEXT_SESSION_ID_PATHS__", &session_id_paths)
        .replace(
            "__SO_CONTEXT_NATIVE_READ_TOOL_NAMES__",
            &native_read_tool_names,
        )
        .replace(
            "__SO_CONTEXT_NATIVE_SEARCH_TOOL_NAMES__",
            &native_search_tool_names,
        )
        .replace(
            "__SO_CONTEXT_NATIVE_SHELL_TOOL_NAMES__",
            &native_shell_tool_names,
        ))
}

fn config_dir_for(home: &Path) -> PathBuf {
    home.join(".config").join("opencode")
}

fn config_path_for(home: &Path) -> PathBuf {
    config_dir_for(home).join("opencode.json")
}

fn plugin_path_for(home: &Path) -> PathBuf {
    config_dir_for(home).join("plugins").join("so-context.ts")
}

#[cfg(test)]
#[path = "opencode_tests.rs"]
mod tests;
