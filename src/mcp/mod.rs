//! MCP module.
//!
//! # Two roles
//!
//! **Daemon side** (`BuiltinServer`): the real MCP server, served directly over
//! the raw JSON-RPC Unix socket.  One `BuiltinServer` instance per connected
//! agent session.
//!
//! **Bridge** (`run_mcp_bridge`): a thin stdio ↔ Unix-socket pipe spawned by
//! each agent session (`so-context mcp`).  It has zero MCP logic — it just
//! forwards bytes in both directions, and injects three metadata fields
//! (`_so_client`, `_so_client_version`, `_so_session_id`) into any
//! `tools/call` request before forwarding, so the daemon can attribute events.
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
use rmcp::{    RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext},
    model::{
        CallToolRequestParams, CallToolResult, ClientCapabilities, ErrorData,
        InitializeRequestParams, InitializeResult, ListToolsResult, PaginatedRequestParams,
        ServerCapabilities, ServerInfo,
    },
    service::{MaybeSendFuture, NotificationContext, RequestContext},
};

use uuid::Uuid;

use crate::daemon::WatchManager;
use crate::daemon::watch_manager::file_uri_to_path;
use crate::socket::{ctrl_socket_path, socket_path};

const ROOTS_LIST_TIMEOUT_MS: u64 = 5_000;

// ---------------------------------------------------------------------------
// Bridge: thin stdio ↔ Unix-socket pipe  (used by `so-context mcp`)
// ---------------------------------------------------------------------------

/// Sends a JSON-RPC notification to the daemon's ctrl socket.
///
/// Used by `so-context ensure-watch` and `so-context unwatch` CLI subcommands.
pub async fn send_ctrl_request(
    method: &str,
    path: &str,
    client: Option<&str>,
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
            "client":     client,
            "session_id": session_id,
        }
    });

    let mut line = serde_json::to_string(&msg)?;
    line.push('\n');

    let mut stream = UnixStream::connect(&sock)
        .await
        .map_err(|e| anyhow::anyhow!("connect to ctrl socket: {e}"))?;
    stream.write_all(line.as_bytes()).await
        .map_err(|e| anyhow::anyhow!("write to ctrl socket: {e}"))?;

    Ok(())
}

/// Pure stdio ↔ Unix-socket byte pipe.
///
/// Every byte from stdin goes straight to the daemon socket, and every byte
/// from the daemon goes straight to stdout. No protocol parsing — the daemon
/// is the real MCP server and handles everything including identity attribution
/// via the `initialize` handshake (`client_info.name` / `client_info.version`).
pub async fn run_mcp_bridge() -> Result<()> {
    use tokio::net::UnixStream;

    let sock = socket_path();

    // Retry connecting for up to 5s to handle the race where the socket file
    // exists but the daemon is still starting (or a stale socket remains).
    let stream = {
        let mut last_err = String::new();
        let mut connected = None;
        for _ in 0..20 {
            match UnixStream::connect(&sock).await {
                Ok(s) => { connected = Some(s); break; }
                Err(e) => {
                    last_err = e.to_string();
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
            }
        }
        connected.ok_or_else(|| anyhow::anyhow!(
            "so-context daemon is not running (could not connect to {}: {}).\n\
             Start it with: so-context daemon",
            sock.display(), last_err
        ))?
    };

    let (mut sock_read, mut sock_write) = tokio::io::split(stream);

    // Pipe stdin → socket in one task, socket → stdout in another.
    // Both run concurrently; we wait for either to finish, then exit.
    let stdin_to_sock = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let _ = tokio::io::copy(&mut stdin, &mut sock_write).await;
    });

    let sock_to_stdout = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        let _ = tokio::io::copy(&mut sock_read, &mut stdout).await;
    });

    // Exit as soon as either direction closes.
    tokio::select! {
        _ = stdin_to_sock  => {}
        _ = sock_to_stdout => {}
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// BuiltinServer — real MCP server logic (runs inside the daemon)
// ---------------------------------------------------------------------------

pub struct BuiltinServer {
    tool_router: ToolRouter<Self>,
    wm: Arc<WatchManager>,
    client_supports_roots: std::sync::atomic::AtomicBool,
    /// MCP client app name from `client_info.name` (e.g. `"opencode"`).
    client: Arc<std::sync::Mutex<Option<String>>>,
    /// MCP client app version from `client_info.version` (e.g. `"1.15.12"`).
    client_version: Arc<std::sync::Mutex<Option<String>>>,
    /// Per-connection session ID — UUID v4 generated at connection time.
    /// The MCP protocol has no session ID; this uniquely identifies the connection.
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
            client: Arc::new(std::sync::Mutex::new(None)),
            client_version: Arc::new(std::sync::Mutex::new(None)),
            session_id: Arc::new(std::sync::Mutex::new(Uuid::new_v4().to_string())),
        }
    }

    pub fn client(&self) -> Option<String> { self.client.lock().unwrap().clone() }
    pub fn client_version(&self) -> Option<String> { self.client_version.lock().unwrap().clone() }
    pub fn session_id(&self) -> String { self.session_id.lock().unwrap().clone() }
}

