//! `so_unwatch` tool — deregister a project path from watching.

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
    props.insert("path".into(), json!({
        "type": "string",
        "description": "Absolute path to the project directory to stop watching."
    }));
    props.insert("agent".into(), json!({
        "type": "string",
        "description": "Agent name — must match the value passed to so_watch."
    }));
    props.insert("session_id".into(), json!({
        "type": "string",
        "description": "Session ID — must match the value passed to so_watch."
    }));
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
                let agent = args.and_then(|a| a.get("agent")).and_then(|v| v.as_str()).map(str::to_string);
                let session_id = args.and_then(|a| a.get("session_id")).and_then(|v| v.as_str()).map(str::to_string);

                let removed = wm.unwatch(&path, agent.as_deref(), session_id.as_deref());
                let msg = if removed {
                    format!(
                        "unwatched: {path} (agent: {}, session: {})",
                        agent.as_deref().unwrap_or("unknown"),
                        session_id.as_deref().unwrap_or("auto"),
                    )
                } else {
                    format!("not watched: {path}")
                };
                Ok(CallToolResult::success(vec![Content::text(msg)]))
            })
        },
    )
}
