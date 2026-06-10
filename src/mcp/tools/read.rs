//! `so_read` tool — read a file or return a compact outline.

use std::ops::RangeInclusive;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use super::infer_connection_project_for_path;
use crate::core_events::{EventRecord, Timer, enqueue};
use crate::core_tokens::count_tokens;
use crate::daemon::WatchManager;
use crate::file_visit_cache::hash_content;
use crate::mcp::prefers_plain_text_tool_output;

pub fn route(wm: Arc<WatchManager>) -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_read",
            "Read file by path. Prefer this over native Read/View for source and config files. mode=full (default) returns full content; mode=outline returns a compact symbol outline from the graph DB. Optional start_line/end_line return an excerpt range and bypass unchanged-file cache notices. Treat the returned text as the authoritative result unless the structured content explicitly says it is only a cached-read notice.",
            schema(),
        ),
        move |ctx| {
            let wm = Arc::clone(&wm);
            Box::pin(async move { handler(ctx, &wm) })
        },
    )
}

fn handler(
    ctx: ToolCallContext<'_, BuiltinServer>,
    wm: &WatchManager,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let client = ctx.service.client();
    let client_version = ctx.service.client_version();
    let connection_id = ctx.service.connection_id();
    let fvc = &ctx.service.file_visit_cache;
    let client_prefers_plain_text = prefers_plain_text_tool_output(client.as_deref());

    let args = ctx
        .arguments
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

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
    let excerpt = parse_excerpt_request(&args)?;
    let project = infer_connection_project_for_path(
        &wm.status(),
        client.as_deref().unwrap_or("unknown"),
        &connection_id,
        path,
    )
    .map(|path| path.display().to_string());
    let timer = Timer::start();

    if mode == "full" {
        let raw_content = std::fs::read_to_string(path).map_err(|e| {
            rmcp::ErrorData::internal_error(format!("failed to read file: {e}"), None)
        })?;
        let line_count = raw_content.lines().count();
        let current_hash = hash_content(&raw_content);

        if excerpt.is_none()
            && let Some(entry) = fvc.file(&connection_id, &session_id, path)
            && entry.content_hash == current_hash
        {
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
            ev.project = project.clone();
            ev.params = Some(serde_json::json!({ "path": path, "mode": mode }).to_string());
            ev.duration_ms = Some(timer.elapsed_ms());
            ev.estimated_origin_tokens = Some(count_tokens(&raw_content));
            ev.estimated_origin_size = Some(raw_content.len() as i64);
            ev.actual_tokens = Some(count_tokens(&msg));
            ev.actual_size = Some(msg.len() as i64);
            ev.result_ok = true;
            enqueue(ev);

            let mut tool_result = CallToolResult::success(vec![Content::text(msg)]);
            if !client_prefers_plain_text {
                tool_result.structured_content =
                    Some(build_read_structured_content(&ReadStructuredContent {
                        path,
                        mode,
                        line_count,
                        current_text_is_authoritative: false,
                        current_text_contains_full_file: false,
                        cached_notice_only: true,
                        excerpt_range: None,
                        line_numbers: false,
                    }));
            }
            return Ok(tool_result);
        }

        let output = if let Some(excerpt) = &excerpt {
            render_excerpt(&raw_content, excerpt)
        } else {
            raw_content.clone()
        };

        if excerpt.is_none() {
            fvc.add_file(
                &connection_id,
                &session_id,
                path,
                count_tokens(&raw_content),
                current_hash,
            );
        }

        let mut ev = EventRecord::new(&session_id, "so_read");
        ev.client = client;
        ev.client_version = client_version;
        ev.client_source = "client_info".to_string();
        ev.session_source = session_source.to_string();
        ev.project = project.clone();
        ev.params = Some(
            serde_json::json!({
                "path": path,
                "mode": mode,
                "start_line": excerpt.as_ref().map(|request| request.start_line),
                "end_line": excerpt.as_ref().map(|request| request.end_line),
                "line_numbers": excerpt.as_ref().map(|request| request.line_numbers),
            })
            .to_string(),
        );
        ev.duration_ms = Some(timer.elapsed_ms());
        ev.actual_tokens = Some(count_tokens(&output));
        ev.estimated_origin_tokens = Some(count_tokens(&raw_content));
        ev.actual_size = Some(output.len() as i64);
        ev.estimated_origin_size = Some(raw_content.len() as i64);
        ev.result_ok = true;
        enqueue(ev);

        let mut tool_result = CallToolResult::success(vec![Content::text(output)]);
        if !client_prefers_plain_text {
            tool_result.structured_content =
                Some(build_read_structured_content(&ReadStructuredContent {
                    path,
                    mode,
                    line_count,
                    current_text_is_authoritative: true,
                    current_text_contains_full_file: excerpt.is_none(),
                    cached_notice_only: false,
                    excerpt_range: excerpt
                        .as_ref()
                        .map(|request| request.start_line..=request.end_line),
                    line_numbers: excerpt
                        .as_ref()
                        .map(|request| request.line_numbers)
                        .unwrap_or(false),
                }));
        }
        return Ok(tool_result);
    }

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
        ev.project = project;
        ev.params = Some(serde_json::json!({ "path": path, "mode": mode }).to_string());
        ev.duration_ms = Some(timer.elapsed_ms());
        ev.actual_tokens = Some(token_count);
        ev.estimated_origin_tokens = Some(count_tokens(&raw_content));
        ev.actual_size = Some(output.len() as i64);
        ev.estimated_origin_size = Some(raw_content.len() as i64);
        ev.result_ok = true;
        enqueue(ev);

        let mut tool_result = CallToolResult::success(vec![Content::text(output)]);
        if !client_prefers_plain_text {
            tool_result.structured_content =
                Some(build_read_structured_content(&ReadStructuredContent {
                    path,
                    mode,
                    line_count: raw_content.lines().count(),
                    current_text_is_authoritative: true,
                    current_text_contains_full_file: false,
                    cached_notice_only: false,
                    excerpt_range: None,
                    line_numbers: false,
                }));
        }
        return Ok(tool_result);
    }

    Err(rmcp::ErrorData::invalid_params(
        format!("unsupported mode: {mode}; expected full or outline"),
        None,
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExcerptRequest {
    start_line: usize,
    end_line: usize,
    line_numbers: bool,
}

fn parse_excerpt_request(args: &JsonObject) -> Result<Option<ExcerptRequest>, rmcp::ErrorData> {
    let start_line = args
        .get("start_line")
        .and_then(serde_json::Value::as_u64)
        .map(|value| value as usize);
    let end_line = args
        .get("end_line")
        .and_then(serde_json::Value::as_u64)
        .map(|value| value as usize);

    if start_line.is_none() && end_line.is_none() {
        return Ok(None);
    }

    let start_line = start_line.unwrap_or(1);
    let end_line = end_line.unwrap_or(start_line);
    if start_line == 0 || end_line == 0 {
        return Err(rmcp::ErrorData::invalid_params(
            "start_line and end_line must be positive integers",
            None,
        ));
    }
    if end_line < start_line {
        return Err(rmcp::ErrorData::invalid_params(
            "end_line must be greater than or equal to start_line",
            None,
        ));
    }

    let line_numbers = args
        .get("line_numbers")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    Ok(Some(ExcerptRequest {
        start_line,
        end_line,
        line_numbers,
    }))
}

fn render_excerpt(content: &str, excerpt: &ExcerptRequest) -> String {
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_no = index + 1;
            if line_no < excerpt.start_line || line_no > excerpt.end_line {
                return None;
            }
            Some(if excerpt.line_numbers {
                format!("{line_no}: {line}")
            } else {
                line.to_string()
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct ReadStructuredContent<'a> {
    path: &'a str,
    mode: &'a str,
    line_count: usize,
    current_text_is_authoritative: bool,
    current_text_contains_full_file: bool,
    cached_notice_only: bool,
    excerpt_range: Option<RangeInclusive<usize>>,
    line_numbers: bool,
}

fn build_read_structured_content(content: &ReadStructuredContent<'_>) -> serde_json::Value {
    serde_json::json!({
        "path": content.path,
        "mode": content.mode,
        "content_kind": if content.cached_notice_only {
            "cached_read_notice"
        } else if content.excerpt_range.is_some() {
            "file_excerpt"
        } else if content.mode == "outline" {
            "file_outline"
        } else {
            "file_content"
        },
        "line_count": content.line_count,
        "current_text_is_authoritative": content.current_text_is_authoritative,
        "current_text_contains_full_file": content.current_text_contains_full_file,
        "preferred_response_source": if content.current_text_is_authoritative {
            "current_text_content"
        } else {
            "cached_context"
        },
        "reread_not_needed_if_text_sufficient": true,
        "result_complete": !content.cached_notice_only,
        "cached_notice_only": content.cached_notice_only,
        "excerpt_start_line": content.excerpt_range.as_ref().map(|range| *range.start()),
        "excerpt_end_line": content.excerpt_range.as_ref().map(|range| *range.end()),
        "line_numbers": content.line_numbers,
    })
}

fn schema() -> Arc<JsonObject> {
    Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "mode": { "type": "string", "enum": ["full", "outline"] },
                "start_line": { "type": "integer", "minimum": 1 },
                "end_line": { "type": "integer", "minimum": 1 },
                "line_numbers": { "type": "boolean", "default": false },
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

#[cfg(test)]
#[path = "read_tests.rs"]
mod tests;
