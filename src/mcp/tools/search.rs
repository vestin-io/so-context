//! `so_search` tool — FTS search over an indexed project graph.

use std::sync::Arc;

use rmcp::model::{CallToolResult, Content, JsonObject, Tool};
use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;

use super::BuiltinServer;
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

    let output = core_graph::search_project(path, query, limit)
        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?;

    Ok(CallToolResult::success(vec![Content::text(output)]))
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
