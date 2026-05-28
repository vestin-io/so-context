//! `so_status` tool — list all watched projects and their state.

use std::sync::Arc;

use rmcp::model::{CallToolResult, Content, Tool};
use rmcp::handler::server::router::tool::ToolRoute;

use super::BuiltinServer;
use crate::daemon::watch_manager::WatchState;
use crate::daemon::WatchManager;

pub fn route(wm: Arc<WatchManager>) -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_status",
            "List all auto-discovered projects currently being watched, their sync state, \
             ref-count, and the list of agent consumers.",
            Arc::new(serde_json::Map::new()),
        ),
        move |_ctx| {
            let wm = Arc::clone(&wm);
            Box::pin(async move { handler(&wm) })
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
            consumers.sort();
            let consumer_str = if consumers.is_empty() {
                "none".to_string()
            } else {
                consumers.join(", ")
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
