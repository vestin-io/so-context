//! `so_watch` tool — register a project path for watching.

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
        "description": "Absolute path to the project directory to watch."
    }));
    props.insert("agent".into(), json!({
        "type": "string",
        "description": "Name of the calling agent (e.g. \"claude\", \"opencode\", \"codex\"). Defaults to \"unknown\"."
    }));
    props.insert("session_id".into(), json!({
        "type": "string",
        "description": "Unique identifier for this agent session. Auto-generated UUID if omitted."
    }));
    schema.insert("properties".into(), Value::Object(props));
    schema.insert("required".into(), json!(["path"]));

    ToolRoute::new_dyn(
        Tool::new(
            "so_watch",
            "Register a project directory for continuous file-change watching and graph \
             indexing. Multiple agent sessions can watch the same project; the watch thread \
             is shared and only stopped when all consumers have unwatched.",
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

                wm.ensure_watching(&path, agent.as_deref(), session_id.as_deref());

                Ok(CallToolResult::success(vec![Content::text(format!(
                    "watching: {path} (agent: {}, session: {})",
                    agent.as_deref().unwrap_or("unknown"),
                    session_id.as_deref().unwrap_or("auto"),
                ))]))
            })
        },
    )
}
