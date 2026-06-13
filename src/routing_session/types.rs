#[cfg(test)]
use serde_json::json;
use serde_json::{Map, Value};

use crate::host_adapter::{HostKind, ToolNamespace, capability_profile};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IdentityContext {
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub connection_id: Option<String>,
}

impl IdentityContext {
    pub fn tool_context_id(&self) -> Option<&str> {
        self.session_id
            .as_deref()
            .filter(|value| !value.is_empty())
            .or_else(|| self.agent_id.as_deref().filter(|value| !value.is_empty()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedToolCall {
    pub host_kind: HostKind,
    pub tool_namespace: ToolNamespace,
    pub tool_name: String,
    pub tool_input: Map<String, Value>,
    pub routing_input: Map<String, Value>,
    pub identity: IdentityContext,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RetryTool {
    Read,
    Search,
    Shell,
}

impl RetryTool {
    pub fn tool_basename(&self) -> &'static str {
        match self {
            Self::Read => "so_read",
            Self::Search => "so_search",
            Self::Shell => "so_shell",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RetryDirective {
    pub tool: RetryTool,
    pub argument_label: &'static str,
    pub arguments: Value,
    pub rationale: &'static str,
    pub usage_note: &'static str,
}

impl RetryDirective {
    pub fn native_read(arguments: Value) -> Self {
        Self {
            tool: RetryTool::Read,
            argument_label: "arguments",
            arguments,
            rationale: "I routed this native file read through {tool} so the file content stays attributable and reusable in shared project context",
            usage_note: "Use `mode: \"outline\"` when you only need structure instead of full file text.",
        }
    }

    pub fn native_search(arguments: Value) -> Self {
        Self {
            tool: RetryTool::Search,
            argument_label: "arguments",
            arguments,
            rationale: "I routed this native search through {tool} so the hits stay attributable and reusable in shared project context",
            usage_note: "Keep native grep-style tools only when you need raw grep semantics or the project is not indexed.",
        }
    }

    pub fn native_shell(arguments: Value) -> Self {
        Self {
            tool: RetryTool::Shell,
            argument_label: "argv",
            arguments,
            rationale: "I routed this short shell command through our context-aware {tool} tool to improve shared project context for the next steps",
            usage_note: "Keep the native shell only for long-running, streaming, or interactive commands.",
        }
    }

    pub fn render_tool_name(&self, host_kind: HostKind) -> String {
        capability_profile(host_kind)
            .self_tool_naming
            .tool_name(self.tool.tool_basename())
    }

    pub fn render_rationale(&self, host_kind: HostKind) -> String {
        let tool_name = self.render_tool_name(host_kind);
        self.rationale.replace("{tool}", &format!("`{tool_name}`"))
    }

    pub fn render_reason(&self, host_kind: HostKind) -> String {
        let rationale = self.render_rationale(host_kind);
        format!(
            "{}. This is expected, not an error. Retry with `{}`: {}. {}",
            rationale, self.argument_label, self.arguments, self.usage_note
        )
    }

    #[cfg(test)]
    pub fn policy_json(&self, host_kind: HostKind) -> Value {
        json!({
            "tool_name": self.render_tool_name(host_kind),
            "argument_label": self.argument_label,
            "rationale": self.render_rationale(host_kind),
            "usage_note": self.usage_note,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RoutingDecision {
    PassThrough,
    EnrichInput(Map<String, Value>),
    Deny { retry: RetryDirective },
}

#[cfg(test)]
pub fn retry_policy_json(host_kind: HostKind) -> Value {
    json!({
        "read": RetryDirective::native_read(Value::Null).policy_json(host_kind),
        "search": RetryDirective::native_search(Value::Null).policy_json(host_kind),
        "shell": RetryDirective::native_shell(Value::Null).policy_json(host_kind),
    })
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
