use anyhow::Result;
use serde_json::{Value, json};

use crate::host_adapter::{HostKind, adapter_for_host};
use crate::routing_session::{RoutingDecision, RoutingService, SessionService};

const PRE_TOOL_USE_EVENT: &str = "PreToolUse";

pub fn run_pre_tool_use_hook_for_host(host_kind: HostKind) -> Result<()> {
    let input: Value = serde_json::from_reader(std::io::stdin()).unwrap_or(Value::Null);

    if let Some(output) = evaluate_pre_tool_hook_for_host(host_kind, &input) {
        println!("{output}");
    }

    Ok(())
}

#[cfg(test)]
fn run_pre_tool_use_hook_value(input: &Value) -> Option<Value> {
    evaluate_pre_tool_hook_for_host(HostKind::Unknown, input)
}

#[cfg(test)]
pub(crate) fn run_pre_tool_use_hook_value_for_test(
    host_kind: HostKind,
    input: &Value,
) -> Option<Value> {
    evaluate_pre_tool_hook_for_host(host_kind, input)
}

pub(crate) fn evaluate_pre_tool_hook_for_host(host_kind: HostKind, input: &Value) -> Option<Value> {
    let adapter = adapter_for_host(host_kind);
    let session_service = SessionService::new();
    let request = adapter.normalize_pre_tool_call(&session_service, input);

    match RoutingService::new().evaluate_tool_call(&request) {
        RoutingDecision::PassThrough => None,
        RoutingDecision::EnrichInput(updated_input) => Some(json!({
            "hookSpecificOutput": {
                "hookEventName": PRE_TOOL_USE_EVENT,
                "permissionDecision": "allow",
                "updatedInput": updated_input,
            }
        })),
        RoutingDecision::Deny { retry } => Some(json!({
            "hookSpecificOutput": {
                "hookEventName": PRE_TOOL_USE_EVENT,
                "permissionDecision": "deny",
                "permissionDecisionReason": retry.render_reason(request.host_kind),
            }
        })),
    }
}

#[cfg(test)]
#[path = "pre_tool_tests.rs"]
mod tests;
