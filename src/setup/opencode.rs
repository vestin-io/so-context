//! Installs/uninstalls so-context into OpenCode's global config.
//!
//! Writes:
//!   - MCP server entry in `~/.config/opencode/opencode.json`
//!   - Plugin file at `~/.config/opencode/plugins/so-context.ts`
//!   - Managed instructions entry in `~/.config/opencode/opencode.json`
//!   - Managed top-level permission overrides for native read/grep

use super::instructions;
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::host_adapter::{HostKind, capability_profile};
use crate::routing_session::{NATIVE_READ_TOOL_NAMES, NATIVE_SEARCH_TOOL_NAMES};
use crate::shell::NATIVE_SHELL_TOOL_NAMES;

const SERVER_NAME: &str = "so-context";
const PERMISSION_BACKUP_MARKER: &str = "__SO_CONTEXT_PERMISSION_BACKUP__=";
const MANAGED_PERMISSION_READ_KEY: &str = "read";
const MANAGED_PERMISSION_GREP_KEY: &str = "grep";

pub(crate) fn install_into_home(home: &Path, binary: &str) -> Result<()> {
    let permission_backup = install_mcp_at(&config_path_for(home), binary)?;
    install_plugin_at(&plugin_path_for(home), binary, &permission_backup)?;
    Ok(())
}

