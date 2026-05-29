//! Daemon — long-running background process.
//!
//! Binds two Unix domain sockets:
//!   - MCP socket  (`so-context.sock`)       — streamable HTTP, for agent sessions
//!   - ctrl socket (`so-context-ctrl.sock`)  — newline-delimited JSON-RPC, for CLI hooks

pub mod watch_manager;

use std::{fs, sync::Arc};

use anyhow::{Context, Result};
use axum::Router;
use hyper_util::rt::TokioIo;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio_util::sync::CancellationToken;
use tower_service::Service as _;

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

        // --- MCP socket ---
        let mcp_path = socket_path();
        if mcp_path.exists() {
            fs::remove_file(&mcp_path)
                .with_context(|| format!("remove stale socket {}", mcp_path.display()))?;
        }
        let mcp_listener = UnixListener::bind(&mcp_path)
            .with_context(|| format!("bind MCP socket {}", mcp_path.display()))?;
        eprintln!("so-context daemon: MCP  on {}", mcp_path.display());

        // --- ctrl socket ---
        let ctrl_path = ctrl_socket_path();
        if ctrl_path.exists() {
            fs::remove_file(&ctrl_path)
                .with_context(|| format!("remove stale ctrl socket {}", ctrl_path.display()))?;
        }
        let ctrl_listener = UnixListener::bind(&ctrl_path)
            .with_context(|| format!("bind ctrl socket {}", ctrl_path.display()))?;
        eprintln!("so-context daemon: ctrl on {}", ctrl_path.display());

        // Spawn ctrl listener on its own task.
        let wm_ctrl = Arc::clone(&wm);
        tokio::spawn(async move {
            run_ctrl_listener(ctrl_listener, wm_ctrl).await;
        });

        // --- MCP HTTP server ---
        let ct = CancellationToken::new();
        let wm_mcp = Arc::clone(&wm);
        let mcp_service: StreamableHttpService<BuiltinServer, LocalSessionManager> =
            StreamableHttpService::new(
                move || Ok(BuiltinServer::new(Arc::clone(&wm_mcp))),
                Default::default(),
                StreamableHttpServerConfig::default().with_cancellation_token(ct.child_token()),
            );

        let router = Router::new().nest_service("/mcp", mcp_service);

        loop {
            let (stream, _addr) = mcp_listener.accept().await?;
            let router = router.clone();
            tokio::spawn(async move {
                let io = TokioIo::new(stream);
                let hyper_svc = hyper::service::service_fn(
                    move |req: hyper::Request<hyper::body::Incoming>| {
                        let mut svc = router.clone();
                        async move { svc.call(req).await }
                    },
                );
                if let Err(e) = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, hyper_svc)
                    .await
                {
                    eprintln!("so-context daemon: MCP connection error: {e}");
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// ctrl socket — newline-delimited JSON-RPC 2.0 (notifications only)
// ---------------------------------------------------------------------------

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
        if line.is_empty() {
            continue;
        }
        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
            dispatch_ctrl(&msg, &wm);
        } else {
            eprintln!("so-context daemon: ctrl invalid JSON: {line}");
        }
    }
}

/// Dispatches a JSON-RPC notification to the appropriate WatchManager operation.
///
/// Expected shape:
/// ```json
/// {"jsonrpc":"2.0","method":"watch","params":{"path":"...","agent":"...","session_id":"..."}}
/// {"jsonrpc":"2.0","method":"unwatch","params":{"path":"...","agent":"...","session_id":"..."}}
/// ```
fn dispatch_ctrl(msg: &serde_json::Value, wm: &WatchManager) {
    let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = msg.get("params");
    let path = params
        .and_then(|p| p.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if path.is_empty() {
        eprintln!("so-context daemon: ctrl missing path in {method} message");
        return;
    }
    let agent = params.and_then(|p| p.get("agent")).and_then(|v| v.as_str());
    let session_id = params
        .and_then(|p| p.get("session_id"))
        .and_then(|v| v.as_str());

    match method {
        "watch" => {
            wm.ensure_watching(path, agent, session_id);
        }
        "unwatch" => {
            wm.unwatch(path, agent, session_id);
        }
        other => {
            eprintln!("so-context daemon: ctrl unknown method: {other}");
        }
    }
}
