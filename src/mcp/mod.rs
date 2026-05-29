//! MCP module.
//!
//! # Two roles
//!
//! **Daemon side** (`BuiltinServer`): the actual MCP server implementation,
//! served by the daemon over a Unix socket via streamable HTTP.  The daemon
//! constructs one `BuiltinServer` per client session.
//!
//! **Bridge** (`run_mcp_bridge`): a short-lived stdio ↔ socket proxy spawned
//! by each agent session (`so-context mcp`).  It connects to the running
//! daemon socket and forwards the MCP stdio transport to it, so the agent
//! thinks it is talking directly to an MCP server over stdio.
//!
//! # Auto-discovery hooks (inside BuiltinServer)
//!
//! 1. `initialize()` — fires on MCP handshake. Extracts `rootUri` /
//!    `workspaceFolders` from the client params and registers them.
//!
//! 2. `on_initialized()` — fires after the handshake ACK. If the client
//!    advertises `roots` capability, sends `roots/list` to retrieve all
//!    workspace roots and registers each one.
//!
//! 3. `call_tool()` pre-hook — last resort: if still no project is registered,
//!    falls back to `std::env::current_dir()` before dispatching the tool.

pub mod tools;

use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    RoleClient, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext},
    model::{
        CallToolRequestParams, CallToolResult, ClientCapabilities, ErrorData,
        InitializeRequestParams, InitializeResult, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo,
    },
    service::{MaybeSendFuture, NotificationContext, RequestContext, RunningService},
    transport::{StreamableHttpClientTransport, stdio},
};

use uuid::Uuid;

use crate::daemon::WatchManager;
use crate::daemon::watch_manager::file_uri_to_path;
use crate::socket::{MCP_ENDPOINT, ctrl_socket_path, socket_path};

const ROOTS_LIST_TIMEOUT_MS: u64 = 5_000;

// ---------------------------------------------------------------------------
// Bridge: stdio → Unix socket (used by `so-context mcp`)
// ---------------------------------------------------------------------------

/// Sends a JSON-RPC notification to the daemon's ctrl socket.
///
/// Used by `so-context ensure-watch` and `so-context unwatch` CLI subcommands,
/// invoked by agent session hooks.
pub async fn send_ctrl_request(
    method: &str,
    path: &str,
    agent: Option<&str>,
    session_id: Option<&str>,
) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;

    let sock = ctrl_socket_path();
    if !sock.exists() {
        eprintln!("so-context: daemon not running, skipping {method} for {path}");
        return Ok(());
    }

    let abs_path = if std::path::Path::new(path).is_absolute() {
        path.to_string()
    } else {
        std::env::current_dir()
            .map(|d| d.join(path).to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string())
    };

    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": {
            "path":       abs_path,
            "agent":      agent,
            "session_id": session_id,
        }
    });

    let mut line = serde_json::to_string(&msg)?;
    line.push('\n');

    let mut stream = UnixStream::connect(&sock)
        .await
        .map_err(|e| anyhow::anyhow!("connect to ctrl socket: {e}"))?;
    stream
        .write_all(line.as_bytes())
        .await
        .map_err(|e| anyhow::anyhow!("write to ctrl socket: {e}"))?;

    Ok(())
}

