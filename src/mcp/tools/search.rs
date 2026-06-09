//! `so_search` tool — FTS search over an indexed project graph.

use std::collections::HashSet;
use std::sync::Arc;

use ignore::WalkBuilder;
use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use super::resolve_project_path_arg;
use crate::core_events::{EventRecord, Timer, enqueue};
use crate::core_graph;
use crate::core_tokens::count_tokens;
use crate::daemon::WatchManager;
use crate::mcp::prefers_plain_text_tool_output;

pub fn route(wm: Arc<WatchManager>) -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_search",
            "Search the indexed project code graph (FTS). Prefer this over native grep-style tools when indexed project search is enough. Treat the returned text as authoritative for the shown hits; when no graph DB exists yet, so_search falls back to a lightweight text search instead of hard failing.",
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

    let query = args
        .get("query")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing string argument: query", None))?;

    let path = resolve_project_path_arg(&args, "path", &client, &connection_id, &wm.status())?;
    let path_display = path.display().to_string();

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(20) as usize;

    let timer = Timer::start();
    let call_result = match core_graph::search_project_with_stats(&path_display, query, limit) {
        Ok((output, matched_files_tokens, matched_files_size)) => Ok(SearchResult {
            output,
            matched_files_tokens,
            matched_files_size,
            backend: "graph_fts",
        }),
        Err(error) if error.contains("graph db not found") => {
            fallback_search_project_with_stats(&path_display, query, limit).map(
                |(output, matched_files_tokens, matched_files_size)| SearchResult {
                    output,
                    matched_files_tokens,
                    matched_files_size,
                    backend: "text_fallback",
                },
            )
        }
        Err(error) => Err(error),
    };
    let duration_ms = timer.elapsed_ms();

    let mut ev = EventRecord::new(&session_id, "so_search");
    ev.client = client;
    ev.client_version = client_version;
    ev.client_source = "client_info".to_string();
    ev.session_source = session_source.to_string();
    ev.project = Some(path_display.clone());
    ev.params = Some(
        serde_json::json!({ "query": query, "path": path_display, "limit": limit }).to_string(),
    );
    ev.duration_ms = Some(duration_ms);

    match call_result {
        Ok(result) => {
            ev.actual_tokens = Some(count_tokens(&result.output));
            ev.estimated_origin_tokens = Some(result.matched_files_tokens);
            ev.actual_size = Some(result.output.len() as i64);
            ev.estimated_origin_size = Some(result.matched_files_size);
            ev.result_ok = true;
            enqueue(ev);

            let shown_result_count = count_search_results(&result.output);
            let mut tool_result = CallToolResult::success(vec![Content::text(result.output)]);
            if !client_prefers_plain_text {
                tool_result.structured_content = Some(build_search_structured_content(
                    query,
                    &path_display,
                    limit,
                    shown_result_count,
                    result.backend,
                ));
            }
            Ok(tool_result)
        }
        Err(e) => {
            ev.result_ok = false;
            enqueue(ev);
            Err(rmcp::ErrorData::internal_error(e, None))
        }
    }
}

struct SearchResult {
    output: String,
    matched_files_tokens: i64,
    matched_files_size: i64,
    backend: &'static str,
}

fn fallback_search_project_with_stats(
    project_path: &str,
    query: &str,
    limit: usize,
) -> Result<(String, i64, i64), String> {
    let terms = parse_fallback_terms(query);
    if terms.is_empty() {
        return Ok(("No results.".to_string(), 0, 0));
    }

    let mut results = Vec::new();
    let mut matched_paths = HashSet::new();

    for entry in WalkBuilder::new(project_path)
        .standard_filters(true)
        .filter_entry(|entry| {
            let s = entry.path().to_string_lossy();
            !s.contains("/.so-context/")
        })
        .build()
    {
        if results.len() >= limit {
            break;
        }

        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        for (index, line) in content.lines().enumerate() {
            if line_matches_terms(line, &terms) {
                results.push(format!("{}:{} [text] {}", path.display(), index + 1, line));
                matched_paths.insert(path.to_string_lossy().to_string());
                if results.len() >= limit {
                    break;
                }
            }
        }
    }

    let mut matched_files_tokens = 0i64;
    let mut matched_files_size = 0i64;
    for path in matched_paths {
        if let Ok(content) = std::fs::read_to_string(&path) {
            matched_files_tokens += count_tokens(&content);
            matched_files_size += content.len() as i64;
        }
    }

    if results.is_empty() {
        Ok(("No results.".to_string(), 0, 0))
    } else {
        Ok((results.join("\n"), matched_files_tokens, matched_files_size))
    }
}

fn parse_fallback_terms(query: &str) -> Vec<String> {
    let normalized = query.trim();
    if normalized.is_empty() {
        return Vec::new();
    }

    let or_terms: Vec<String> = normalized
        .split(" OR ")
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(strip_query_wrappers)
        .collect();

    if or_terms.len() > 1 {
        return or_terms;
    }

    vec![strip_query_wrappers(normalized)]
}

fn strip_query_wrappers(term: &str) -> String {
    term.trim_matches('"').trim_matches('\'').to_string()
}

fn line_matches_terms(line: &str, terms: &[String]) -> bool {
    terms
        .iter()
        .any(|needle| !needle.is_empty() && line.contains(needle))
}

fn count_search_results(output: &str) -> usize {
    if output.trim().is_empty() || output.trim() == "No results." {
        0
    } else {
        output.lines().count()
    }
}

fn build_search_structured_content(
    query: &str,
    path: &str,
    limit: usize,
    shown_result_count: usize,
    backend: &str,
) -> serde_json::Value {
    let may_have_more_results = shown_result_count >= limit;
    serde_json::json!({
        "query": query,
        "path": path,
        "limit": limit,
        "content_kind": "search_results",
        "search_backend": backend,
        "shown_result_count": shown_result_count,
        "result_complete": !may_have_more_results,
        "may_have_more_results": may_have_more_results,
        "current_text_is_authoritative": true,
        "preferred_response_source": "current_text_content",
        "rerun_not_needed_if_text_sufficient": true,
    })
}

fn schema() -> Arc<JsonObject> {
    Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "path": {
                    "type": "string",
                    "description": "Project root path. Defaults to the sole watched project for the current MCP connection."
                },
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 20 },
                "_so_session_id": {
                    "type": "string",
                    "description": "Agent session ID injected by the so-context PreToolUse hook. Do not set manually."
                }
            },
            "required": ["query"]
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        build_search_structured_content, count_search_results, line_matches_terms,
        parse_fallback_terms,
    };

    #[test]
    fn no_results_count_as_zero() {
        assert_eq!(count_search_results("No results."), 0);
    }

    #[test]
    fn search_result_metadata_marks_limit_saturation_as_partial() {
        let content = build_search_structured_content("foo", "/tmp/project", 2, 2, "graph_fts");

        assert_eq!(content["content_kind"], "search_results");
        assert_eq!(content["shown_result_count"], 2);
        assert_eq!(content["result_complete"], false);
        assert_eq!(content["may_have_more_results"], true);
    }

    #[test]
    fn fallback_terms_support_or_queries() {
        assert_eq!(
            parse_fallback_terms("\"foo\" OR bar"),
            vec!["foo".to_string(), "bar".to_string()]
        );
    }

    #[test]
    fn fallback_line_matching_is_case_sensitive() {
        assert!(line_matches_terms(
            "ConfigRepository loads recommendation files",
            &["ConfigRepository".to_string()]
        ));
        assert!(!line_matches_terms(
            "ConfigRepository loads recommendation files",
            &["configrepository".to_string()]
        ));
    }
}
