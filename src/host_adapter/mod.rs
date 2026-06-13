mod agent_adapter;
mod profile;

use crate::routing_session::{NATIVE_READ_TOOL_NAMES, NATIVE_SEARCH_TOOL_NAMES};
use crate::shell::NATIVE_SHELL_TOOL_NAMES;
pub(crate) use agent_adapter::adapter_for_host;
pub(crate) use profile::capability_profile;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostKind {
    Codex,
    Claude,
    OpenCode,
    Unknown,
}

impl HostKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolNamespace {
    Native,
    SoContextMcp,
    OtherMcp,
}

impl ToolNamespace {
    pub fn from_tool_name(tool_name: &str) -> Self {
        if tool_name.starts_with("mcp__so-context__") || tool_name.starts_with("so-context_") {
            Self::SoContextMcp
        } else if tool_name.starts_with("mcp__") {
            Self::OtherMcp
        } else {
            Self::Native
        }
    }

    pub fn for_host(host_kind: HostKind, tool_name: &str) -> Self {
        if host_kind == HostKind::Unknown {
            return Self::from_tool_name(tool_name);
        }

        let profile = capability_profile(host_kind);
        if profile.self_tool_naming.matches(tool_name) {
            return Self::SoContextMcp;
        }

        match host_kind {
            HostKind::OpenCode => {
                if is_known_native_tool_name(tool_name) {
                    Self::Native
                } else if tool_name.contains('_') || tool_name.starts_with("mcp__") {
                    Self::OtherMcp
                } else {
                    Self::Native
                }
            }
            _ => {
                if tool_name.starts_with("mcp__") {
                    Self::OtherMcp
                } else {
                    Self::Native
                }
            }
        }
    }
}

fn is_known_native_tool_name(tool_name: &str) -> bool {
    NATIVE_READ_TOOL_NAMES.contains(&tool_name)
        || NATIVE_SEARCH_TOOL_NAMES.contains(&tool_name)
        || NATIVE_SHELL_TOOL_NAMES.contains(&tool_name)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
