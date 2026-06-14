use serde_json::{Map, Value};

use crate::host_adapter::ToolNamespace;
use crate::routing_session::{NormalizedToolCall, RoutingDecision, ToolInputPatch};
use crate::shell::{
    NATIVE_SHELL_TOOL_NAMES, ShellRedirectContext, rewrite_native_shell_tool_input,
};

pub struct RoutingService;

impl RoutingService {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate_tool_call(&self, request: &NormalizedToolCall) -> RoutingDecision {
        if request.tool_namespace == ToolNamespace::SoContextMcp {
            return self.patch_self_tool_input(request);
        }

        if NATIVE_SHELL_TOOL_NAMES.contains(&request.tool_name.as_str()) {
            return self.rewrite_native_shell_if_needed(request);
        }

        RoutingDecision::PassThrough
    }

    fn patch_self_tool_input(&self, request: &NormalizedToolCall) -> RoutingDecision {
        let mut tool_input = request.tool_input.clone();
        let mut changed = false;

        if let Some(context_id) = request.identity.tool_context_id() {
            tool_input.insert(
                "_so_session_id".to_string(),
                Value::String(context_id.to_string()),
            );
            changed = true;
        }

        changed |= normalize_self_tool_input(&request.tool_name, &mut tool_input);
        let input_key_order = self_tool_input_key_order(&request.tool_name, &tool_input);

        if !changed && input_key_order.is_empty() {
            return RoutingDecision::PassThrough;
        }

        RoutingDecision::PatchInput(
            ToolInputPatch::new(tool_input).with_input_key_order(input_key_order),
        )
    }

    fn rewrite_native_shell_if_needed(&self, request: &NormalizedToolCall) -> RoutingDecision {
        let context = ShellRedirectContext {
            client: Some(request.host_kind.as_str().to_string()).filter(|value| value != "unknown"),
            session_id: request.identity.shell_session_id().map(ToString::to_string),
            agent_id: request.identity.shell_agent_id().map(ToString::to_string),
        };

        rewrite_native_shell_tool_input(&request.tool_input, &context)
            .map(ToolInputPatch::new)
            .map(RoutingDecision::PatchInput)
            .unwrap_or(RoutingDecision::PassThrough)
    }
}

fn normalize_self_tool_input(tool_name: &str, tool_input: &mut Map<String, Value>) -> bool {
    if !is_so_shell_tool(tool_name) {
        return false;
    }

    let mut changed = false;
    if tool_input.get("full").and_then(Value::as_bool) != Some(true)
        && tool_input.remove("full_reason").is_some()
    {
        changed = true;
    }

    let Some(command) = derive_shell_display_command(tool_input) else {
        return changed;
    };

    if tool_input.get("command").and_then(Value::as_str) != Some(command.as_str()) {
        tool_input.insert("command".to_string(), Value::String(command));
        changed = true;
    }

    changed
}

fn self_tool_input_key_order(tool_name: &str, tool_input: &Map<String, Value>) -> Vec<String> {
    if is_so_shell_tool(tool_name) {
        return ordered_keys(
            tool_input,
            &["command", "argv", "cwd", "full", "full_reason"],
            &["_so_session_id"],
        );
    }

    if tool_input.contains_key("_so_session_id") {
        return ordered_keys(tool_input, &[], &["_so_session_id"]);
    }

    Vec::new()
}

fn ordered_keys(
    tool_input: &Map<String, Value>,
    leading: &[&str],
    trailing: &[&str],
) -> Vec<String> {
    let mut ordered = Vec::new();

    for key in leading {
        if tool_input.contains_key(*key) {
            ordered.push((*key).to_string());
        }
    }

    for key in tool_input.keys() {
        if leading.contains(&key.as_str()) || trailing.contains(&key.as_str()) {
            continue;
        }
        ordered.push(key.clone());
    }

    for key in trailing {
        if tool_input.contains_key(*key) {
            ordered.push((*key).to_string());
        }
    }

    ordered
}

fn derive_shell_display_command(tool_input: &Map<String, Value>) -> Option<String> {
    if let Some(argv) = tool_input.get("argv").and_then(Value::as_array) {
        let argv = argv
            .iter()
            .map(|value| value.as_str().map(ToString::to_string))
            .collect::<Option<Vec<_>>>()?;
        if !argv.is_empty() {
            return Some(crate::shell::types::ShellInvocation::render_argv(&argv));
        }
    }

    tool_input
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(ToString::to_string)
}

fn is_so_shell_tool(tool_name: &str) -> bool {
    tool_name.ends_with("so_shell")
}
