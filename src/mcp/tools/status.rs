//! `so_status` tool — list all watched projects and their state.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, Tool};

use super::BuiltinServer;
use crate::core_events::{EventRecord, Timer, enqueue};
use crate::core_tokens::count_tokens;
use crate::daemon::watch_manager::WatchState;
use crate::daemon::WatchManager;

pub fn route(wm: Arc<WatchManager>) -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_status",
            "List all auto-discovered projects currently being watched, their sync state, \
             ref-count, and the list of agent consumers.",
            Arc::new({
                let mut m = serde_json::Map::new();
                m.insert("type".to_string(), serde_json::Value::String("object".to_string()));
                m.insert("properties".to_string(), serde_json::json!({
                    "_so_session_id": {
                        "type": "string",
                        "description": "Agent session ID injected by the so-context PreToolUse hook. Do not set manually."
                    }
                }));
                m
            }),
        ),
        move |ctx: ToolCallContext<'_, BuiltinServer>| {
            let wm = Arc::clone(&wm);
            Box::pin(async move {
                let client         = ctx.service.client();
                let client_version = ctx.service.client_version();
                let connection_id  = ctx.service.connection_id();

                // Agent session ID injected by the PreToolUse hook on the agent side.
                // Falls back to the connection_id when not provided.
                let (session_id, session_source) = match ctx.arguments
                    .as_ref()
                    .and_then(|a| a.get("_so_session_id"))
                    .and_then(serde_json::Value::as_str)
                    .filter(|s| !s.is_empty())
                {
                    Some(sid) => (sid.to_string(), "hook"),
                    None      => (connection_id,   "connection"),
                };

                let timer = Timer::start();
                let result = handler(&wm);
                let duration_ms = timer.elapsed_ms();

                let mut ev = EventRecord::new(&session_id, "so_status");
                ev.client         = client;
                ev.client_version = client_version;
                ev.client_source  = "client_info".to_string();
                ev.session_source = session_source.to_string();
                ev.duration_ms             = Some(duration_ms);
                ev.estimated_origin_tokens = Some(0);

                match &result {
                    Ok(r) => {
                        let text: String = r.content.iter()
                            .filter_map(|c| c.as_text())
                            .map(|t| t.text.as_str())
                            .collect::<Vec<_>>()
                            .join("");
                        let tokens = count_tokens(&text);
                        ev.actual_tokens           = Some(tokens);
                        ev.actual_size             = Some(text.len() as i64);
                        ev.estimated_origin_size   = Some(0);
                        ev.result_ok = true;
                    }
                    Err(_) => { ev.result_ok = false; }
                }
                enqueue(ev);
                result
            })
        },
    )
}

fn handler(wm: &WatchManager) -> Result<CallToolResult, rmcp::ErrorData> {
    let statuses = wm.status();
    if statuses.is_empty() {
        return Ok(CallToolResult::success(vec![Content::text(
            "no projects being watched".to_string(),
        )]));
    }
    let lines: Vec<String> = statuses
        .iter()
        .map(|s| {
            let state = match &s.state {
                WatchState::Indexing  => "indexing".to_string(),
                WatchState::Running   => "running".to_string(),
                WatchState::Failed(e) => format!("failed: {e}"),
            };
            let mut consumers = s.consumers.clone();
            consumers.sort_by(|a, b| a.key().cmp(&b.key()));
            let consumer_str = if consumers.is_empty() {
                "none".to_string()
            } else {
                consumers
                    .iter()
                    .map(|c| format!("{}({})", c.client, c.session_id))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            format!(
                "{} — {state}  [refs: {}, consumers: {consumer_str}]",
                s.path.display(),
                s.ref_count,
            )
        })
        .collect();
    Ok(CallToolResult::success(vec![Content::text(lines.join("\n"))]))
}
