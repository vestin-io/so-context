use crate::host_adapter::HostKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookMatcherStyle {
    RegexPrefix,
    GlobPrefix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelfToolNaming {
    pub tool_prefix: &'static str,
    pub hook_matcher_style: HookMatcherStyle,
}

impl SelfToolNaming {
    pub fn tool_name(&self, tool_basename: &str) -> String {
        format!("{}{}", self.tool_prefix, tool_basename)
    }

    pub fn matches(&self, tool_name: &str) -> bool {
        tool_name.starts_with(self.tool_prefix)
    }

    pub fn hook_matcher(&self) -> String {
        match self.hook_matcher_style {
            HookMatcherStyle::RegexPrefix => format!("{}.*", self.tool_prefix),
            HookMatcherStyle::GlobPrefix => format!("{}*", self.tool_prefix),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityPathProfile {
    pub session_id_paths: &'static [&'static str],
    pub agent_id_paths: &'static [&'static str],
    pub connection_id_paths: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostCapabilityProfile {
    pub kind: HostKind,
    pub display_name: &'static str,
    pub self_tool_naming: SelfToolNaming,
    pub identity_paths: IdentityPathProfile,
}

const MCP_QUALIFIED_SELF_TOOL_NAMING: SelfToolNaming = SelfToolNaming {
    tool_prefix: "mcp__so-context__",
    hook_matcher_style: HookMatcherStyle::RegexPrefix,
};

const OPENCODE_SELF_TOOL_NAMING: SelfToolNaming = SelfToolNaming {
    tool_prefix: "so-context_",
    hook_matcher_style: HookMatcherStyle::GlobPrefix,
};

const STANDARD_HOOK_IDENTITY_PATHS: IdentityPathProfile = IdentityPathProfile {
    session_id_paths: &["session_id"],
    agent_id_paths: &["agent_id"],
    connection_id_paths: &["connection_id"],
};

const OPENCODE_IDENTITY_PATHS: IdentityPathProfile = IdentityPathProfile {
    session_id_paths: &["session.id", "sessionID"],
    agent_id_paths: &[],
    connection_id_paths: &[],
};

const CODEX_PROFILE: HostCapabilityProfile = HostCapabilityProfile {
    kind: HostKind::Codex,
    display_name: "Codex",
    self_tool_naming: MCP_QUALIFIED_SELF_TOOL_NAMING,
    identity_paths: STANDARD_HOOK_IDENTITY_PATHS,
};

const CLAUDE_PROFILE: HostCapabilityProfile = HostCapabilityProfile {
    kind: HostKind::Claude,
    display_name: "Claude",
    self_tool_naming: MCP_QUALIFIED_SELF_TOOL_NAMING,
    identity_paths: STANDARD_HOOK_IDENTITY_PATHS,
};

const OPENCODE_PROFILE: HostCapabilityProfile = HostCapabilityProfile {
    kind: HostKind::OpenCode,
    display_name: "OpenCode",
    self_tool_naming: OPENCODE_SELF_TOOL_NAMING,
    identity_paths: OPENCODE_IDENTITY_PATHS,
};

const UNKNOWN_PROFILE: HostCapabilityProfile = HostCapabilityProfile {
    kind: HostKind::Unknown,
    display_name: "Unknown",
    self_tool_naming: MCP_QUALIFIED_SELF_TOOL_NAMING,
    identity_paths: STANDARD_HOOK_IDENTITY_PATHS,
};

pub fn capability_profile(kind: HostKind) -> HostCapabilityProfile {
    match kind {
        HostKind::Codex => CODEX_PROFILE,
        HostKind::Claude => CLAUDE_PROFILE,
        HostKind::OpenCode => OPENCODE_PROFILE,
        HostKind::Unknown => UNKNOWN_PROFILE,
    }
}

#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
