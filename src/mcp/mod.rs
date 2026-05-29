//! MCP server — exposes daemon capabilities to agent hosts over stdio.
//!
//! Auto-discovery hooks (no explicit "start watching" tool needed):
//!
//! 1. `initialize()` — fires on MCP handshake. Extracts `rootUri` /
//!    `workspaceFolders` from the client params and registers them with
//!    [`ProjectRegistry`] immediately.
//!
//! 2. `on_initialized()` — fires after the handshake ACK. If the client
//!    advertises `roots` capability, sends `roots/list` to retrieve all
//!    workspace roots and registers each one.
//!
//! 3. `call_tool()` pre-hook — last resort: if still no project is registered,
//!    falls back to `std::env::current_dir()` before dispatching the tool.
//!
//! Multiple agent sessions all share the same [`ProjectRegistry`] /
//! [`WatchManager`]. Duplicate roots are silently de-duplicated — N agents
//! pointing at the same repo = 1 watcher thread.

pub mod tools;

use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext},
    model::{
        CallToolResult, ClientCapabilities, ErrorData, InitializeRequestParams, InitializeResult,
        ListToolsResult, ServerCapabilities, ServerInfo,
    },
    service::{MaybeSendFuture, NotificationContext, RequestContext},
    transport::stdio,
};

use crate::daemon::WatchManager;
use crate::daemon::watch_manager::file_uri_to_path;

const ROOTS_LIST_TIMEOUT_MS: u64 = 5_000;

/// Starts the MCP stdio server. Blocks until the transport closes.
pub async fn run_stdio_server(watch_manager: Arc<WatchManager>) -> Result<()> {
    let server = BuiltinServer::new(watch_manager);
    server.serve(stdio()).await?.waiting().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

pub(crate) struct BuiltinServer {
    tool_router: ToolRouter<Self>,
    wm: Arc<WatchManager>,
    client_supports_roots: std::sync::atomic::AtomicBool,
}

impl BuiltinServer {
    fn new(wm: Arc<WatchManager>) -> Self {
        let mut tool_router = ToolRouter::<Self>::new();
        tool_router.add_route(tools::read::route());
        tool_router.add_route(tools::search::route());
        tool_router.add_route(tools::status::route(Arc::clone(&wm)));

        Self {
            tool_router,
            wm,
            client_supports_roots: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl ServerHandler for BuiltinServer {
    // -----------------------------------------------------------------------
    // Handshake — hook 1: extract roots from initialize params
    // -----------------------------------------------------------------------

    fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<InitializeResult, ErrorData>> + MaybeSendFuture + '_
    {
        // Record whether the client supports roots/list.
        let supports_roots = matches!(
            &request.capabilities,
            ClientCapabilities { roots: Some(_), .. }
        );
        self.client_supports_roots
            .store(supports_roots, std::sync::atomic::Ordering::Relaxed);

        // Extract rootUri / workspaceFolders from raw client_info extensions
        // or fall back to any path hinted in the context peer info.
        // rmcp stores the full params on the peer — inspect raw capabilities.
        // We also check for a `rootUri` field that some clients send.
        if let Some(peer_info) = context.peer.peer_info() {
            // peer_info is the InitializeRequestParams — serialise to JSON
            // and pull out known path fields without a custom deserializer.
            if let Ok(v) = serde_json::to_value(&peer_info) {
                for key in ["rootUri", "root_uri"] {
                    if let Some(uri) = v.get(key).and_then(|v| v.as_str()) {
                        self.wm.ensure_watching(&file_uri_to_path(uri));
                    }
                }
                if let Some(folders) = v.get("workspaceFolders").and_then(|v| v.as_array()) {
                    for folder in folders {
                        if let Some(uri) = folder.get("uri").and_then(|v| v.as_str()) {
                            self.wm.ensure_watching(&file_uri_to_path(uri));
                        }
                    }
                }
            }
        }

        // Delegate to default (sets peer info, returns get_info()).
        async move {
            if context.peer.peer_info().is_none() {
                context.peer.set_peer_info(request);
            }
            Ok(self.get_info())
        }
    }

    // -----------------------------------------------------------------------
    // Post-handshake — hook 2: roots/list from client
    // -----------------------------------------------------------------------

    fn on_initialized(
        &self,
        context: NotificationContext<RoleServer>,
    ) -> impl std::future::Future<Output = ()> + MaybeSendFuture + '_ {
        async move {
            if !self
                .client_supports_roots
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                // Client doesn't support roots — fall back to cwd.
                if let Ok(cwd) = std::env::current_dir() {
                    self.wm.ensure_watching(cwd.to_string_lossy().as_ref());
                }
                return;
            }

            // Ask the client for its workspace roots with a timeout.
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(ROOTS_LIST_TIMEOUT_MS),
                context.peer.list_roots(),
            )
            .await;

            match result {
                Ok(Ok(roots_result)) => {
                    if roots_result.roots.is_empty() {
                        eprintln!("[mcp] client returned no roots; falling back to cwd");
                        if let Ok(cwd) = std::env::current_dir() {
                            self.wm.ensure_watching(cwd.to_string_lossy().as_ref());
                        }
                    } else {
                        for root in &roots_result.roots {
                            self.wm.ensure_watching(&file_uri_to_path(&root.uri));
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("[mcp] roots/list failed: {e}; falling back to cwd");
                    if let Ok(cwd) = std::env::current_dir() {
                        self.wm.ensure_watching(cwd.to_string_lossy().as_ref());
                    }
                }
                Err(_timeout) => {
                    eprintln!("[mcp] roots/list timed out; falling back to cwd");
                    if let Ok(cwd) = std::env::current_dir() {
                        self.wm.ensure_watching(cwd.to_string_lossy().as_ref());
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Tool dispatch — hook 3: cwd last-resort before every tool call
    // -----------------------------------------------------------------------

    fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + MaybeSendFuture + '_
    {
        async move {
            // Last resort: if no project has been registered yet, try cwd now.
            // This covers clients that never send roots and whose on_initialized
            // raced with the first tool call.
            if let Ok(cwd) = std::env::current_dir() {
                self.wm.ensure_watching(cwd.to_string_lossy().as_ref());
            }

            self.tool_router
                .call(ToolCallContext::new(self, request, context))
                .await
        }
    }

    // -----------------------------------------------------------------------
    // Other
    // -----------------------------------------------------------------------

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "so-context MCP server.\n\
\n\
Projects are auto-discovered from workspace roots on connect — no setup needed.\n\
\n\
Tools:\n\
  so_read      — read a file (mode: full / outline / graph)\n\
  so_search    — FTS search over an indexed project graph\n\
  so_status    — list all watched projects and their current state",
        )
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
