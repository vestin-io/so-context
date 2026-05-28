//! `so_watch` tool — register a project path for watching.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, Tool};
use serde_json::{Map, Value, json};

use super::BuiltinServer;
use crate::core_events::{EventRecord, Timer, chars_to_tokens};
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
        move |ctx: ToolCallContext<'_, BuiltinServer>| {
            let wm = Arc::clone(&wm);
            Box::pin(async move {
                let agent      = ctx.service.agent();
                let session_id = ctx.service.session_id();
                let args = ctx.arguments.as_ref();
                let path = args
                    .and_then(|a| a.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(".")
                    .to_string();
                let agent_arg = args.and_then(|a| a.get("agent")).and_then(|v| v.as_str()).map(str::to_string);
                let session_id_arg = args.and_then(|a| a.get("session_id")).and_then(|v| v.as_str()).map(str::to_string);

                let timer = Timer::start();
                wm.ensure_watching(&path, agent_arg.as_deref(), session_id_arg.as_deref());
                let duration_ms = timer.elapsed_ms();

                let msg = format!(
                    "watching: {path} (agent: {}, session: {})",
                    agent_arg.as_deref().unwrap_or("unknown"),
                    session_id_arg.as_deref().unwrap_or("auto"),
                );

                let mut ev = EventRecord::new(&agent, &session_id, "so_watch");
                ev.project     = Some(path.clone());
                ev.params      = Some(serde_json::json!({
                    "path": path,
                    "agent": agent_arg,
                    "session_id": session_id_arg,
                }).to_string());
                ev.duration_ms             = Some(duration_ms);
                ev.actual_tokens           = Some(chars_to_tokens(msg.len()));
                ev.estimated_origin_tokens = Some(0);
                ev.insert();

                Ok(CallToolResult::success(vec![Content::text(msg)]))
            })
        },
    )
}
