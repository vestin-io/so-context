//! Installs/uninstalls so-context into OpenCode's global config.
//!
//! Writes:
//!   - MCP server entry in `~/.config/opencode/opencode.json`
//!   - Plugin file at `~/.config/opencode/plugins/so-context.ts`

use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;

const SERVER_NAME: &str = "so-context";

pub fn config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config").join("opencode")
}

pub fn config_path() -> PathBuf {
    config_dir().join("opencode.json")
}

pub fn plugin_path() -> PathBuf {
    config_dir().join("plugins").join("so-context.ts")
}

pub fn install(binary: &str) -> Result<()> {
    install_mcp(binary)?;
    install_plugin(binary)?;
    Ok(())
}

pub fn uninstall() -> Result<()> {
    uninstall_mcp()?;
    uninstall_plugin()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// MCP server entry
// ---------------------------------------------------------------------------

fn install_mcp(binary: &str) -> Result<()> {
    let path = config_path();
    fs::create_dir_all(path.parent().unwrap())
        .with_context(|| format!("create dir {}", path.parent().unwrap().display()))?;

    let mut root: Value = if path.exists() {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read {}", path.display()))?;
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
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    println!("OpenCode: wrote MCP entry to {}", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// Uninstall
// ---------------------------------------------------------------------------

fn uninstall_mcp() -> Result<()> {
    let path = config_path();
    if !path.exists() {
        println!("OpenCode: config not found, nothing to remove");
        return Ok(());
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    let mut root: Value = serde_json::from_str(&text).unwrap_or(Value::Object(Map::new()));

    if let Some(mcp) = root.as_object_mut()
        .and_then(|o| o.get_mut("mcp"))
        .and_then(|v| v.as_object_mut())
    {
        mcp.remove(SERVER_NAME);
    }

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    println!("OpenCode: removed MCP entry from {}", path.display());
    Ok(())
}

fn uninstall_plugin() -> Result<()> {
    let path = plugin_path();
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
        println!("OpenCode: removed plugin {}", path.display());
    } else {
        println!("OpenCode: plugin not found, nothing to remove");
    }
    Ok(())
}

fn install_plugin(_binary: &str) -> Result<()> {
    let path = plugin_path();
    fs::create_dir_all(path.parent().unwrap())
        .with_context(|| format!("create dir {}", path.parent().unwrap().display()))?;

    let source = include_str!("so-context.ts");
    fs::write(&path, source).with_context(|| format!("write {}", path.display()))?;
    println!("OpenCode: wrote plugin to {}", path.display());
    Ok(())
}
