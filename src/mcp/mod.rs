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

use crate::core_metrics::{MetricsSummary, MetricsWindow};
use crate::daemon::WatchManager;
use crate::daemon::watch_manager::file_uri_to_path;
use crate::file_visit_cache::FileVisitCache;
use crate::socket::{ctrl_socket_path, socket_path};

const ROOTS_LIST_TIMEOUT_MS: u64 = 5_000;
const CTRL_QUERY_TIMEOUT_MS: u64 = 5_000;

pub fn prefers_plain_text_tool_output(client: Option<&str>) -> bool {
    let Some(client) = client else {
        return false;
    };

    let normalized = client.trim().to_ascii_lowercase();
    normalized.contains("codex")
}

#[cfg(test)]
mod tests {
    use super::prefers_plain_text_tool_output;

    #[test]
    fn codex_clients_prefer_plain_text_tool_output() {
        assert!(prefers_plain_text_tool_output(Some("Codex")));
        assert!(prefers_plain_text_tool_output(Some("codex-desktop")));
        assert!(prefers_plain_text_tool_output(Some("OpenAI Codex CLI")));
    }

    #[test]
    fn non_codex_clients_keep_structured_content() {
        assert!(!prefers_plain_text_tool_output(None));
        assert!(!prefers_plain_text_tool_output(Some("claude-code")));
        assert!(!prefers_plain_text_tool_output(Some("cursor")));
    }
}

// ---------------------------------------------------------------------------
// Bridge: thin stdio ↔ Unix-socket pipe  (used by `so-context mcp`)
// ---------------------------------------------------------------------------

/// Sends a `compact_reset` notification to the daemon's ctrl socket.
///
/// Called by the PostCompact hook handler after context compaction so the
/// daemon drops all file-visit cache entries for that session.
pub async fn send_compact_reset(connection_id: &str, session_id: &str) -> Result<()> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "compact_reset",
        "params": {
            "connection_id": connection_id,
            "session_id":    session_id,
        }
    });
    send_ctrl_message(&msg, true).await
}

/// Sends a JSON-RPC notification to the daemon's ctrl socket.
///
/// Used by `so-context watch` and `so-context unwatch` CLI subcommands.
pub async fn send_ctrl_request(
    method: &str,
    path: &str,
    client: Option<&str>,
    session_id: Option<&str>,
) -> Result<()> {
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
    send_ctrl_message(&msg, false).await
}

pub async fn send_ctrl_status_request() -> Result<String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let sock = ctrl_socket_path();
    if !sock.exists() {
        return Err(anyhow::anyhow!(
            "so-context daemon is not running.\nStart it with: so-context daemon"
        ));
    }

    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "status",
        "method": "status",
        "params": {}
    });

    let mut line = serde_json::to_string(&msg)?;
    line.push('\n');

    let stream = UnixStream::connect(&sock)
        .await
        .map_err(|e| anyhow::anyhow!("connect to ctrl socket: {e}"))?;
    let (read_half, mut write_half) = stream.into_split();

    write_half
        .write_all(line.as_bytes())
        .await
        .map_err(|e| anyhow::anyhow!("write to ctrl socket: {e}"))?;

    let mut lines = BufReader::new(read_half).lines();
    let Some(response_line) = tokio::time::timeout(
        std::time::Duration::from_millis(CTRL_QUERY_TIMEOUT_MS),
        lines.next_line(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for status response from so-context daemon"))?
    .map_err(|e| anyhow::anyhow!("read ctrl response: {e}"))?
    else {
        return Err(anyhow::anyhow!(
            "so-context daemon did not respond to status request"
        ));
    };

    let response: serde_json::Value = serde_json::from_str(&response_line)?;
    Ok(response
        .pointer("/result/text")
        .and_then(|v| v.as_str())
        .unwrap_or("no projects being watched")
        .to_string())
}

pub async fn send_ctrl_metrics_request(
    window: MetricsWindow,
    project: Option<&str>,
    top: usize,
) -> Result<MetricsSummary> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let sock = ctrl_socket_path();
    if !sock.exists() {
        return Err(anyhow::anyhow!(
            "so-context daemon is not running.\nStart it with: so-context daemon"
        ));
    }

    let normalized_project = project.map(|path| {
        if std::path::Path::new(path).is_absolute() {
            path.to_string()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path).to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string())
        }
    });

    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "metrics",
        "method": "metrics",
        "params": {
            "window": window.label(),
            "project": normalized_project,
            "top": top,
        }
    });

    let mut line = serde_json::to_string(&msg)?;
    line.push('\n');

    let stream = UnixStream::connect(&sock).await.map_err(|e| {
        if matches!(
            e.kind(),
            std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::NotFound
                | std::io::ErrorKind::ConnectionReset
        ) {
            anyhow::anyhow!("so-context daemon is not running.\nStart it with: so-context daemon")
        } else {
            anyhow::anyhow!("connect to ctrl socket: {e}")
        }
    })?;
    let (read_half, mut write_half) = stream.into_split();

    write_half
        .write_all(line.as_bytes())
        .await
        .map_err(|e| anyhow::anyhow!("write to ctrl socket: {e}"))?;

    let mut lines = BufReader::new(read_half).lines();
    let Some(response_line) = tokio::time::timeout(
        std::time::Duration::from_millis(CTRL_QUERY_TIMEOUT_MS),
        lines.next_line(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for metrics response from so-context daemon"))?
    .map_err(|e| anyhow::anyhow!("read ctrl response: {e}"))?
    else {
        return Err(anyhow::anyhow!("empty ctrl response for metrics"));
    };

    let response: serde_json::Value = serde_json::from_str(&response_line)?;
    if let Some(message) = response.pointer("/error/message").and_then(|v| v.as_str()) {
        return Err(anyhow::anyhow!(message.to_string()));
    }

    let Some(summary) = response.get("result") else {
        return Err(anyhow::anyhow!("missing ctrl metrics result payload"));
    };

    Ok(serde_json::from_value(summary.clone())?)
}

async fn send_ctrl_message(msg: &serde_json::Value, silent_if_missing: bool) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;

    let sock = ctrl_socket_path();
    if !sock.exists() {
        if silent_if_missing {
            return Ok(());
        }
        let method = msg
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("request");
        let path = msg
            .pointer("/params/path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        eprintln!("so-context: daemon not running, skipping {method} for {path}");
        return Ok(());
    }

    let mut line = serde_json::to_string(msg)?;
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
                Ok(s) => {
                    connected = Some(s);
                    break;
                }
                Err(e) => {
                    last_err = e.to_string();
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
            }
        }
        connected.ok_or_else(|| {
            anyhow::anyhow!(
                "so-context daemon is not running (could not connect to {}: {}).\n\
             Start it with: so-context daemon",
                sock.display(),
                last_err
            )
        })?
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
    }

    // Tool dispatch — hook 3: cwd last-resort before every tool call

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + MaybeSendFuture + '_
    {
        async move {
            self.tool_router
                .call(ToolCallContext::new(self, request, context))
                .await
        }
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
