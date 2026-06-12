//! MCP module.
//!
//! # Two roles
//!
//! **Daemon side** (`BuiltinServer`): the real MCP server, served directly over
//! the raw JSON-RPC Unix socket.  One `BuiltinServer` instance per connected
//! agent session.
//!
//! **Bridge** (`run_mcp_bridge`): a thin stdio ↔ Unix-socket pipe spawned by
//! each agent session (`so-context mcp`). It has zero MCP logic and forwards
//! bytes unchanged in both directions. Session attribution comes from the
//! MCP handshake (`client_info`) plus the agent-side PreToolUse hook that
//! injects `_so_session_id` into so-context tool calls before they reach
//! the bridge.
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
mod transport;

use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    RoleServer, ServerHandler,
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
use crate::file_visit_cache::FileVisitCache;

pub use self::transport::{
    run_mcp_bridge, send_compact_reset, send_ctrl_metrics_request, send_ctrl_request,
    send_ctrl_status_request,
};

const ROOTS_LIST_TIMEOUT_MS: u64 = 5_000;

pub fn prefers_plain_text_tool_output(client: Option<&str>) -> bool {
    let Some(client) = client else {
        return false;
    };

    let normalized = client.trim().to_ascii_lowercase();
    normalized.contains("codex")
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

// ---------------------------------------------------------------------------
// BuiltinServer — real MCP server logic (runs inside the daemon)
// ---------------------------------------------------------------------------

pub struct BuiltinServer {
    tool_router: ToolRouter<Self>,
    wm: Arc<WatchManager>,
    /// Shared file-visit cache (one instance for the entire daemon lifetime,
    /// shared across all MCP connections via cheap `Clone`).
    pub file_visit_cache: FileVisitCache,
    client_supports_roots: std::sync::atomic::AtomicBool,
    /// MCP client app name from `client_info.name` (e.g. `"opencode"`).
    client: Arc<std::sync::Mutex<Option<String>>>,
    /// MCP client app version from `client_info.version` (e.g. `"1.15.12"`).
    client_version: Arc<std::sync::Mutex<Option<String>>>,
    /// Per-connection UUID — generated at connection time to uniquely identify
    /// the Unix socket connection. Not the same as an agent session ID.
    connection_id: Arc<std::sync::Mutex<String>>,
}

impl BuiltinServer {
    pub fn new(wm: Arc<WatchManager>, file_visit_cache: FileVisitCache) -> Self {
        let mut tool_router = ToolRouter::<Self>::new();
        tool_router.add_route(tools::read::route(Arc::clone(&wm)));
        tool_router.add_route(tools::references::route(Arc::clone(&wm)));
        tool_router.add_route(tools::search::route(Arc::clone(&wm)));
        tool_router.add_route(tools::shell::route(Arc::clone(&wm)));
        tool_router.add_route(tools::shell_output::route());
        tool_router.add_route(tools::status::route(Arc::clone(&wm)));

        Self {
            tool_router,
            wm,
            file_visit_cache,
            client_supports_roots: std::sync::atomic::AtomicBool::new(false),
            client: Arc::new(std::sync::Mutex::new(None)),
            client_version: Arc::new(std::sync::Mutex::new(None)),
            connection_id: Arc::new(std::sync::Mutex::new(Uuid::new_v4().to_string())),
        }
    }

    pub fn client(&self) -> Option<String> {
        self.client.lock().unwrap().clone()
    }
    pub fn client_version(&self) -> Option<String> {
        self.client_version.lock().unwrap().clone()
    }
    pub fn connection_id(&self) -> String {
        self.connection_id.lock().unwrap().clone()
    }
}

impl ServerHandler for BuiltinServer {
    // Handshake — hook 1: extract roots from initialize params

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
                        Some(&self.connection_id()),
                    );
                }
            }
            if let Some(folders) = params.get("workspaceFolders").and_then(|v| v.as_array()) {
                for folder in folders {
                    if let Some(uri) = folder.get("uri").and_then(|v| v.as_str()) {
                        self.wm.ensure_watching(
                            &file_uri_to_path(uri),
                            self.client().as_deref(),
                            Some(&self.connection_id()),
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

    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        if !self
            .client_supports_roots
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            if let Ok(cwd) = std::env::current_dir() {
                self.wm.ensure_watching(
                    cwd.to_string_lossy().as_ref(),
                    self.client().as_deref(),
                    Some(&self.connection_id()),
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
                            Some(&self.connection_id()),
                        );
                    }
                } else {
                    for root in &roots_result.roots {
                        self.wm.ensure_watching(
                            &file_uri_to_path(&root.uri),
                            self.client().as_deref(),
                            Some(&self.connection_id()),
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
                        Some(&self.connection_id()),
                    );
                }
            }
            Err(_timeout) => {
                eprintln!("[mcp] roots/list timed out; falling back to cwd");
                if let Ok(cwd) = std::env::current_dir() {
                    self.wm.ensure_watching(
                        cwd.to_string_lossy().as_ref(),
                        self.client().as_deref(),
                        Some(&self.connection_id()),
                    );
                }
            }
        }
    }

    // Tool dispatch — hook 3: cwd last-resort before every tool call

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_router
            .call(ToolCallContext::new(self, request, context))
            .await
    }

    // Metadata

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "so-context MCP server.\n\
\n\
Projects are auto-discovered from workspace roots on connect — no setup needed.\n\
\n\
Prefer so_read over native read tools for source and config files.\n\
Use so_read for exact file contents in shared project context instead of native Read/View.\n\
Do not reread the same file in a native read tool just to confirm text already returned by so_read.\n\
\n\
Prefer so_search over native grep-style tools when indexed project search is enough.\n\
Use so_search for attributable project hits instead of native Grep/rg when raw grep semantics are not required.\n\
Keep native grep-style tools only for raw grep semantics, shell-native pipelines, or unindexed project fallback.\n\
\n\
Prefer so_shell over native shell tools for short, one-shot commands.\n\
Use compressed so_shell output as the default and preferred final answer.\n\
Only call so_shell_output when the user explicitly asks for verbatim raw output or the compressed summary is missing required detail.\n\
Do not call so_shell_output just to confirm, double-check, or restate a compressed result that already answers the request.\n\
The text content returned by so_shell or so_shell_output is the actual command output. Read and use that text directly; do not rerun the same command in native shell just to confirm stdout unless the result is empty or the user explicitly asks for a rerun.\n\
Do not set full=true on the first so_shell call. Sequence is strict: compressed so_shell first, then so_shell_output, and only if tee is unavailable may you rerun so_shell with full=true and full_reason=tee_missing_or_expired.\n\
Keep native shell only for long-running, streaming, or interactive commands.\n\
\n\
Tools:\n\
  so_read         — read a file (mode: full / outline)\n\
  so_search       — FTS search over an indexed project graph\n\
  so_shell        — run a local shell command and return compressed output\n\
  so_shell_output — fetch cached raw output from a prior so_shell run_id\n\
  so_references   — find all usages of a symbol (callers/callees/imports/all)\n\
  so_status       — list all watched projects and their current state",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult {
            tools: self.tool_router.list_all(),
            ..Default::default()
        })
    }
}
