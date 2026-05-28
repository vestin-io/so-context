//! Installs so-context as an MCP server in Codex CLI's global config.toml.
//!
//! Config path: ~/.codex/config.toml
//! MCP section: [mcp_servers.so-context]
//!              command = "..."
//!              args = ["mcp"]

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

    // Ensure [mcp_servers] table exists.
    if doc.get("mcp_servers").is_none() {
        doc["mcp_servers"] = Item::Table(Table::new());
    }
    let mcp_servers = doc["mcp_servers"]
        .as_table_mut()
        .context("[mcp_servers] is not a table")?;

    let mut server_table = Table::new();
    server_table["command"] = value(binary);
    let mut args = Array::new();
    args.push("mcp");
    server_table["args"] = value(args);

    mcp_servers[SERVER_NAME] = Item::Table(server_table);

    fs::write(&path, doc.to_string())
        .with_context(|| format!("write {}", path.display()))?;
    println!("Codex: wrote MCP entry to {}", path.display());
    Ok(())
}
