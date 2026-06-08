//! Daemon — long-running background process.
//!
//! Binds three Unix domain sockets:
//!   - MCP socket  (`so-context.sock`)       — raw newline-delimited JSON-RPC, for agent sessions
//!   - ctrl socket (`so-context-ctrl.sock`)  — raw newline-delimited JSON-RPC, for CLI hooks
//!   - HTTP socket (`so-context-http.sock`)  — Axum HTTP server, for dashboard UI (future)

mod control;
pub mod watch_manager;

use std::{fs, sync::Arc};

use anyhow::{Context, Result};
use rmcp::ServiceExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
pub use watch_manager::WatchManager;

use crate::core_metrics::{MetricsRequest, MetricsWindow, build_metrics_summary};
use crate::file_visit_cache::FileVisitCache;
use crate::mcp::BuiltinServer;
use crate::mcp::tools::status::render_status_text;
use crate::socket::{ctrl_socket_path, socket_path};
pub use control::{restart_background, start_background, stop_background};

/// The daemon runtime.
pub struct Daemon {
    pub watch_manager: Arc<WatchManager>,
    pub file_visit_cache: FileVisitCache,
}

impl Daemon {
    pub fn new() -> Self {
        Self {
            watch_manager: Arc::new(WatchManager::new()),
            file_visit_cache: FileVisitCache::new(),
        }
    }

    pub async fn run(self) -> Result<()> {
        let wm = Arc::clone(&self.watch_manager);
        let fvc = self.file_visit_cache;
        control::ensure_no_active_daemon().await?;
        control::cleanup_stale_runtime_files()?;
        let _pid_guard = control::write_pid_file_for_current_process()?;

        // --- MCP socket (raw JSON-RPC) ---
        let mcp_path = socket_path();
        if let Some(parent) = mcp_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create MCP socket dir {}", parent.display()))?;
        }
        let mcp_listener = UnixListener::bind(&mcp_path)
            .with_context(|| format!("bind MCP socket {}", mcp_path.display()))?;
        eprintln!("so-context daemon: MCP  on {}", mcp_path.display());

        // --- ctrl socket (raw JSON-RPC notifications) ---
        let ctrl_path = ctrl_socket_path();
        if let Some(parent) = ctrl_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create ctrl socket dir {}", parent.display()))?;
        }
        let ctrl_listener = UnixListener::bind(&ctrl_path)
            .with_context(|| format!("bind ctrl socket {}", ctrl_path.display()))?;
        eprintln!("so-context daemon: ctrl on {}", ctrl_path.display());

        // Spawn ctrl listener.
        let wm_ctrl = Arc::clone(&wm);
        let fvc_ctrl = fvc.clone();
        tokio::spawn(async move {
            run_ctrl_listener(ctrl_listener, wm_ctrl, fvc_ctrl).await;
        });

        // --- MCP accept loop ---
        loop {
            let (stream, _addr) = mcp_listener.accept().await?;
            let wm_mcp = Arc::clone(&wm);
            let fvc_mcp = fvc.clone();
            tokio::spawn(async move {
                let server = BuiltinServer::new(Arc::clone(&wm_mcp), fvc_mcp);
                // Capture connection_id now (set at construction, never changes).
                // client is captured after serve() so we get the post-handshake value.
                let connection_id = server.connection_id();
                match server.serve(stream).await {
                    Ok(svc) => {
                        let client = svc.peer_info().map(|i| i.client_info.name.clone());
                        if let Err(e) = svc.waiting().await {
                            eprintln!("so-context daemon: MCP session error: {e}");
                        }
                        wm_mcp.unwatch_by_session(client.as_deref(), &connection_id);
                    }
                    Err(e) => {
                        eprintln!("so-context daemon: MCP serve error: {e}");
                        // serve failed before handshake — unwatch with unknown client
                        wm_mcp.unwatch_by_session(None, &connection_id);
                    }
                }
            });
        }
    }
}

// ctrl socket — newline-delimited JSON-RPC 2.0 (notifications only)

