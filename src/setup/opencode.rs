//! Installs/uninstalls so-context into OpenCode's global config.
//!
//! Writes:
//!   - MCP server entry in `~/.config/opencode/opencode.json`
//!   - Plugin file at `~/.config/opencode/plugins/so-context.ts`
//!   - Managed instructions entry in `~/.config/opencode/opencode.json`

use super::instructions;
use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::host_adapter::{HostKind, capability_profile};
use crate::shell::NATIVE_SHELL_TOOL_NAMES;

const SERVER_NAME: &str = "so-context";
const PERMISSION_BACKUP_MARKER: &str = "__SO_CONTEXT_PERMISSION_BACKUP__=";
const MANAGED_PERMISSION_READ_KEY: &str = "read";
const MANAGED_PERMISSION_GREP_KEY: &str = "grep";

pub(crate) fn install_into_home(home: &Path, binary: &str) -> Result<()> {
    let plugin_path = plugin_path_for(home);
    let legacy_permission_backup = read_permission_backup_from_plugin(&plugin_path)?;
    install_mcp_at(
        &config_path_for(home),
        binary,
        legacy_permission_backup.as_ref(),
    )?;
    install_plugin_at(&plugin_path, binary)?;
    Ok(())
}

pub(crate) fn uninstall_from_home(home: &Path) -> Result<()> {
    let plugin_path = plugin_path_for(home);
    let legacy_permission_backup = read_permission_backup_from_plugin(&plugin_path)?;
    uninstall_mcp_at(&config_path_for(home), legacy_permission_backup.as_ref())?;
    uninstall_plugin_at(&plugin_path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP server entry
// ---------------------------------------------------------------------------

fn install_mcp_at(
    path: &Path,
    binary: &str,
    legacy_permission_backup: Option<&Value>,
) -> Result<()> {
    let profile = capability_profile(HostKind::OpenCode);
    fs::create_dir_all(path.parent().unwrap())
        .with_context(|| format!("create dir {}", path.parent().unwrap().display()))?;

    let mut root: Value = if path.exists() {
        let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&text).unwrap_or(Value::Object(Map::new()))
    } else {
        Value::Object(Map::new())
    };

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
    restore_legacy_managed_permissions_if_unchanged(object, legacy_permission_backup);
    install_instructions(object);

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

fn uninstall_mcp_at(path: &Path, legacy_permission_backup: Option<&Value>) -> Result<()> {
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
        restore_legacy_managed_permissions_if_unchanged(object, legacy_permission_backup);
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
    let native_shell_tool_names = serde_json::to_string(&NATIVE_SHELL_TOOL_NAMES)?;
    Ok(include_str!("so-context.ts")
        .replace("__SO_CONTEXT_BINARY__", &escaped_binary)
        .replace("__SO_CONTEXT_TOOL_PREFIX__", tool_prefix)
        .replace("__SO_CONTEXT_SESSION_ID_PATHS__", &session_id_paths)
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

fn restore_legacy_managed_permissions_if_unchanged(
    root: &mut Map<String, Value>,
    legacy_permission_backup: Option<&Value>,
) {
    if legacy_permission_backup.is_none() {
        remove_pure_legacy_managed_permissions(root);
        return;
    }

    let Some(previous_permission) = legacy_permission_backup else {
        return;
    };

    let current_permission = root.get("permission").cloned().unwrap_or(Value::Null);
    if !current_permission_matches_legacy_managed(&current_permission, previous_permission) {
        return;
    }

    if previous_permission.is_null() {
        root.remove("permission");
        return;
    }

    root.insert("permission".to_string(), previous_permission.clone());
}

fn remove_pure_legacy_managed_permissions(root: &mut Map<String, Value>) {
    let Some(permission) = root.get("permission").and_then(Value::as_object) else {
        return;
    };

    if permission.len() != 2 {
        return;
    }

    let has_read =
        permission.get(MANAGED_PERMISSION_READ_KEY) == Some(&Value::String("ask".to_string()));
    let has_grep =
        permission.get(MANAGED_PERMISSION_GREP_KEY) == Some(&Value::String("ask".to_string()));

    if has_read && has_grep {
        root.remove("permission");
    }
}

fn current_permission_matches_legacy_managed(
    current_permission: &Value,
    previous_permission: &Value,
) -> bool {
    let Some(current) = current_permission.as_object() else {
        return false;
    };

    match previous_permission {
        Value::Null => {
            current.len() == 2
                && current.get(MANAGED_PERMISSION_READ_KEY)
                    == Some(&Value::String("ask".to_string()))
                && current.get(MANAGED_PERMISSION_GREP_KEY)
                    == Some(&Value::String("ask".to_string()))
        }
        Value::String(action) => {
            current.len() == 3
                && current.get("*") == Some(&Value::String(action.clone()))
                && current.get(MANAGED_PERMISSION_READ_KEY)
                    == Some(&Value::String("ask".to_string()))
                && current.get(MANAGED_PERMISSION_GREP_KEY)
                    == Some(&Value::String("ask".to_string()))
        }
        Value::Object(previous) => {
            if current.get(MANAGED_PERMISSION_READ_KEY) != Some(&Value::String("ask".to_string()))
                || current.get(MANAGED_PERMISSION_GREP_KEY)
                    != Some(&Value::String("ask".to_string()))
            {
                return false;
            }

            let mut current_without_managed = current.clone();
            current_without_managed.remove(MANAGED_PERMISSION_READ_KEY);
            current_without_managed.remove(MANAGED_PERMISSION_GREP_KEY);
            current_without_managed == *previous
        }
        _ => false,
    }
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
