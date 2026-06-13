use serde_json::Value;

use crate::host_adapter::ToolNamespace;
use crate::routing_session::{
    NATIVE_READ_TOOL_NAMES, NATIVE_SEARCH_TOOL_NAMES, NormalizedToolCall, RetryDirective,
    RoutingDecision,
};
use crate::shell::{
    NATIVE_SHELL_TOOL_NAMES, parse_simple_shell_command, rewrite_env_prefix, should_prefer_so_shell,
};

pub struct RoutingService;

impl RoutingService {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate_tool_call(&self, request: &NormalizedToolCall) -> RoutingDecision {
        if request.tool_namespace == ToolNamespace::SoContextMcp {
            return self.inject_session_id(request);
        }

        if NATIVE_SHELL_TOOL_NAMES.contains(&request.tool_name.as_str()) {
            return self.deny_native_shell_if_needed(request);
        }

        if NATIVE_READ_TOOL_NAMES.contains(&request.tool_name.as_str()) {
            return self.deny_native_read_if_needed(request);
        }

        if NATIVE_SEARCH_TOOL_NAMES.contains(&request.tool_name.as_str()) {
            return self.deny_native_search_if_needed(request);
        }

        RoutingDecision::PassThrough
    }

    fn inject_session_id(&self, request: &NormalizedToolCall) -> RoutingDecision {
        let Some(context_id) = request.identity.tool_context_id() else {
            return RoutingDecision::PassThrough;
        };

        let mut tool_input = request.tool_input.clone();

        tool_input.insert(
            "_so_session_id".to_string(),
            Value::String(context_id.to_string()),
        );

        RoutingDecision::EnrichInput(tool_input)
    }

    fn deny_native_shell_if_needed(&self, request: &NormalizedToolCall) -> RoutingDecision {
        let Some(command) = request
            .routing_input
            .get("command")
            .and_then(|value| value.as_str())
        else {
            return RoutingDecision::PassThrough;
        };

        let command = command.trim();
        if command.is_empty() {
            return RoutingDecision::PassThrough;
        }

        let Some(argv) = parse_simple_shell_command(command) else {
            return RoutingDecision::PassThrough;
        };
        let inspected_argv = rewrite_env_prefix(argv);
        if !should_prefer_so_shell(&inspected_argv) {
            return RoutingDecision::PassThrough;
        }

        RoutingDecision::Deny {
            retry: RetryDirective::native_shell(Value::Array(
                inspected_argv
                    .into_iter()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            )),
        }
    }

    fn deny_native_read_if_needed(&self, request: &NormalizedToolCall) -> RoutingDecision {
        let Some(retry) = Self::preferred_so_read_args(&request.routing_input) else {
            return RoutingDecision::PassThrough;
        };
        RoutingDecision::Deny {
            retry: RetryDirective::native_read(retry),
        }
    }

    fn preferred_so_read_args(tool_input: &serde_json::Map<String, Value>) -> Option<Value> {
        let path = tool_input
            .get("path")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())?
            .trim();

        let mut retry = serde_json::Map::new();
        retry.insert("path".to_string(), Value::String(path.to_string()));

        if let Some(mode) = tool_input
            .get("mode")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            retry.insert("mode".to_string(), Value::String(mode.to_string()));
        }

        let mut has_excerpt = false;
        if let Some(start_line) = tool_input
            .get("start_line")
            .and_then(|value| value.as_u64())
            .filter(|value| *value > 0)
        {
            retry.insert("start_line".to_string(), Value::Number(start_line.into()));
            has_excerpt = true;
        }
        if let Some(end_line) = tool_input
            .get("end_line")
            .and_then(|value| value.as_u64())
            .filter(|value| *value > 0)
        {
            retry.insert("end_line".to_string(), Value::Number(end_line.into()));
            has_excerpt = true;
        }
        if let Some(line_numbers) = tool_input
            .get("line_numbers")
            .and_then(|value| value.as_bool())
        {
            retry.insert("line_numbers".to_string(), Value::Bool(line_numbers));
            has_excerpt = true;
        }

        if !retry.contains_key("mode") && !has_excerpt {
            retry.insert("mode".to_string(), Value::String("full".to_string()));
        }

        Some(Value::Object(retry))
    }

    fn deny_native_search_if_needed(&self, request: &NormalizedToolCall) -> RoutingDecision {
        let Some(retry) = Self::preferred_so_search_args(&request.routing_input) else {
            return RoutingDecision::PassThrough;
        };

        RoutingDecision::Deny {
            retry: RetryDirective::native_search(Value::Object(retry)),
        }
    }

    fn preferred_so_search_args(
        tool_input: &serde_json::Map<String, Value>,
    ) -> Option<serde_json::Map<String, Value>> {
        if tool_input
            .get("_has_extra_search_semantics")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            return None;
        }

        let query = tool_input
            .get("query")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| {
                if !Self::is_simple_literal_search_query(value) {
                    return None;
                }
                Some(value.to_string())
            })?;

        if tool_input.contains_key("regex") {
            return None;
        }

        let path = tool_input
            .get("path")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let mut retry = serde_json::Map::new();
        retry.insert("query".to_string(), Value::String(query));
        if let Some(path) = path {
            retry.insert("path".to_string(), Value::String(path.to_string()));
        }
        if let Some(limit) = tool_input
            .get("limit")
            .and_then(|value| value.as_u64())
            .filter(|value| *value > 0)
        {
            retry.insert("limit".to_string(), Value::Number(limit.into()));
        }

        Some(retry)
    }

    fn is_simple_literal_search_query(query: &str) -> bool {
        !query.is_empty()
            && !query.chars().any(|ch| {
                matches!(
                    ch,
                    '\\' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
                )
            })
    }
}