/// Connects to the daemon's Unix socket and bridges it to stdio.
///
/// Starts an MCP client connection to the daemon (via Unix socket / streamable
/// HTTP), then exposes it over stdio so the calling agent session can talk to
/// the shared daemon as if it were a local MCP server.
///
/// If the daemon is not running this returns an error immediately.
pub async fn run_mcp_bridge() -> Result<()> {
    let sock = socket_path();
    if !sock.exists() {
        anyhow::bail!(
            "so-context daemon is not running (socket not found: {}).\n\
             Start it with: so-context daemon",
            sock.display()
        );
    }

    let transport =
        StreamableHttpClientTransport::from_unix_socket(sock.to_str().unwrap(), MCP_ENDPOINT);

    // Connect to the daemon as an MCP client.
    let client_to_daemon: RunningService<RoleClient, ()> = ()
        .serve(transport)
        .await
        .map_err(|e| anyhow::anyhow!("connect to daemon: {e}"))?;

    // Wrap the daemon client in a BridgeServer and serve it over stdio.
    let bridge = BridgeServer {
        daemon: Arc::new(client_to_daemon),
    };
    bridge
        .serve(stdio())
        .await
        .map_err(|e| anyhow::anyhow!("mcp bridge serve: {e}"))?
        .waiting()
        .await
        .map_err(|e| anyhow::anyhow!("mcp bridge wait: {e}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// BridgeServer — forwards stdio agent requests to the daemon client
// ---------------------------------------------------------------------------

/// MCP server handler that proxies all requests to a connected daemon client.
struct BridgeServer {
    daemon: Arc<RunningService<RoleClient, ()>>,
}

impl ServerHandler for BridgeServer {
    fn initialize(
        &self,
        _request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<InitializeResult, ErrorData>> + MaybeSendFuture + '_
    {
        async move {
            // Return the daemon's own server info so the agent sees accurate metadata.
            Ok(self.get_info())
        }
    }

    fn get_info(&self) -> ServerInfo {
        // Forward the daemon's own server info if available.
        if let Some(info) = self.daemon.peer().peer_info() {
            ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
                .with_server_info(info.server_info.clone())
        } else {
            ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
        }
    }

    fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_
    {
        async move {
            self.daemon
                .peer()
                .list_tools(request)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + MaybeSendFuture + '_
    {
        async move {
            self.daemon
                .peer()
                .call_tool(request)
                .await
                .map_err(|e| ErrorData::internal_error(e.to_string(), None))
        }
    }
}

// ---------------------------------------------------------------------------
// BuiltinServer — the actual MCP server logic (runs inside the daemon)
// ---------------------------------------------------------------------------

pub struct BuiltinServer {
    tool_router: ToolRouter<Self>,
    wm: Arc<WatchManager>,
    client_supports_roots: std::sync::atomic::AtomicBool,
    /// Agent name extracted from `client_info.name` during the MCP handshake.
    /// Defaults to `"unknown"` until handshake fires.
    agent: Arc<std::sync::Mutex<String>>,
    /// Per-connection session ID — seeded with a UUID v4 so every connection
    /// is unique even before the handshake. Overwritten with `client_info.version`
    /// (or left as UUID for anonymous MCP callers).
    session_id: Arc<std::sync::Mutex<String>>,
}

impl BuiltinServer {
    pub fn new(wm: Arc<WatchManager>) -> Self {
        let mut tool_router = ToolRouter::<Self>::new();
        tool_router.add_route(tools::read::route());
        tool_router.add_route(tools::search::route());
        tool_router.add_route(tools::status::route(Arc::clone(&wm)));

        Self {
            tool_router,
            wm,
            client_supports_roots: std::sync::atomic::AtomicBool::new(false),
            agent: Arc::new(std::sync::Mutex::new("unknown".to_string())),
            session_id: Arc::new(std::sync::Mutex::new(Uuid::new_v4().to_string())),
        }
    }

    fn agent(&self) -> String {
        self.agent.lock().unwrap().clone()
    }
    fn session_id(&self) -> String {
        self.session_id.lock().unwrap().clone()
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
        let supports_roots = matches!(
            &request.capabilities,
            ClientCapabilities { roots: Some(_), .. }
        );
        self.client_supports_roots
            .store(supports_roots, std::sync::atomic::Ordering::Relaxed);

        // Extract agent name and session ID from client_info.
        {
            let info = &request.client_info;
            *self.agent.lock().unwrap() = info.name.clone();
            *self.session_id.lock().unwrap() = info.version.clone();
        }

        if let Some(peer_info) = context.peer.peer_info() {
            if let Ok(v) = serde_json::to_value(&peer_info) {
                for key in ["rootUri", "root_uri"] {
                    if let Some(uri) = v.get(key).and_then(|v| v.as_str()) {
                        self.wm.ensure_watching(
                            &file_uri_to_path(uri),
                            Some(&self.agent()),
                            Some(&self.session_id()),
                        );
                    }
                }
                if let Some(folders) = v.get("workspaceFolders").and_then(|v| v.as_array()) {
                    for folder in folders {
                        if let Some(uri) = folder.get("uri").and_then(|v| v.as_str()) {
                            self.wm.ensure_watching(
                                &file_uri_to_path(uri),
                                Some(&self.agent()),
                                Some(&self.session_id()),
                            );
                        }
                    }
                }
            }
        }

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
                if let Ok(cwd) = std::env::current_dir() {
                    self.wm.ensure_watching(
                        cwd.to_string_lossy().as_ref(),
                        Some(&self.agent()),
                        Some(&self.session_id()),
                    );
                }
                return;
            }

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
                            self.wm.ensure_watching(
                                cwd.to_string_lossy().as_ref(),
                                Some(&self.agent()),
                                Some(&self.session_id()),
                            );
                        }
                    } else {
                        for root in &roots_result.roots {
                            self.wm.ensure_watching(
                                &file_uri_to_path(&root.uri),
                                Some(&self.agent()),
                                Some(&self.session_id()),
                            );
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("[mcp] roots/list failed: {e}; falling back to cwd");
                    if let Ok(cwd) = std::env::current_dir() {
                        self.wm.ensure_watching(
                            cwd.to_string_lossy().as_ref(),
                            Some(&self.agent()),
                            Some(&self.session_id()),
                        );
                    }
                }
                Err(_timeout) => {
                    eprintln!("[mcp] roots/list timed out; falling back to cwd");
                    if let Ok(cwd) = std::env::current_dir() {
                        self.wm.ensure_watching(
                            cwd.to_string_lossy().as_ref(),
                            Some(&self.agent()),
                            Some(&self.session_id()),
                        );
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
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + MaybeSendFuture + '_
    {
        async move {
            if let Ok(cwd) = std::env::current_dir() {
                self.wm.ensure_watching(
                    cwd.to_string_lossy().as_ref(),
                    Some(&self.agent()),
                    Some(&self.session_id()),
                );
            }

            self.tool_router
                .call(ToolCallContext::new(self, request, context))
                .await
        }
    }

    // -----------------------------------------------------------------------
    // Metadata
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
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_
    {
        async move {
            Ok(ListToolsResult {
                tools: self.tool_router.list_all(),
                ..Default::default()
            })
        }
    }
}