impl ServerHandler for BuiltinServer {
    // Handshake — hook 1: extract roots from initialize params

    fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<InitializeResult, ErrorData>>
           + MaybeSendFuture
           + '_ {
        let supports_roots = matches!(
            &request.capabilities,
            ClientCapabilities { roots: Some(_), .. }
        );
        self.client_supports_roots
            .store(supports_roots, std::sync::atomic::Ordering::Relaxed);

        // Extract client name and version from client_info.
        {
            let info = &request.client_info;
            *self.client.lock().unwrap() = Some(info.name.clone());
            *self.client_version.lock().unwrap() = Some(info.version.clone());
            // session_id stays as UUID v4 — no session ID in MCP protocol
        }

        // Extract workspace roots from initialize params if present.
        if let Ok(v) = serde_json::to_value(&request) {
            let params = v.get("params").unwrap_or(&v);
            for key in ["rootUri", "root_uri"] {
                if let Some(uri) = params.get(key).and_then(|v| v.as_str()) {
                    self.wm.ensure_watching(
                        &file_uri_to_path(uri),
                        self.client().as_deref(),
                        Some(&self.session_id()),
                    );
                }
            }
            if let Some(folders) = params.get("workspaceFolders").and_then(|v| v.as_array()) {
                for folder in folders {
                    if let Some(uri) = folder.get("uri").and_then(|v| v.as_str()) {
                        self.wm.ensure_watching(
                            &file_uri_to_path(uri),
                            self.client().as_deref(),
                            Some(&self.session_id()),
                        );
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

    // Post-handshake — hook 2: roots/list from client

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
                        self.client().as_deref(),
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
                                self.client().as_deref(),
                                Some(&self.session_id()),
                            );
                        }
                    } else {
                        for root in &roots_result.roots {
                            self.wm.ensure_watching(
                                &file_uri_to_path(&root.uri),
                                self.client().as_deref(),
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
                            self.client().as_deref(),
                            Some(&self.session_id()),
                        );
                    }
                }
                Err(_timeout) => {
                    eprintln!("[mcp] roots/list timed out; falling back to cwd");
                    if let Ok(cwd) = std::env::current_dir() {
                        self.wm.ensure_watching(
                            cwd.to_string_lossy().as_ref(),
                            self.client().as_deref(),
                            Some(&self.session_id()),
                        );
                    }
                }
            }
        }
    }

    // Tool dispatch — hook 3: cwd last-resort before every tool call

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>>
           + MaybeSendFuture
           + '_ {
        async move {
            if let Ok(cwd) = std::env::current_dir() {
                self.wm.ensure_watching(
                    cwd.to_string_lossy().as_ref(),
                    self.client().as_deref(),
                    Some(&self.session_id()),
                );
            }

            self.tool_router
                .call(ToolCallContext::new(self, request, context))
                .await
        }
    }

    // Metadata

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
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
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>>
           + MaybeSendFuture
           + '_ {
        async move {
            Ok(ListToolsResult {
                tools: self.tool_router.list_all(),
                ..Default::default()
            })
        }
    }
}
