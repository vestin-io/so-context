//! Daemon — long-running background process.
//!
//! Binds a Unix domain socket, serves the MCP streamable-HTTP server over it,
//! and owns all background services (WatchManager, graph DB).
//!
//! Multiple agent sessions connect via the `so-context mcp` bridge, which
//! forwards stdio ↔ the daemon socket. Because every agent shares the same
//! daemon process there is exactly one WatchManager and one SQLite database
//! per machine user.

pub mod watch_manager;

use std::{fs, sync::Arc};

use anyhow::{Context, Result};
use axum::Router;
use hyper_util::rt::TokioIo;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService,
    session::local::LocalSessionManager,
};
use tokio::net::UnixListener;
use tokio_util::sync::CancellationToken;
use tower_service::Service as _;

pub use watch_manager::WatchManager;

use crate::mcp::BuiltinServer;
use crate::socket::socket_path;

/// The daemon runtime. Construct with [`Daemon::new`] and start with [`Daemon::run`].
pub struct Daemon {
    pub watch_manager: Arc<WatchManager>,
}

impl Daemon {
    pub fn new() -> Self {
        Self {
            watch_manager: Arc::new(WatchManager::new()),
        }
    }

    /// Binds the Unix socket and serves the MCP HTTP server until the process exits.
    pub async fn run(self) -> Result<()> {
        let path = socket_path();

        // Remove stale socket file from a previous run.
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("remove stale socket {}", path.display()))?;
        }

        let listener = UnixListener::bind(&path)
            .with_context(|| format!("bind Unix socket {}", path.display()))?;

        eprintln!("so-context daemon listening on {}", path.display());

        let ct = CancellationToken::new();
        let watch_manager = Arc::clone(&self.watch_manager);

        let service: StreamableHttpService<BuiltinServer, LocalSessionManager> =
            StreamableHttpService::new(
                move || Ok(BuiltinServer::new(Arc::clone(&watch_manager))),
                Default::default(),
                StreamableHttpServerConfig::default()
                    .with_cancellation_token(ct.child_token()),
            );

        let router = Router::new().nest_service("/mcp", service);

        // Serve over the Unix socket using hyper directly.
        loop {
            let (stream, _addr) = listener.accept().await?;
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
                    eprintln!("so-context daemon: connection error: {e}");
                }
            });
        }
    }
}