pub(crate) fn uninstall_from_home(home: &Path) -> Result<()> {
    let plugin_path = plugin_path_for(home);
    let permission_backup = read_permission_backup_from_plugin(&plugin_path)?;
    uninstall_mcp_at(&config_path_for(home), permission_backup.as_ref())?;
    uninstall_plugin_at(&plugin_path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP server entry
// ---------------------------------------------------------------------------

fn install_mcp_at(path: &Path, binary: &str) -> Result<Value> {
    let profile = capability_profile(HostKind::OpenCode);
    fs::create_dir_all(path.parent().unwrap())
        .with_context(|| format!("create dir {}", path.parent().unwrap().display()))?;

    let mut root: Value = if path.exists() {
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&text).unwrap_or(Value::Object(Map::new()))
    } else {
        Value::Object(Map::new())
    };
    let previous_permission = root.get("permission").cloned().unwrap_or(Value::Null);

    {
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
    }

    let object = root.as_object_mut().unwrap();
    install_instructions(object);
    install_managed_permissions(object, &previous_permission);

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    println!(
        "{}: wrote MCP entry to {}",
        profile.display_name,
        path.display()
    );
    Ok(previous_permission)
}

// ---------------------------------------------------------------------------
// Uninstall
// ---------------------------------------------------------------------------

fn uninstall_mcp_at(path: &Path, permission_backup: Option<&Value>) -> Result<()> {
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

    if let Some(object) = root.as_object_mut() {
        uninstall_instructions(object);
        restore_permissions_if_unchanged(object, permission_backup);
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

fn install_plugin_at(path: &Path, binary: &str, permission_backup: &Value) -> Result<()> {
    let profile = capability_profile(HostKind::OpenCode);
    fs::create_dir_all(path.parent().unwrap())
        .with_context(|| format!("create dir {}", path.parent().unwrap().display()))?;

    let source = render_plugin_source(binary, permission_backup)?;
    fs::write(path, source).with_context(|| format!("write {}", path.display()))?;
    println!(
        "{}: wrote plugin to {}",
        profile.display_name,
        path.display()
    );
    Ok(())
}

fn render_plugin_source(binary: &str, permission_backup: &Value) -> Result<String> {
    let profile = capability_profile(HostKind::OpenCode);
    let escaped_binary = binary.replace('\\', "\\\\").replace('"', "\\\"");
    let tool_prefix = profile.self_tool_naming.tool_prefix;
    let session_id_paths = serde_json::to_string(profile.identity_paths.session_id_paths)?;
    let native_read_tool_names = serde_json::to_string(&NATIVE_READ_TOOL_NAMES)?;
    let native_search_tool_names = serde_json::to_string(&NATIVE_SEARCH_TOOL_NAMES)?;
    let native_shell_tool_names = serde_json::to_string(&NATIVE_SHELL_TOOL_NAMES)?;
    let escaped_permission_backup = serde_json::to_string(permission_backup)?;
    Ok(include_str!("so-context.ts")
        .replace("__SO_CONTEXT_BINARY__", &escaped_binary)
        .replace("__SO_CONTEXT_TOOL_PREFIX__", tool_prefix)
        .replace("__SO_CONTEXT_SESSION_ID_PATHS__", &session_id_paths)
        .replace("__SO_CONTEXT_PERMISSION_BACKUP_JSON__", &escaped_permission_backup)
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

fn install_managed_permissions(root: &mut Map<String, Value>, previous_permission: &Value) {
    root.insert(
        "permission".to_string(),
        managed_permission_value(previous_permission),
    );
}

fn managed_permission_value(previous_permission: &Value) -> Value {
    match previous_permission {
        Value::Object(existing) => {
            let mut permission = existing.clone();
            permission.insert(
                MANAGED_PERMISSION_READ_KEY.to_string(),
                Value::String("ask".to_string()),
            );
            permission.insert(
                MANAGED_PERMISSION_GREP_KEY.to_string(),
                Value::String("ask".to_string()),
            );
            Value::Object(permission)
        }
        Value::String(action) => {
            let mut permission = Map::new();
            permission.insert("*".to_string(), Value::String(action.clone()));
            permission.insert(
                MANAGED_PERMISSION_READ_KEY.to_string(),
                Value::String("ask".to_string()),
            );
            permission.insert(
                MANAGED_PERMISSION_GREP_KEY.to_string(),
                Value::String("ask".to_string()),
            );
            Value::Object(permission)
        }
        _ => serde_json::json!({
            "read": "ask",
            "grep": "ask"
        }),
    }
}

fn restore_permissions_if_unchanged(root: &mut Map<String, Value>, permission_backup: Option<&Value>) {
    let Some(previous_permission) = permission_backup else {
        return;
    };

    let current_permission = root.get("permission").cloned().unwrap_or(Value::Null);
    if current_permission != managed_permission_value(previous_permission) {
        return;
    }

    if previous_permission.is_null() {
        root.remove("permission");
        return;
    }

    root.insert("permission".to_string(), previous_permission.clone());
}

fn install_instructions(root: &mut Map<String, Value>) {
    let managed = instructions::opencode_instructions_body();
    let instructions = root
        .entry("instructions")
        .or_insert_with(|| Value::Array(Vec::new()));

    let Some(items) = instructions.as_array_mut() else {
        *instructions = Value::Array(vec![Value::String(managed)]);
        return;
    };

    items.retain(|item| !is_managed_instruction(item));
    items.push(Value::String(managed));
}

fn uninstall_instructions(root: &mut Map<String, Value>) {
    let Some(items) = root.get_mut("instructions").and_then(Value::as_array_mut) else {
        return;
    };

    items.retain(|item| !is_managed_instruction(item));
}

fn is_managed_instruction(item: &Value) -> bool {
    item.as_str()
        .map(|value| value.starts_with("## so-context\n\n"))
        .unwrap_or(false)
}

fn read_permission_backup_from_plugin(path: &Path) -> Result<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }

    let source = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Some(line) = source
        .lines()
        .find(|line| line.contains(PERMISSION_BACKUP_MARKER))
    else {
        return Ok(None);
    };

    let Some(raw_backup) = line.split(PERMISSION_BACKUP_MARKER).nth(1) else {
        return Ok(None);
    };

    let backup = serde_json::from_str(raw_backup.trim())
        .with_context(|| format!("parse permission backup from {}", path.display()))?;
    Ok(Some(backup))
}

#[cfg(test)]
#[path = "opencode_tests.rs"]
mod tests;
