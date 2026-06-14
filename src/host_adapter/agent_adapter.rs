use serde_json::{Map, Value};

use crate::host_adapter::profile::HostCapabilityProfile;
use crate::host_adapter::{HostKind, ToolNamespace, capability_profile};
use crate::routing_session::{NormalizedToolCall, SessionService};

pub trait AgentAdapter {
    fn profile(&self) -> HostCapabilityProfile;

    fn host_kind(&self) -> HostKind {
        self.profile().kind
    }

    fn normalize_pre_tool_call(
        &self,
        session_service: &SessionService,
        input: &Value,
    ) -> NormalizedToolCall {
        self.normalize_tool_call(
            session_service,
            self.standard_tool_name(input),
            self.standard_tool_input(input),
            self.identity_payload(input),
        )
    }

    fn normalize_tool_call(
        &self,
        session_service: &SessionService,
        tool_name: &str,
        tool_input: Map<String, Value>,
        identity_payload: &Value,
    ) -> NormalizedToolCall {
        NormalizedToolCall {
            host_kind: self.host_kind(),
            tool_namespace: ToolNamespace::for_host(self.host_kind(), tool_name),
            tool_name: tool_name.to_string(),
            tool_input,
            identity: session_service.identity_from_hook_input(self.host_kind(), identity_payload),
        }
    }

    fn standard_tool_name<'a>(&self, input: &'a Value) -> &'a str {
        input
            .get("tool_name")
            .and_then(|value| value.as_str())
            .unwrap_or("")
    }

    fn standard_tool_input(&self, input: &Value) -> Map<String, Value> {
        input
            .get("tool_input")
            .and_then(|value| value.as_object())
            .cloned()
            .unwrap_or_default()
    }

    fn identity_payload<'a>(&self, input: &'a Value) -> &'a Value {
        input
    }

    #[cfg(test)]
    fn normalize_plugin_tool_call(
        &self,
        session_service: &SessionService,
        input: &Value,
        output_args: &Map<String, Value>,
    ) -> NormalizedToolCall {
        let tool_name = input
            .get("tool")
            .and_then(|value| value.as_str())
            .unwrap_or("");

        self.normalize_tool_call(session_service, tool_name, output_args.clone(), input)
    }

    fn compact_reset_ids(
        &self,
        session_service: &SessionService,
        input: &Value,
    ) -> Option<(String, String)> {
        session_service.compact_reset_ids(self.host_kind(), input)
    }
}

pub struct CodexAdapter;
pub struct ClaudeAdapter;
pub struct OpenCodeAdapter;
pub struct UnknownAdapter;

impl AgentAdapter for CodexAdapter {
    fn profile(&self) -> HostCapabilityProfile {
        capability_profile(HostKind::Codex)
    }
}

impl AgentAdapter for ClaudeAdapter {
    fn profile(&self) -> HostCapabilityProfile {
        capability_profile(HostKind::Claude)
    }
}

impl AgentAdapter for OpenCodeAdapter {
    fn profile(&self) -> HostCapabilityProfile {
        capability_profile(HostKind::OpenCode)
    }
}

impl AgentAdapter for UnknownAdapter {
    fn profile(&self) -> HostCapabilityProfile {
        capability_profile(HostKind::Unknown)
    }
}

static CODEX_ADAPTER: CodexAdapter = CodexAdapter;
static CLAUDE_ADAPTER: ClaudeAdapter = ClaudeAdapter;
static OPENCODE_ADAPTER: OpenCodeAdapter = OpenCodeAdapter;
static UNKNOWN_ADAPTER: UnknownAdapter = UnknownAdapter;

pub fn adapter_for_host(host_kind: HostKind) -> &'static dyn AgentAdapter {
    match host_kind {
        HostKind::Codex => &CODEX_ADAPTER,
        HostKind::Claude => &CLAUDE_ADAPTER,
        HostKind::OpenCode => &OPENCODE_ADAPTER,
        HostKind::Unknown => &UNKNOWN_ADAPTER,
    }
}

#[cfg(test)]
#[path = "agent_adapter_tests.rs"]
mod tests;
