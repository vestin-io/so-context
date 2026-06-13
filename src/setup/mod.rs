//! Installs/uninstalls so-context as an MCP server into the global config files
//! of supported AI agent CLIs: Claude Code, OpenCode, and Codex CLI.

pub mod claude;
pub mod codex;
mod hook_binding;
mod instructions;
pub mod opencode;
mod renderer;

use anyhow::Result;
use renderer::all_renderers;

/// Installs the so-context MCP server entry in all supported agent configs.
pub fn install_all(binary: &str) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    for renderer in all_renderers(binary) {
        if let Err(e) = renderer.install() {
            errors.push(format!("{}: {e:#}", renderer.host_kind().as_str()));
        }
    }

    finish("Setup", errors)
}

/// Removes all so-context entries from supported agent configs.
pub fn uninstall_all(binary: &str) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    for renderer in all_renderers(binary) {
        if let Err(e) = renderer.uninstall() {
            errors.push(format!("{}: {e:#}", renderer.host_kind().as_str()));
        }
    }

    finish("Uninstall", errors)
}

#[cfg(test)]
pub(crate) fn install_all_for_home(home: &std::path::Path, binary: &str) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    for renderer in renderer::all_renderers_for_home(home, binary) {
        if let Err(e) = renderer.install() {
            errors.push(format!("{}: {e:#}", renderer.host_kind().as_str()));
        }
    }

    finish("Setup", errors)
}

#[cfg(test)]
pub(crate) fn uninstall_all_for_home(home: &std::path::Path, binary: &str) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    for renderer in renderer::all_renderers_for_home(home, binary) {
        if let Err(e) = renderer.uninstall() {
            errors.push(format!("{}: {e:#}", renderer.host_kind().as_str()));
        }
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
