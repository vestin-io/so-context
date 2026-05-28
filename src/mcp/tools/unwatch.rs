//! `so_unwatch` tool — deregister a project path from watching.

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
        move |ctx: ToolCallContext<'_, BuiltinServer>| {
            let wm = Arc::clone(&wm);
            Box::pin(async move {
                let agent_caller      = ctx.service.agent();
                let session_id_caller = ctx.service.session_id();
                let args = ctx.arguments.as_ref();
                let path = args
                    .and_then(|a| a.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(".")
                    .to_string();
                let agent_arg = args.and_then(|a| a.get("agent")).and_then(|v| v.as_str()).map(str::to_string);
                let session_id_arg = args.and_then(|a| a.get("session_id")).and_then(|v| v.as_str()).map(str::to_string);

                let timer = Timer::start();
                let removed = wm.unwatch(&path, agent_arg.as_deref(), session_id_arg.as_deref());
                let duration_ms = timer.elapsed_ms();

                let msg = if removed {
                    format!(
                        "unwatched: {path} (agent: {}, session: {})",
                        agent_arg.as_deref().unwrap_or("unknown"),
                        session_id_arg.as_deref().unwrap_or("auto"),
                    )
                } else {
                    format!("not watched: {path}")
                };

                let mut ev = EventRecord::new(&agent_caller, &session_id_caller, "so_unwatch");
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
