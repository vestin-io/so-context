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
                m
            }),
        ),
        move |ctx: ToolCallContext<'_, BuiltinServer>| {
            let wm = Arc::clone(&wm);
            Box::pin(async move {
                let agent         = ctx.service.agent();
                let agent_version = ctx.service.agent_version();
                let session_id    = ctx.service.session_id();

                let timer = Timer::start();
                let result = handler(&wm);
                let duration_ms = timer.elapsed_ms();

                let mut ev = EventRecord::new(&agent, &session_id, "so_status");
                ev.agent_version  = agent_version;
                ev.agent_source   = "client_info".to_string();
                ev.session_source = "generated".to_string();
                ev.duration_ms             = Some(duration_ms);
                ev.estimated_origin_tokens = Some(0);

                match &result {
                    Ok(r) => {
                        let tokens: i64 = r.content.iter()
                            .filter_map(|c| c.as_text())
                            .map(|t| count_tokens(&t.text))
                            .sum();
                        ev.actual_tokens = Some(tokens);
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
                    .map(|c| format!("{}({})", c.agent, c.session_id))
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