async fn run_ctrl_listener(listener: UnixListener, wm: Arc<WatchManager>, fvc: FileVisitCache) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let wm = Arc::clone(&wm);
                let fvc = fvc.clone();
                tokio::spawn(async move { handle_ctrl_connection(stream, wm, fvc).await });
            }
            Err(e) => {
                eprintln!("so-context daemon: ctrl accept error: {e}");
            }
        }
    }
}

async fn handle_ctrl_connection(stream: UnixStream, wm: Arc<WatchManager>, fvc: FileVisitCache) {
    let (read_half, mut write_half) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
            if let Some(response) = dispatch_ctrl(&msg, &wm, &fvc) {
                match serde_json::to_string(&response) {
                    Ok(mut payload) => {
                        payload.push('\n');
                        if let Err(e) = write_half.write_all(payload.as_bytes()).await {
                            eprintln!("so-context daemon: ctrl write error: {e}");
                            break;
                        }
                    }
                    Err(e) => eprintln!("so-context daemon: ctrl encode error: {e}"),
                }
            }
        } else {
            eprintln!("so-context daemon: ctrl invalid JSON: {line}");
        }
    }
}

fn dispatch_ctrl(
    msg: &serde_json::Value,
    wm: &WatchManager,
    fvc: &FileVisitCache,
) -> Option<serde_json::Value> {
    let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let id = msg.get("id").cloned();
    let params = msg.get("params");
    let path = params
        .and_then(|p| p.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let client = params
        .and_then(|p| p.get("client"))
        .and_then(|v| v.as_str());
    let session_id = params
        .and_then(|p| p.get("session_id"))
        .and_then(|v| v.as_str());

    match method {
        "watch" | "unwatch" => {
            if path.is_empty() {
                eprintln!("so-context daemon: ctrl missing path in {method} message");
                return None;
            }
            if method == "watch" {
                wm.ensure_watching(path, client, session_id);
            } else {
                wm.unwatch(path, client, session_id);
            }
            None
        }
        "compact_reset" => {
            // Reset all file-visit cache entries for this session so the agent
            // receives full content again after context compaction.
            let connection_id = params
                .and_then(|p| p.get("connection_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let Some(session_id) = session_id.filter(|value| !value.is_empty()) else {
                eprintln!("so-context daemon: ctrl compact_reset missing session_id");
                return None;
            };
            let deleted = (!connection_id.is_empty()
                && fvc.delete_context_window(connection_id, session_id))
                || fvc.delete_context_window_globally(session_id);
            eprintln!(
                "so-context daemon: compact_reset connection={connection_id} session={} deleted={}",
                session_id, deleted
            );
            None
        }
        "status" => {
            let text = render_status_text(wm);
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id.unwrap_or(serde_json::Value::Null),
                "result": { "text": text }
            }))
        }
        "metrics" => {
            let window = match params
                .and_then(|p| p.get("window"))
                .and_then(|v| v.as_str())
                .unwrap_or("24h")
            {
                "24h" => MetricsWindow::Last24Hours,
                "7d" => MetricsWindow::Last7Days,
                "all" => MetricsWindow::All,
                other => {
                    return Some(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id.unwrap_or(serde_json::Value::Null),
                        "error": { "message": format!("unsupported metrics window: {other}") }
                    }));
                }
            };
            let project = params
                .and_then(|p| p.get("project"))
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string());
            let top = params
                .and_then(|p| p.get("top"))
                .and_then(|v| v.as_u64())
                .unwrap_or(5) as usize;
            let request = MetricsRequest {
                window,
                project,
                top_n: top,
            };
            match build_metrics_summary(&request) {
                Ok(summary) => Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id.unwrap_or(serde_json::Value::Null),
                    "result": summary
                })),
                Err(error) => Some(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id.unwrap_or(serde_json::Value::Null),
                    "error": { "message": error.to_string() }
                })),
            }
        }
        other => {
            eprintln!("so-context daemon: ctrl unknown method: {other}");
            None
        }
    }
}
