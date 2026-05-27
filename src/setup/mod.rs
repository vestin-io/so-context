//! Installs so-context as an MCP server into the global config files of
//! supported AI agent CLIs: Claude Code, OpenCode, and Codex CLI.

pub mod claude;
pub mod codex;
pub mod opencode;

use anyhow::Result;

/// Installs the so-context MCP server entry in all supported agent configs.
///
/// `binary` is the path to the `so-context` executable (e.g. from
/// `std::env::current_exe()`).
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

    if errors.is_empty() {
        println!("Setup complete. Restart your agent to pick up the new MCP server.");
        Ok(())
    } else {
        for e in &errors {
            eprintln!("  warning: {e}");
        }
        // Non-fatal: partial success is acceptable.
        println!("Setup finished with warnings (see above).");
        Ok(())
    }
}
