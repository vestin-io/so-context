//! `so_read` tool — read a file or return a compact outline.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use crate::core_events::{EventRecord, enqueue};
use crate::core_tokens::count_tokens;
use crate::file_visit_cache::hash_content;

pub fn route() -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_read",
            "Read file by path. mode=full (default) returns full content; mode=outline returns a compact symbol outline from the graph DB.",
            schema(),
        ),
        |ctx| Box::pin(async move { handler(ctx) }),
    )
}

fn handler(ctx: ToolCallContext<'_, BuiltinServer>) -> Result<CallToolResult, rmcp::ErrorData> {
    let client = ctx.service.client();
    let client_version = ctx.service.client_version();
    let connection_id = ctx.service.connection_id();
    let fvc = &ctx.service.file_visit_cache;

    let args = ctx
        .arguments
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

    // Agent session ID injected by the PreToolUse hook on the agent side.
    // Falls back to the connection_id when not provided (e.g. agents without hook support).
    let (session_id, session_source) = match args
        .get("_so_session_id")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
    {
        Some(sid) => (sid.to_string(), "hook"),
        None => (connection_id.clone(), "connection"),
    };

    let path = args
        .get("path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing string argument: path", None))?;

    let mode = args
        .get("mode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("full");

    // -----------------------------------------------------------------------
    // File-visit cache check — full mode only, mirrors lean-ctx behaviour.
    // -----------------------------------------------------------------------
    if mode == "full" {
        // Read file content upfront so we can hash it regardless of the cache
        // decision — we need the hash to detect modifications.
        let raw_content = std::fs::read_to_string(path).map_err(|e| {
            rmcp::ErrorData::internal_error(format!("failed to read file: {e}"), None)
        })?;

        let current_hash = hash_content(&raw_content);

        if let Some(entry) = fvc.get_file(&connection_id, &session_id, path) {
            if entry.content_hash == current_hash {
                // File already in context and unchanged — return a compact stub.
                let line_count = raw_content.lines().count();
                let short = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path);
                let msg = format!(
                    "{short} [unchanged, {line_count}L, use cached context]\n\
                     File unchanged since last read. Reuse existing context instead of re-reading."
                );

                let mut ev = EventRecord::new(&session_id, "so_read");
                ev.client = client.clone();
                ev.client_version = client_version.clone();
                ev.client_source = "client_info".to_string();
                ev.session_source = session_source.to_string();
                ev.project = Some(path.to_string());
                ev.params = Some(serde_json::json!({ "path": path, "mode": mode }).to_string());
                ev.duration_ms = Some(0);
                ev.estimated_origin_tokens = Some(count_tokens(&raw_content));
                ev.estimated_origin_size = Some(raw_content.len() as i64);
                ev.actual_tokens = Some(count_tokens(&msg));
                ev.actual_size = Some(msg.len() as i64);
                ev.result_ok = true;
                enqueue(ev);

                return Ok(CallToolResult::success(vec![Content::text(msg)]));
            }
            // File was read before but has changed — fall through to full read.
        }

        let token_count = count_tokens(&raw_content);
        fvc.add_file(&connection_id, &session_id, path, token_count, current_hash);

        let mut ev = EventRecord::new(&session_id, "so_read");
        ev.client = client;
        ev.client_version = client_version;
        ev.client_source = "client_info".to_string();
        ev.session_source = session_source.to_string();
        ev.project = Some(path.to_string());
        ev.params = Some(serde_json::json!({ "path": path, "mode": mode }).to_string());
        ev.duration_ms = Some(0);
        ev.actual_tokens = Some(token_count);
        ev.estimated_origin_tokens = Some(token_count);
        ev.actual_size = Some(raw_content.len() as i64);
        ev.estimated_origin_size = Some(raw_content.len() as i64);
        ev.result_ok = true;
        enqueue(ev);

        return Ok(CallToolResult::success(vec![Content::text(raw_content)]));
    }

    // -----------------------------------------------------------------------
    // Outline mode — query graph DB, fall back to regex scan.
    // -----------------------------------------------------------------------
    if mode == "outline" {
        let raw_content = std::fs::read_to_string(path).map_err(|e| {
            rmcp::ErrorData::internal_error(format!("failed to read file: {e}"), None)
        })?;

        let output = crate::core_read::build_outline_for_path(path, &raw_content);
        let token_count = count_tokens(&output);

        let mut ev = EventRecord::new(&session_id, "so_read");
        ev.client = client;
        ev.client_version = client_version;
        ev.client_source = "client_info".to_string();
        ev.session_source = session_source.to_string();
        ev.project = Some(path.to_string());
        ev.params = Some(serde_json::json!({ "path": path, "mode": mode }).to_string());
        ev.duration_ms = Some(0);
        ev.actual_tokens = Some(token_count);
        ev.estimated_origin_tokens = Some(count_tokens(&raw_content));
        ev.actual_size = Some(output.len() as i64);
        ev.estimated_origin_size = Some(raw_content.len() as i64);
        ev.result_ok = true;
        enqueue(ev);

        return Ok(CallToolResult::success(vec![Content::text(output)]));
    }

    Err(rmcp::ErrorData::invalid_params(
        format!("unsupported mode: {mode}; expected full or outline"),
        None,
    ))
}

fn schema() -> Arc<JsonObject> {
    Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "mode": { "type": "string", "enum": ["full", "outline"] },
                "_so_session_id": {
                    "type": "string",
                    "description": "Agent session ID injected by the so-context PreToolUse hook. Do not set manually."
                }
            },
            "required": ["path"]
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    )
}
