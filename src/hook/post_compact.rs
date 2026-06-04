use anyhow::Result;
use serde_json::Value;

/// PostCompact hook handler.
///
/// Reads the Claude Code PostCompact JSON payload from stdin and sends a
/// `compact_reset` ctrl notification to the daemon so it clears file-visit
/// cache entries for the compacted session. This ensures the agent receives
/// full file content again rather than "use cached context" stubs.
///
/// Expected payload fields:
///   - `session_id`     — identifies the Claude Code session
///   - `connection_id`  — identifies the MCP connection within the session
///     (Claude Code injects this; fall back to session_id if absent)
pub async fn run_post_compact_hook() -> Result<()> {
    let input: Value = serde_json::from_reader(std::io::stdin()).unwrap_or(Value::Null);

    let session_id = input
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    let connection_id = input
        .get("connection_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or(session_id);

    let (cid, sid) = match (connection_id, session_id) {
        (Some(c), Some(s)) => (c, s),
        _ => return Ok(()),
    };

    if let Err(e) = crate::mcp::send_compact_reset(cid, sid).await {
        eprintln!("so-context compact hook: {e}");
    }

    Ok(())
}
