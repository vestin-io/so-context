//! `so_search` tool — FTS search over an indexed project graph.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use super::resolve_project_path_arg;
use crate::core_events::{EventRecord, Timer, enqueue};
use crate::core_graph;
use crate::core_tokens::count_tokens;
use crate::daemon::WatchManager;

pub fn route(wm: Arc<WatchManager>) -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_search",
            "Search the indexed project code graph (FTS). Index the project first if results are missing or stale.",
            schema(),
        ),
        move |ctx| {
            let wm = Arc::clone(&wm);
            Box::pin(async move { handler(ctx, &wm) })
        },
    )
}

fn handler(
    ctx: ToolCallContext<'_, BuiltinServer>,
    wm: &WatchManager,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let client = ctx.service.client();
    let client_version = ctx.service.client_version();
    let connection_id = ctx.service.connection_id();

    let args = ctx
        .arguments
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

    // Agent session ID injected by the PreToolUse hook on the agent side.
    // Falls back to the connection_id when not provided (e.g. agents without hook support).
    let (session_id, session_source) = match args
        .get("_so_session_id")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
    {
        Some(sid) => (sid.to_string(), "hook"),
        None => (connection_id.clone(), "connection"),
    };

    let query = args
        .get("query")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing string argument: query", None))?;

    let path = resolve_project_path_arg(&args, "path", &client, &connection_id, &wm.status())?;
    let path_display = path.display().to_string();

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(20) as usize;

    let timer = Timer::start();
    let call_result = core_graph::search_project_with_stats(&path_display, query, limit);
    let duration_ms = timer.elapsed_ms();

    let mut ev = EventRecord::new(&session_id, "so_search");
    ev.client = client;
    ev.client_version = client_version;
    ev.client_source = "client_info".to_string();
    ev.session_source = session_source.to_string();
    ev.project = Some(path_display.clone());
    ev.params = Some(
        serde_json::json!({ "query": query, "path": path_display, "limit": limit }).to_string(),
    );
    ev.duration_ms = Some(duration_ms);

    match call_result {
        Ok((output, matched_files_tokens, matched_files_size)) => {
            ev.actual_tokens = Some(count_tokens(&output));
            ev.estimated_origin_tokens = Some(matched_files_tokens);
            ev.actual_size = Some(output.len() as i64);
            ev.estimated_origin_size = Some(matched_files_size);
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
                "query": { "type": "string" },
                "path": {
                    "type": "string",
                    "description": "Project root path. Defaults to the sole watched project for the current MCP connection."
                },
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 20 },
                "_so_session_id": {
                    "type": "string",
                    "description": "Agent session ID injected by the so-context PreToolUse hook. Do not set manually."
                }
            },
            "required": ["query"]
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    )
}
