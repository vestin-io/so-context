use serde_json::{Map, Value};

use crate::host_adapter::{HostKind, ToolNamespace};
use crate::routing_session::{
    NATIVE_READ_TOOL_NAMES, NATIVE_SEARCH_TOOL_NAMES, NormalizedToolCall,
    SIMPLE_NATIVE_SEARCH_KEYS, SessionService,
};
use crate::shell::NATIVE_SHELL_TOOL_NAMES;

pub trait AgentAdapter {
    fn host_kind(&self) -> HostKind;

    fn normalize_pre_tool_call(
        &self,
        session_service: &SessionService,
        input: &Value,
    ) -> NormalizedToolCall;

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

fn normalize_tool_call(
    host_kind: HostKind,
    session_service: &SessionService,
    tool_name: &str,
    tool_input: Map<String, Value>,
    identity_payload: &Value,
) -> NormalizedToolCall {
    NormalizedToolCall {
        host_kind,
        tool_namespace: ToolNamespace::for_host(host_kind, tool_name),
        tool_name: tool_name.to_string(),
        routing_input: canonical_routing_input(tool_name, &tool_input),
        tool_input,
        identity: session_service.identity_from_hook_input(host_kind, identity_payload),
    }
}

fn normalize_standard_hook_call(
    host_kind: HostKind,
    session_service: &SessionService,
    input: &Value,
) -> NormalizedToolCall {
    let tool_name = input
        .get("tool_name")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let tool_input = input
        .get("tool_input")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();

    normalize_tool_call(host_kind, session_service, tool_name, tool_input, input)
}

#[cfg(test)]
pub(crate) fn normalize_plugin_tool_call(
    host_kind: HostKind,
    session_service: &SessionService,
    input: &Value,
    output_args: &Map<String, Value>,
) -> NormalizedToolCall {
    let tool_name = input
        .get("tool")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    normalize_tool_call(
        host_kind,
        session_service,
        tool_name,
        output_args.clone(),
        input,
    )
}

fn canonical_routing_input(tool_name: &str, tool_input: &Map<String, Value>) -> Map<String, Value> {
    if NATIVE_SHELL_TOOL_NAMES.contains(&tool_name) {
        let mut canonical = Map::new();
        if let Some(command) = tool_input
            .get("command")
            .or_else(|| tool_input.get("cmd"))
            .cloned()
        {
            canonical.insert("command".to_string(), command);
        }
        return canonical;
    }

    if NATIVE_READ_TOOL_NAMES.contains(&tool_name) {
        let mut canonical = Map::new();
        if let Some(path) = tool_input
            .get("path")
            .or_else(|| tool_input.get("file_path"))
            .cloned()
        {
            canonical.insert("path".to_string(), path);
        }
        copy_if_present(tool_input, &mut canonical, "mode", "mode");
        copy_if_present(tool_input, &mut canonical, "start_line", "start_line");
        copy_if_present(tool_input, &mut canonical, "end_line", "end_line");
        copy_if_present(tool_input, &mut canonical, "line_numbers", "line_numbers");
        return canonical;
    }

    if NATIVE_SEARCH_TOOL_NAMES.contains(&tool_name) {
        let mut canonical = Map::new();
        if let Some(query) = tool_input
            .get("query")
            .or_else(|| tool_input.get("pattern"))
            .cloned()
        {
            canonical.insert("query".to_string(), query);
        }
        if let Some(path) = tool_input
            .get("path")
            .or_else(|| tool_input.get("directory"))
            .or_else(|| tool_input.get("root"))
            .cloned()
        {
            canonical.insert("path".to_string(), path);
        }
        copy_if_present(tool_input, &mut canonical, "limit", "limit");
        if let Some(regex) = tool_input
            .get("regex")
            .or_else(|| tool_input.get("regexp"))
            .cloned()
        {
            canonical.insert("regex".to_string(), regex);
        }
        let has_extra_semantics = tool_input.keys().any(|key| {
            !SIMPLE_NATIVE_SEARCH_KEYS.contains(&key.as_str()) && key != "regex" && key != "regexp"
        });
        if has_extra_semantics {
            canonical.insert("_has_extra_search_semantics".to_string(), Value::Bool(true));
        }
        return canonical;
    }

    tool_input.clone()
}

fn copy_if_present(
    from: &Map<String, Value>,
    to: &mut Map<String, Value>,
    from_key: &str,
    to_key: &str,
) {
    if let Some(value) = from.get(from_key).cloned() {
        to.insert(to_key.to_string(), value);
    }
}

impl AgentAdapter for CodexAdapter {
    fn host_kind(&self) -> HostKind {
        HostKind::Codex
    }

    fn normalize_pre_tool_call(
        &self,
        session_service: &SessionService,
        input: &Value,
    ) -> NormalizedToolCall {
        normalize_standard_hook_call(self.host_kind(), session_service, input)
    }
}

impl AgentAdapter for ClaudeAdapter {
    fn host_kind(&self) -> HostKind {
        HostKind::Claude
    }

    fn normalize_pre_tool_call(
        &self,
        session_service: &SessionService,
        input: &Value,
    ) -> NormalizedToolCall {
        normalize_standard_hook_call(self.host_kind(), session_service, input)
    }
}

impl AgentAdapter for OpenCodeAdapter {
    fn host_kind(&self) -> HostKind {
        HostKind::OpenCode
    }

    fn normalize_pre_tool_call(
        &self,
        session_service: &SessionService,
        input: &Value,
    ) -> NormalizedToolCall {
        normalize_standard_hook_call(self.host_kind(), session_service, input)
    }
}

impl AgentAdapter for UnknownAdapter {
    fn host_kind(&self) -> HostKind {
        HostKind::Unknown
    }

    fn normalize_pre_tool_call(
        &self,
        session_service: &SessionService,
        input: &Value,
    ) -> NormalizedToolCall {
        normalize_standard_hook_call(self.host_kind(), session_service, input)
    }
}

pub fn adapter_for_host(host_kind: HostKind) -> Box<dyn AgentAdapter> {
    match host_kind {
        HostKind::Codex => Box::new(CodexAdapter),
        HostKind::Claude => Box::new(ClaudeAdapter),
        HostKind::OpenCode => Box::new(OpenCodeAdapter),
        HostKind::Unknown => Box::new(UnknownAdapter),
    }
}

#[cfg(test)]
#[path = "agent_adapter_tests.rs"]
mod tests;
