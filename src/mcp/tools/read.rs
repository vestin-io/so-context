//! `so_read` tool — read a file or trigger a graph index.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use crate::core_events::{EventRecord, Timer, enqueue};
use crate::core_read;
use crate::core_tokens::count_tokens;

pub fn route() -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_read",
            "Read file/project by path. mode=full (default), outline, or graph.",
            schema(),
        ),
        |ctx| Box::pin(async move { handler(ctx) }),
    )
}

fn handler(ctx: ToolCallContext<'_, BuiltinServer>) -> Result<CallToolResult, rmcp::ErrorData> {
    let meta_args = ctx.arguments.as_ref();
    let agent = meta_args
        .and_then(|a| a.get("_so_agent"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| ctx.service.agent());
    let session_id = meta_args
        .and_then(|a| a.get("_so_session_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| ctx.service.session_id());

    let args = ctx
        .arguments
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

    let path = args
        .get("path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing string argument: path", None))?;

    let mode = args
        .get("mode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("full");

    // For outline/graph modes, read the raw file size before processing so we
    // can compute estimated_origin_tokens (what the agent would have read in full).
    let full_file_tokens: Option<i64> = if mode == "outline" || mode == "graph" {
        std::fs::read_to_string(path).ok().map(|c| count_tokens(&c))
    } else {
        None
    };

    let timer = Timer::start();
    let call_result = core_read::read(path, mode);
    let duration_ms = timer.elapsed_ms();

    let mut ev = EventRecord::new(&agent, &session_id, "so_read");
    ev.project     = Some(path.to_string());
    ev.params      = Some(serde_json::json!({ "path": path, "mode": mode }).to_string());
    ev.duration_ms = Some(duration_ms);

    match call_result {
        Ok(output) => {
            let actual = count_tokens(&output);
            ev.actual_tokens = Some(actual);
            ev.estimated_origin_tokens = Some(match mode {
                // outline/graph: agent would have read the full file
                "outline" | "graph" => {
                    full_file_tokens.unwrap_or(actual)
                }
                // full: no saving — same as actual
                _ => actual,
            });
            ev.result_ok = true;
            enqueue(ev);
            Ok(CallToolResult::success(vec![Content::text(output)]))
        }
        Err(e) => {
            ev.result_ok = false;
            enqueue(ev);
            Err(rmcp::ErrorData::internal_error(e, None))
        }
    }
}

fn schema() -> Arc<JsonObject> {
    Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "mode": { "type": "string", "enum": ["full", "outline", "graph"] }
            },
            "required": ["path"]
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    )
}
