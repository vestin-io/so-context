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
    props.insert(
        "path".into(),
        json!({
            "type": "string",
            "description": "Absolute path to the project directory to watch."
        }),
    );
    schema.insert("properties".into(), Value::Object(props));
    schema.insert("required".into(), json!(["path"]));

    ToolRoute::new_dyn(
        Tool::new(
            "so_watch",
            "Register a project directory for continuous file-change watching and graph indexing.",
            Arc::new(schema),
        ),
        move |ctx| {
            let wm = Arc::clone(&wm);
            Box::pin(async move {
                let path = ctx
                    .arguments
                    .as_ref()
                    .and_then(|a| a.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(".")
                    .to_string();
                wm.ensure_watching(&path);
                Ok(CallToolResult::success(vec![Content::text(format!(
                    "watching: {path}"
                ))]))
            })
        },
    )
}
