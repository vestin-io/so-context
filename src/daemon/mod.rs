//! Daemon — top-level runtime that owns all services.
//!
//! Current services:
//! - [`WatchManager`] — monitors projects, resolves roots, deduplicates, owns
//!                       GraphDb connections
//! - MCP server       — exposes tools to agent hosts (stdio transport)
//!
//! Planned:
//! - Event collector
//! - UI dashboard (HTTP / WebSocket)

pub mod watch_manager;

use std::sync::Arc;

use anyhow::Result;

pub use watch_manager::WatchManager;

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

    /// Starts all daemon services and blocks until the process exits.
    pub async fn run(self) -> Result<()> {
        crate::mcp::run_stdio_server(Arc::clone(&self.watch_manager)).await
    }
}
