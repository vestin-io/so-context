//! Installs so-context as an MCP server in OpenCode's global config.
//!
//! Config path: ~/.config/opencode/opencode.json
//! MCP section: "mcp": { "<name>": { "type": "local", "command": [...] } }

use anyhow::{Context, Result};
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;

const SERVER_NAME: &str = "so-context";

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".config")
        .join("opencode")
        .join("opencode.json")
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
            "command": [binary, "daemon"],
            "enabled": true
        }),
    );

    let text = serde_json::to_string_pretty(&root)?;
    fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    println!("OpenCode: wrote MCP entry to {}", path.display());
    Ok(())
}
