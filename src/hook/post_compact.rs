use anyhow::Result;
use serde_json::Value;

use crate::host_adapter::{HostKind, adapter_for_host};
use crate::routing_session::SessionService;

/// PostCompact hook handler.
///
/// Reads the host compact/reset JSON payload from stdin and sends a
/// `compact_reset` ctrl notification to the daemon so it clears file-visit
/// cache entries for the compacted session. This ensures the agent receives
/// full file content again rather than "use cached context" stubs.
///
/// Expected payload fields vary by host and are normalized through
/// `SessionService` plus the selected `AgentAdapter`.
pub async fn run_post_compact_hook_for_host(host_kind: HostKind) -> Result<()> {
    let adapter = adapter_for_host(host_kind);
    let input: Value = serde_json::from_reader(std::io::stdin()).unwrap_or(Value::Null);
    let session_service = SessionService::new();
    let Some((cid, sid)) = adapter.compact_reset_ids(&session_service, &input) else {
        return Ok(());
    };

    if let Err(e) = crate::mcp::send_compact_reset(&cid, &sid).await {
        eprintln!("so-context compact hook: {e}");
    }

    Ok(())
}
