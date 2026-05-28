//! `so_events` tool — query the event log and token savings stats.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use crate::core_events::{EventQuery, query_events, query_stats};

pub fn route() -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_events",
            "Query the so-context event log. Use mode=stats for aggregate token savings, \
             or mode=list for recent events. Filter by agent, session_id, tool, or project.",
            schema(),
        ),
        |ctx| Box::pin(async move { handler(ctx) }),
    )
}

fn handler(ctx: rmcp::handler::server::tool::ToolCallContext<'_, BuiltinServer>) -> Result<CallToolResult, rmcp::ErrorData> {
    let args = ctx.arguments.as_ref();

    let mode = args
        .and_then(|a| a.get("mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("stats");

    let agent = args.and_then(|a| a.get("agent")).and_then(|v| v.as_str());
    let session_id = args.and_then(|a| a.get("session_id")).and_then(|v| v.as_str());
    let tool = args.and_then(|a| a.get("tool")).and_then(|v| v.as_str());
    let project = args.and_then(|a| a.get("project")).and_then(|v| v.as_str());
    let limit = args.and_then(|a| a.get("limit")).and_then(|v| v.as_u64()).unwrap_or(50) as usize;

    let output = match mode {
        "stats" => query_stats(agent, session_id)
            .map_err(|e| rmcp::ErrorData::internal_error(e, None))?,

        "list" => {
            let q = EventQuery {
                agent:      agent.map(str::to_string),
                session_id: session_id.map(str::to_string),
                tool:       tool.map(str::to_string),
                project:    project.map(str::to_string),
                limit,
            };
            let rows = query_events(&q)
                .map_err(|e| rmcp::ErrorData::internal_error(e, None))?;

            if rows.is_empty() {
                "no events found".to_string()
            } else {
                rows.iter().map(|r| {
                    let saved = r.tokens_saved().map(|s| format!("saved={s}")).unwrap_or_default();
                    let params = r.params.as_deref().unwrap_or("{}");
                    format!(
                        "[{}] {} | {} | {} | actual={} {} | {}",
                        r.ts,
                        r.agent,
                        r.session_id,
                        r.tool,
                        r.actual_tokens.unwrap_or(0),
                        saved,
                        params,
                    )
                }).collect::<Vec<_>>().join("\n")
            }
        }

        other => return Err(rmcp::ErrorData::invalid_params(
            format!("unknown mode: {other}; expected stats or list"),
            None,
        )),
    };

    Ok(CallToolResult::success(vec![Content::text(output)]))
}

fn schema() -> Arc<JsonObject> {
    Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "mode":       { "type": "string", "enum": ["stats", "list"], "default": "stats" },
                "agent":      { "type": "string", "description": "Filter by agent name." },
                "session_id": { "type": "string", "description": "Filter by session ID." },
                "tool":       { "type": "string", "description": "Filter by tool name (list mode)." },
                "project":    { "type": "string", "description": "Filter by project path (list mode)." },
                "limit":      { "type": "integer", "default": 50, "description": "Max rows (list mode)." }
            }
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    )
}
