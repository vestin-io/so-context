//! Installs/uninstalls so-context as an MCP server into the global config files
//! of supported AI agent CLIs: Claude Code, OpenCode, and Codex CLI.

pub mod claude;
pub mod codex;
mod instructions;
pub mod opencode;

use anyhow::Result;

/// Installs the so-context MCP server entry in all supported agent configs.
pub fn install_all(binary: &str) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    if let Err(e) = claude::install(binary) {
        errors.push(format!("Claude: {e:#}"));
    }
    if let Err(e) = opencode::install(binary) {
        errors.push(format!("OpenCode: {e:#}"));
    }
    if let Err(e) = codex::install(binary) {
        errors.push(format!("Codex: {e:#}"));
    }

    finish("Setup", errors)
}

/// Removes all so-context entries from supported agent configs.
pub fn uninstall_all(binary: &str) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    if let Err(e) = claude::uninstall(binary) {
        errors.push(format!("Claude: {e:#}"));
    }
    if let Err(e) = opencode::uninstall() {
        errors.push(format!("OpenCode: {e:#}"));
    }
    if let Err(e) = codex::uninstall() {
        errors.push(format!("Codex: {e:#}"));
    }

    finish("Uninstall", errors)
}

fn finish(op: &str, errors: Vec<String>) -> Result<()> {
    if errors.is_empty() {
        println!("{op} complete. Restart your agent to pick up the changes.");
        Ok(())
    } else {
        for e in &errors {
            eprintln!("  warning: {e}");
        }
        println!("{op} finished with warnings (see above).");
        Ok(())
    }
}
