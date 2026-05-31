//! `so_references` tool — find all usages of a symbol across the project graph.

use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use crate::core_events::{EventRecord, Timer, enqueue};
use crate::core_graph;
use crate::core_tokens::count_tokens;

pub fn route() -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_references",
            "Find all usages of a named symbol (callers, callees, imports, or all). \
             Run so_read(mode=graph) or graph_watch first to build the index.",
            schema(),
        ),
        |ctx| Box::pin(async move { handler(ctx) }),
    )
}

fn handler(ctx: ToolCallContext<'_, BuiltinServer>) -> Result<CallToolResult, rmcp::ErrorData> {
    let client         = ctx.service.client();
    let client_version = ctx.service.client_version();
    let connection_id  = ctx.service.connection_id();

    let args = ctx
        .arguments
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

    let (session_id, session_source) = match args
        .get("_so_session_id")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
    {
        Some(sid) => (sid.to_string(), "hook"),
        None      => (connection_id,   "connection"),
    };

    let symbol = args
        .get("symbol")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing string argument: symbol", None))?;

    let path = args
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(".");

    let kind = args
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("all");

    let include_declaration = args
        .get("include_declaration")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    let timer = Timer::start();
    let call_result = core_graph::references_project(path, symbol, kind, include_declaration);
    let duration_ms = timer.elapsed_ms();

    let mut ev = EventRecord::new(&session_id, "so_references");
    ev.client         = client;
    ev.client_version = client_version;
    ev.client_source  = "client_info".to_string();
    ev.session_source = session_source.to_string();
    ev.project        = Some(path.to_string());
    ev.params         = Some(serde_json::json!({
        "symbol": symbol, "path": path, "kind": kind,
        "include_declaration": include_declaration,
    }).to_string());
    ev.duration_ms    = Some(duration_ms);

    match call_result {
        Ok(refs) => {
            let output = format_references(symbol, kind, &refs);
            ev.actual_tokens = Some(count_tokens(&output));
            ev.actual_size   = Some(output.len() as i64);
            ev.result_ok     = true;
            enqueue(ev);
            Ok(CallToolResult::success(vec![Content::text(output)]))
        }
        Err(e) => {
            ev.result_ok = false;
            enqueue(ev);
            Err(rmcp::ErrorData::internal_error(e, None))
        }
    }
}

fn format_references(
    symbol: &str,
    kind: &str,
    refs: &[core_graph::ReferenceEntry],
) -> String {
    if refs.is_empty() {
        return format!("No references found for `{symbol}` (kind={kind}).");
    }

    let mut lines = Vec::with_capacity(refs.len() + 1);
    lines.push(format!(
        "References for `{symbol}` (kind={kind}): {} result(s)\n",
        refs.len()
    ));

    for r in refs {
        lines.push(format!(
            "{}:{} [{}] {} — {}",
            r.file, r.line, r.sym_kind, r.symbol, r.ref_kind
        ));
    }

    lines.join("\n")
}

fn schema() -> Arc<JsonObject> {
    Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "Name of the symbol to find references for."
                },
                "path": {
                    "type": "string",
                    "default": ".",
                    "description": "Project root path. Defaults to current directory."
                },
                "kind": {
                    "type": "string",
                    "enum": ["all", "callers", "callees", "imports"],
                    "default": "all",
                    "description": "Filter reference direction. 'all' returns every edge involving the symbol."
                },
                "include_declaration": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include the symbol's own definition site in results."
                },
                "_so_session_id": {
                    "type": "string",
                    "description": "Agent session ID injected by the so-context PreToolUse hook. Do not set manually."
                }
            },
            "required": ["symbol"]
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    )
}
