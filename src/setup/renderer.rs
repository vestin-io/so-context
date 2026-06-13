use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::host_adapter::HostKind;

use super::{claude, codex, opencode};

pub trait SetupRenderer {
    fn host_kind(&self) -> HostKind;
    fn install(&self) -> Result<()>;
    fn uninstall(&self) -> Result<()>;
}

pub struct ClaudeSetupRenderer {
    home: PathBuf,
    binary: String,
}

pub struct CodexSetupRenderer {
    home: PathBuf,
    binary: String,
}

pub struct OpenCodeSetupRenderer {
    home: PathBuf,
    binary: String,
}

impl SetupRenderer for ClaudeSetupRenderer {
    fn host_kind(&self) -> HostKind {
        HostKind::Claude
    }

    fn install(&self) -> Result<()> {
        claude::install_into_home(&self.home, &self.binary)
    }

    fn uninstall(&self) -> Result<()> {
        claude::uninstall_from_home(&self.home, &self.binary)
    }
}

impl SetupRenderer for CodexSetupRenderer {
    fn host_kind(&self) -> HostKind {
        HostKind::Codex
    }

    fn install(&self) -> Result<()> {
        codex::install_into_home(&self.home, &self.binary)
    }

    fn uninstall(&self) -> Result<()> {
        codex::uninstall_from_home(&self.home, &self.binary)
    }
}

impl SetupRenderer for OpenCodeSetupRenderer {
    fn host_kind(&self) -> HostKind {
        HostKind::OpenCode
    }

    fn install(&self) -> Result<()> {
        opencode::install_into_home(&self.home, &self.binary)
    }

    fn uninstall(&self) -> Result<()> {
        opencode::uninstall_from_home(&self.home)
    }
}

pub fn all_renderers(binary: &str) -> Vec<Box<dyn SetupRenderer>> {
    all_renderers_for_home(&super::instructions::home_dir(), binary)
}

pub fn all_renderers_for_home(home: &Path, binary: &str) -> Vec<Box<dyn SetupRenderer>> {
    vec![
        Box::new(ClaudeSetupRenderer {
            home: home.to_path_buf(),
            binary: binary.to_string(),
        }),
        Box::new(OpenCodeSetupRenderer {
            home: home.to_path_buf(),
            binary: binary.to_string(),
        }),
        Box::new(CodexSetupRenderer {
            home: home.to_path_buf(),
            binary: binary.to_string(),
        }),
    ]
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod tests;
