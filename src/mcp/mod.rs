use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRoute, tool::ToolCallContext},
    model::{CallToolResult, Content, ListToolsResult, ServerCapabilities, ServerInfo, Tool},
    service::{MaybeSendFuture, RequestContext},
    transport::stdio,
};

pub async fn run_stdio_server() -> Result<()> {
    let server = BuiltinServer::new();
    server.serve(stdio()).await?.waiting().await?;
    Ok(())
}

pub(crate) struct BuiltinServer {
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

impl BuiltinServer {
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
                "Read file content by path. mode=full (default) or outline",
                so_read_schema(),
            ),
            |ctx| Box::pin(async move { so_read_tool(ctx) }),
        ));

        Self { tool_router }
    }
}

impl ServerHandler for BuiltinServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "so-context MCP server for context-efficient file access.\n\
Use `so_read` with:\n\
- `path` (required): absolute or relative file path\n\
- `mode` (optional): `full` (default) or `outline`\n\
Prefer `outline` for quick structure, then `full` only when needed.",
        )
    }

    fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + MaybeSendFuture + '_
    {
        async move { self.tool_router.call(ToolCallContext::new(self, request, context)).await }
    }

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

fn so_read_schema() -> Arc<serde_json::Map<String, serde_json::Value>> {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "mode": { "type": "string", "enum": ["full", "outline"] }
        },
        "required": ["path"]
    })
    .as_object()
    .cloned()
    .unwrap_or_default();

    Arc::new(schema)
}

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
