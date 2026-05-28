//! `so_watch` tool — register a project path for watching.
//!
//! Optional `agent_id` parameter identifies the calling agent session so the
//! daemon can track which agents are consuming each project.

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
            "description": "Absolute path to the project directory to watch."
        }),
    );
    props.insert(
        "agent_id".into(),
        json!({
            "type": "string",
            "description": "Optional unique identifier for the calling agent session (e.g. process ID or session token). Used to track consumers per project."
        }),
    );
    schema.insert("properties".into(), Value::Object(props));
    schema.insert("required".into(), json!(["path"]));

    ToolRoute::new_dyn(
        Tool::new(
            "so_watch",
            "Register a project directory for continuous file-change watching and graph indexing. \
             Multiple agent sessions can watch the same project; the watch thread is shared and \
             only stopped when all consumers unwatch.",
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

                wm.ensure_watching(&path, agent_id.as_deref());

                let msg = match agent_id {
                    Some(id) => format!("watching: {path} (agent: {id})"),
                    None    => format!("watching: {path}"),
                };
                Ok(CallToolResult::success(vec![Content::text(msg)]))
            })
        },
    )
}
