use serde_json::{Map, Value};

use crate::host_adapter::{HostKind, ToolNamespace};

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

    pub fn shell_session_id(&self) -> Option<&str> {
        self.tool_context_id()
    }

    pub fn shell_agent_id(&self) -> Option<&str> {
        self.agent_id.as_deref().filter(|value| !value.is_empty())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizedToolCall {
    pub host_kind: HostKind,
    pub tool_namespace: ToolNamespace,
    pub tool_name: String,
    pub tool_input: Map<String, Value>,
    pub identity: IdentityContext,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolInputPatch {
    pub updated_input: Map<String, Value>,
    pub input_key_order: Vec<String>,
}

impl ToolInputPatch {
    pub fn new(updated_input: Map<String, Value>) -> Self {
        Self {
            updated_input,
            input_key_order: Vec::new(),
        }
    }

    pub fn with_input_key_order(mut self, input_key_order: Vec<String>) -> Self {
        self.input_key_order = input_key_order;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RoutingDecision {
    PassThrough,
    PatchInput(ToolInputPatch),
}
