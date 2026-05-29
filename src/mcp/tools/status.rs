//! `so_status` tool — list all watched projects and their state.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, Tool};

use super::BuiltinServer;
use crate::core_events::{EventRecord, Timer, chars_to_tokens};
use crate::daemon::WatchManager;
use crate::daemon::watch_manager::WatchState;

pub fn route(wm: Arc<WatchManager>) -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_status",
            "List all auto-discovered projects currently being watched, their sync state, \
             ref-count, and the list of agent consumers.",
            Arc::new(serde_json::Map::new()),
        ),
        move |ctx: ToolCallContext<'_, BuiltinServer>| {
            let wm = Arc::clone(&wm);
            Box::pin(async move {
                let agent = ctx.service.agent();
                let session_id = ctx.service.session_id();

                let timer = Timer::start();
                let result = handler(&wm);
                let duration_ms = timer.elapsed_ms();

                let mut ev = EventRecord::new(&agent, &session_id, "so_status");
                ev.duration_ms = Some(duration_ms);
                ev.estimated_origin_tokens = Some(0);

                match &result {
                    Ok(r) => {
                        let text_len: usize = r
                            .content
                            .iter()
                            .filter_map(|c| c.as_text())
                            .map(|t| t.text.len())
                            .sum();
                        ev.actual_tokens = Some(chars_to_tokens(text_len));
                        ev.result_ok = true;
                    }
                    Err(_) => {
                        ev.result_ok = false;
                    }
                }
                ev.insert();
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
                WatchState::Indexing => "indexing".to_string(),
                WatchState::Running => "running".to_string(),
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
    Ok(CallToolResult::success(vec![Content::text(
        lines.join("\n"),
    )]))
}
