//! `so_search` tool — FTS search over an indexed project graph.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use crate::core_events::{EventRecord, Timer, chars_to_tokens};
use crate::core_graph;

pub fn route() -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_search",
            "Search indexed project code graph (FTS). Run graph_watch or so_read(mode=graph) first.",
            schema(),
        ),
        |ctx| Box::pin(async move { handler(ctx) }),
    )
}

fn handler(ctx: ToolCallContext<'_, BuiltinServer>) -> Result<CallToolResult, rmcp::ErrorData> {
    let agent = ctx.service.agent();
    let session_id = ctx.service.session_id();

    let args = ctx
        .arguments
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

    let query = args
        .get("query")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing string argument: query", None))?;

    let path = args
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(".");

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(20) as usize;

    let timer = Timer::start();
    let call_result = core_graph::search_project_with_stats(path, query, limit);
    let duration_ms = timer.elapsed_ms();

    let mut ev = EventRecord::new(&agent, &session_id, "so_search");
    ev.project = Some(path.to_string());
    ev.params =
        Some(serde_json::json!({ "query": query, "path": path, "limit": limit }).to_string());
    ev.duration_ms = Some(duration_ms);

    match call_result {
        Ok((output, matched_files_chars)) => {
            ev.actual_tokens = Some(chars_to_tokens(output.len()));
            ev.estimated_origin_tokens = Some(chars_to_tokens(matched_files_chars));
            ev.result_ok = true;
            ev.insert();
            Ok(CallToolResult::success(vec![Content::text(output)]))
        }
        Err(e) => {
            ev.result_ok = false;
            ev.insert();
            Err(rmcp::ErrorData::internal_error(e, None))
        }
    }
}

fn schema() -> Arc<JsonObject> {
    Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "path": { "type": "string", "default": "." },
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 20 }
            },
            "required": ["query"]
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    )
}
