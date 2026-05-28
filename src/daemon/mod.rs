//! Daemon — long-running background process.
//!
//! Binds three Unix domain sockets:
//!   - MCP socket  (`so-context.sock`)       — raw newline-delimited JSON-RPC, for agent sessions
//!   - ctrl socket (`so-context-ctrl.sock`)  — raw newline-delimited JSON-RPC, for CLI hooks
//!   - HTTP socket (`so-context-http.sock`)  — Axum HTTP server, for dashboard UI (future)

pub mod watch_manager;

use std::{fs, sync::Arc};

use anyhow::{Context, Result};
use rmcp::ServiceExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

pub use watch_manager::WatchManager;

use crate::mcp::BuiltinServer;
use crate::socket::{ctrl_socket_path, socket_path};

/// The daemon runtime.
pub struct Daemon {
    pub watch_manager: Arc<WatchManager>,
}

impl Daemon {
    pub fn new() -> Self {
        Self {
            watch_manager: Arc::new(WatchManager::new()),
        }
    }

    pub async fn run(self) -> Result<()> {
        let wm = Arc::clone(&self.watch_manager);

        // --- MCP socket (raw JSON-RPC) ---
        let mcp_path = socket_path();
        if let Some(parent) = mcp_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create MCP socket dir {}", parent.display()))?;
        }
        if mcp_path.exists() {
            fs::remove_file(&mcp_path)
                .with_context(|| format!("remove stale socket {}", mcp_path.display()))?;
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
        if ctrl_path.exists() {
            fs::remove_file(&ctrl_path)
                .with_context(|| format!("remove stale ctrl socket {}", ctrl_path.display()))?;
        }
        let ctrl_listener = UnixListener::bind(&ctrl_path)
            .with_context(|| format!("bind ctrl socket {}", ctrl_path.display()))?;
        eprintln!("so-context daemon: ctrl on {}", ctrl_path.display());

        // Spawn ctrl listener.
        let wm_ctrl = Arc::clone(&wm);
        tokio::spawn(async move {
            run_ctrl_listener(ctrl_listener, wm_ctrl).await;
        });

        // --- MCP accept loop ---
        loop {
            let (stream, _addr) = mcp_listener.accept().await?;
            let wm_mcp = Arc::clone(&wm);
            tokio::spawn(async move {
                let server = BuiltinServer::new(Arc::clone(&wm_mcp));
                let transport = stream;
                match server.serve(transport).await {
                    Ok(svc) => {
                        if let Err(e) = svc.waiting().await {
                            eprintln!("so-context daemon: MCP session error: {e}");
                        }
                    }
                    Err(e) => {
                        eprintln!("so-context daemon: MCP serve error: {e}");
                    }
                }
            });
        }
    }
}

// ctrl socket — newline-delimited JSON-RPC 2.0 (notifications only)

async fn run_ctrl_listener(listener: UnixListener, wm: Arc<WatchManager>) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let wm = Arc::clone(&wm);
                tokio::spawn(async move { handle_ctrl_connection(stream, wm).await });
            }
            Err(e) => {
                eprintln!("so-context daemon: ctrl accept error: {e}");
            }
        }
    }
}

async fn handle_ctrl_connection(stream: UnixStream, wm: Arc<WatchManager>) {
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() { continue; }
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
            dispatch_ctrl(&msg, &wm);
        } else {
            eprintln!("so-context daemon: ctrl invalid JSON: {line}");
        }
    }
}

fn dispatch_ctrl(msg: &serde_json::Value, wm: &WatchManager) {
    let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = msg.get("params");
    let path = params.and_then(|p| p.get("path")).and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        eprintln!("so-context daemon: ctrl missing path in {method} message");
        return;
    }
    let client     = params.and_then(|p| p.get("client")).and_then(|v| v.as_str());
    let session_id = params.and_then(|p| p.get("session_id")).and_then(|v| v.as_str());

    match method {
        "watch"   => { wm.ensure_watching(path, client, session_id); }
        "unwatch" => { wm.unwatch(path, client, session_id); }
        other     => { eprintln!("so-context daemon: ctrl unknown method: {other}"); }
    }
}
