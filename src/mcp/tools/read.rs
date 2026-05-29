//! `so_read` tool — read a file or trigger a graph index.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use crate::core_read;

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

    let output =
        core_read::read(path, mode).map_err(|e| rmcp::ErrorData::internal_error(e, None))?;

    Ok(CallToolResult::success(vec![Content::text(output)]))
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
