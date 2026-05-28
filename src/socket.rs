//! Shared Unix socket paths for daemon communication.
//!
//! Two sockets:
//!   - MCP socket   — streamable HTTP, for agent MCP sessions only
//!   - ctrl socket  — newline-delimited JSON-RPC, for CLI hook commands (watch/unwatch)

use std::path::PathBuf;

fn runtime_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir);
    }
    let uid = {
        #[cfg(unix)]
        unsafe { libc::getuid() }
        #[cfg(not(unix))]
        0u32
    };
    PathBuf::from(format!("/tmp/so-context-{uid}"))
}

/// Unix socket the daemon serves MCP (streamable HTTP) over — agents only.
pub fn socket_path() -> PathBuf {
    runtime_dir().join("so-context.sock")
}

/// Unix socket the daemon listens on for CLI control messages (watch/unwatch).
pub fn ctrl_socket_path() -> PathBuf {
    runtime_dir().join("so-context-ctrl.sock")
}
