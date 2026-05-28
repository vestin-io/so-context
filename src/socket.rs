//! Shared Unix socket path for daemon ↔ MCP bridge communication.

use std::path::PathBuf;

/// Returns the path to the Unix domain socket the daemon listens on.
///
/// Uses `$XDG_RUNTIME_DIR/so-context.sock` when available, otherwise
/// falls back to `/tmp/so-context-<uid>.sock`.
pub fn socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("so-context.sock");
    }
    // Fallback: /tmp/so-context-<uid>.sock (uid avoids collisions between users)
    let uid = {
        #[cfg(unix)]
        {
            // SAFETY: getuid() is always safe.
            unsafe { libc::getuid() }
        }
        #[cfg(not(unix))]
        {
            0u32
        }
    };
    PathBuf::from(format!("/tmp/so-context-{uid}.sock"))
}

/// The HTTP endpoint the daemon serves MCP over (relative to socket root).
pub const MCP_ENDPOINT: &str = "http://localhost/mcp";
