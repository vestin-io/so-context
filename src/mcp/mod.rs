use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRoute, tool::ToolCallContext},
    model::{CallToolResult, Content, ListToolsResult, ServerCapabilities, ServerInfo, Tool},
    service::{MaybeSendFuture, RequestContext},
    transport::stdio,
};

/// Starts the built-in MCP server over stdio transport and blocks until exit.
pub async fn run_stdio_server() -> Result<()> {
    let server = BuiltinServer::new();
    server.serve(stdio()).await?.waiting().await?;
    Ok(())
}

pub(crate) struct BuiltinServer {
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

impl BuiltinServer {
    /// Constructs the built-in MCP server and registers all tool routes.
    fn new() -> Self {
        let mut tool_router = rmcp::handler::server::router::tool::ToolRouter::<Self>::new();
        tool_router.add_route(ToolRoute::new_dyn(
            Tool::new(
                "ping",
                "Return pong from the built-in daemon",
                Arc::new(Default::default()),
            ),
            |_ctx| {
                Box::pin(async {
                    Ok(CallToolResult::success(vec![Content::text(
                        "pong".to_string(),
                    )]))
                })
            },
        ));
        tool_router.add_route(ToolRoute::new_dyn(
            Tool::new(
                "so_read",
                "Read file/project by path. mode=full (default), outline, or graph.",
                so_read_schema(),
            ),
            |ctx| Box::pin(async move { so_read_tool(ctx) }),
        ));
        tool_router.add_route(ToolRoute::new_dyn(
            Tool::new(
                "so_search",
                "Search indexed project code graph (FTS). Run graph index first.",
                so_search_schema(),
            ),
            |ctx| Box::pin(async move { so_search_tool(ctx) }),
        ));

        Self { tool_router }
    }
}

impl ServerHandler for BuiltinServer {
    /// Returns server capabilities and high-level usage instructions.
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "so-context MCP server for context-efficient file access.\n\
Use `so_read` with:\n\
- `path` (required): file path for `full/outline`, project folder path for `graph`\n\
- `mode` (optional): `full` (default), `outline`, or `graph`\n\
Prefer `outline` for quick structure, `full` for full content, and `graph` to index project code into SQLite.\n\
Use `so_search` to search indexed symbols/content in the project graph.",
        )
    }

    /// Dispatches incoming tool call requests to the registered tool router.
    fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + MaybeSendFuture + '_
    {
        async move { self.tool_router.call(ToolCallContext::new(self, request, context)).await }
    }

    /// Returns the list of currently registered tools.
    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + MaybeSendFuture + '_
    {
        async move {
            Ok(ListToolsResult {
                tools: self.tool_router.list_all(),
                ..Default::default()
            })
        }
    }
}

/// Builds JSON schema for `so_read` tool arguments.
fn so_read_schema() -> Arc<serde_json::Map<String, serde_json::Value>> {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "mode": { "type": "string", "enum": ["full", "outline", "graph"] }
        },
        "required": ["path"]
    })
    .as_object()
    .cloned()
    .unwrap_or_default();

    Arc::new(schema)
}

/// Handles `so_read` tool invocation and returns the resulting text content.
fn so_read_tool(ctx: ToolCallContext<'_, BuiltinServer>) -> Result<CallToolResult, rmcp::ErrorData> {
    let args = ctx
        .arguments
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

    let path = args
        .get("path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing string argument: path", None))?;

    let mode = args
        .get("mode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("full");

    let output =
        crate::core_read::read(path, mode).map_err(|e| rmcp::ErrorData::internal_error(e, None))?;

    Ok(CallToolResult::success(vec![Content::text(output)]))
}

/// Builds JSON schema for `so_search` tool arguments.
fn so_search_schema() -> Arc<serde_json::Map<String, serde_json::Value>> {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "query": { "type": "string" },
            "path": { "type": "string", "default": "." },
            "limit": { "type": "integer", "minimum": 1, "maximum": 200, "default": 20 }
        },
        "required": ["query"]
    })
    .as_object()
    .cloned()
    .unwrap_or_default();

    Arc::new(schema)
}

/// Handles `so_search` tool invocation and returns search results as text.
fn so_search_tool(
    ctx: ToolCallContext<'_, BuiltinServer>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let args = ctx
        .arguments
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

    let query = args
        .get("query")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing string argument: query", None))?;

    let path = args
        .get("path")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(".");

    let limit = args
        .get("limit")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(20) as usize;

    let output = crate::core_graph::search_project(path, query, limit)
        .map_err(|e| rmcp::ErrorData::internal_error(e, None))?;

    Ok(CallToolResult::success(vec![Content::text(output)]))
}
