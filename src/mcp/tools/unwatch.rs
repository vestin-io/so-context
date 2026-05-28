//! `so_unwatch` tool — deregister a project path from watching.
//!
//! Decrements the ref-count for the project. The watch thread is only stopped
//! when the last consumer calls unwatch.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::model::{CallToolResult, Content, Tool};
use serde_json::{Map, Value, json};

use super::BuiltinServer;
use crate::daemon::WatchManager;

pub fn route(wm: Arc<WatchManager>) -> ToolRoute<BuiltinServer> {
    let mut schema = Map::new();
    schema.insert("type".into(), Value::String("object".into()));
    let mut props = Map::new();
    props.insert(
        "path".into(),
        json!({
            "type": "string",
            "description": "Absolute path to the project directory to stop watching."
        }),
    );
    props.insert(
        "agent_id".into(),
        json!({
            "type": "string",
            "description": "The agent_id that was passed to so_watch. Must match to correctly decrement the ref-count."
        }),
    );
    schema.insert("properties".into(), Value::Object(props));
    schema.insert("required".into(), json!(["path"]));

    ToolRoute::new_dyn(
        Tool::new(
            "so_unwatch",
            "Deregister this agent session from watching a project directory. \
             The watch thread is only stopped when all consumers have unwatched.",
            Arc::new(schema),
        ),
        move |ctx| {
            let wm = Arc::clone(&wm);
            Box::pin(async move {
                let args = ctx.arguments.as_ref();
                let path = args
                    .and_then(|a| a.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(".")
                    .to_string();
                let agent_id = args
                    .and_then(|a| a.get("agent_id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let removed = wm.unwatch(&path, agent_id.as_deref());

                let msg = if removed {
                    match agent_id {
                        Some(id) => format!("unwatched: {path} (agent: {id})"),
                        None    => format!("unwatched: {path}"),
                    }
                } else {
                    format!("not watched: {path}")
                };
                Ok(CallToolResult::success(vec![Content::text(msg)]))
            })
        },
    )
}
