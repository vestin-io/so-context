//! `so_shell_output` tool — fetch raw output from a prior `so_shell` run.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use crate::core_events::{EventRecord, Timer, enqueue};
use crate::core_tokens::count_tokens;
use crate::shell::get_spooled_output;

pub fn route() -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_shell_output",
            "Fetch raw output cached from an earlier so_shell call by run_id. The returned text content is the actual cached command output to use directly. Use this only when the user explicitly asks for verbatim raw output or the compressed shell result is missing required detail. Do not use this tool just to double-check or confirm a compressed result that already answers the request.",
            schema(),
        ),
        |ctx| Box::pin(async move { handler(ctx) }),
    )
}

fn handler(ctx: ToolCallContext<'_, BuiltinServer>) -> Result<CallToolResult, rmcp::ErrorData> {
    let client = ctx.service.client();
    let client_version = ctx.service.client_version();
    let connection_id = ctx.service.connection_id();
    let args = ctx
        .arguments
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

    let (session_id, session_source) = match args
        .get("_so_session_id")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
    {
        Some(sid) => (sid.to_string(), "hook"),
        None => (connection_id, "connection"),
    };

    let run_id = args
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing string argument: run_id", None))?
        .to_string();
    let reason = args
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing string argument: reason", None))?;
    if !matches!(
        reason,
        "user_requested_verbatim_output" | "compressed_summary_missing_required_detail"
    ) {
        return Err(rmcp::ErrorData::invalid_params(
            "reason must be one of: user_requested_verbatim_output, compressed_summary_missing_required_detail",
            None,
        ));
    }

    let timer = Timer::start();
    let result = match get_spooled_output(&run_id, &session_id, client.as_deref()) {
        Some(output) => {
            let text = render_cached_output(&output.full_output, output.exit_code);
            let mut tool_result = if output.exit_code == 0 {
                CallToolResult::success(vec![Content::text(text.clone())])
            } else {
                CallToolResult::error(vec![Content::text(text.clone())])
            };
            tool_result.structured_content = Some(serde_json::json!({
                "run_id": output.run_id,
                "argv": output.argv,
                "cwd": output.cwd.map(|path| path.to_string_lossy().to_string()),
                "exit_code": output.exit_code,
                "content_kind": "raw_output",
                "source": "spool",
                "reason": reason,
                "use_policy": "only_for_verbatim_user_request_or_missing_required_detail",
                "output_is_in_text": true,
                "rerun_not_needed_if_text_sufficient": true,
            }));
            Ok((tool_result, Some(text), true))
        }
        None => {
            let message = format!(
                "no cached raw shell output found for run_id `{run_id}`; rerun so_shell with `full: true` and `full_reason: \"tee_missing_or_expired\"` if you still need the original output"
            );
            let mut tool_result = CallToolResult::error(vec![Content::text(message.clone())]);
            tool_result.structured_content = Some(serde_json::json!({
                "run_id": run_id,
                "content_kind": "raw_output",
                "source": "spool",
                "reason": reason,
                "found": false,
            }));
            Ok((tool_result, Some(message), false))
        }
    }?;
    let duration_ms = timer.elapsed_ms();

    let mut event = EventRecord::new(&session_id, "so_shell_output");
    event.client = client;
    event.client_version = client_version;
    event.client_source = "client_info".to_string();
    event.session_source = session_source.to_string();
    event.params = Some(serde_json::json!({ "run_id": run_id, "reason": reason }).to_string());
    event.duration_ms = Some(duration_ms);
    event.result_ok = result.2;

    if let Some(text) = result.1.as_ref() {
        let tokens = count_tokens(text);
        event.estimated_origin_tokens = Some(tokens);
        event.actual_tokens = Some(tokens);
        event.estimated_origin_size = Some(text.len() as i64);
        event.actual_size = Some(text.len() as i64);
    }

    enqueue(event);
    Ok(result.0)
}

fn render_cached_output(full_output: &str, exit_code: i32) -> String {
    if exit_code == 0 {
        return full_output.to_string();
    }

    if full_output.is_empty() {
        return format!("[shell] exit_code={exit_code}\n");
    }

    if full_output.ends_with('\n') {
        format!("{full_output}[shell] exit_code={exit_code}\n")
    } else {
        format!("{full_output}\n[shell] exit_code={exit_code}\n")
    }
}

fn schema() -> Arc<JsonObject> {
    Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "run_id": {
                    "type": "string",
                    "description": "Run ID returned by a prior so_shell call."
                },
                "reason": {
                    "type": "string",
                    "enum": [
                        "user_requested_verbatim_output",
                        "compressed_summary_missing_required_detail"
                    ],
                    "description": "Why raw output is needed. Use this only when the user explicitly asked for verbatim raw output, or the compressed summary omitted required detail."
                },
                "_so_session_id": {
                    "type": "string",
                    "description": "Agent session ID injected by the so-context PreToolUse hook. Do not set manually."
                }
            },
            "required": ["run_id", "reason"]
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    )
}
