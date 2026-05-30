//! `so-context hook` — PreToolUse hook handler for agent CLIs.
//!
//! Reads the hook JSON payload from stdin, injects `_so_session_id` into the
//! tool arguments for so-context MCP tool calls, and writes the rewritten
//! input to stdout. Exits 0 with no output for non-so-context tools so the
//! agent continues normally.
//!
//! Context window ID resolution:
//!   - `agent_id`   — Claude Code: present when firing inside a sub-agent,
//!                    uniquely identifies that context window.
//!   - `session_id` — Fallback: main thread (Claude Code) or any Codex call.
//!                    Codex sub-agents inherit the parent session_id.
//!
//! Output shape (Claude Code / Codex PreToolUse):
//! ```json
//! {
//!   "hookSpecificOutput": {
//!     "hookEventName": "PreToolUse",
//!     "permissionDecision": "allow",
//!     "updatedInput": { ...original tool_input..., "_so_session_id": "<id>" }
//!   }
//! }
//! ```

//! `so-context hook` — PreToolUse hook handler for agent CLIs.
//!
//! Reads the hook JSON payload from stdin, injects `_so_session_id` into the
//! tool arguments for so-context MCP tool calls, and writes the rewritten
//! input to stdout. Exits 0 with no output for non-so-context tools so the
//! agent continues normally.
//!
//! Context window ID resolution:
//!   - `agent_id`   — Claude Code: present when firing inside a sub-agent,
//!                    uniquely identifies that context window.
//!   - `session_id` — Fallback: main thread (Claude Code) or any Codex call.
//!                    Codex sub-agents inherit the parent session_id.
//!
//! Output shape (Claude Code / Codex PreToolUse):
//! ```json
//! {
//!   "hookSpecificOutput": {
//!     "hookEventName": "PreToolUse",
//!     "permissionDecision": "allow",
//!     "updatedInput": { ...original tool_input..., "_so_session_id": "<id>" }
//!   }
//! }
//! ```

use anyhow::Result;

const SO_CONTEXT_TOOL_PREFIX: &str = "mcp__so-context__";

pub fn run_pre_tool_use_hook() -> Result<()> {
    let input: serde_json::Value = serde_json::from_reader(std::io::stdin())
        .unwrap_or(serde_json::Value::Null);

    let tool_name = input
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Only act on so-context MCP tools — exit 0 silently for everything else.
    if !tool_name.starts_with(SO_CONTEXT_TOOL_PREFIX) {
        return Ok(());
    }

    // Claude Code: agent_id is present when firing inside a sub-agent and
    // uniquely identifies that context window. Fall back to session_id for
    // the main thread or for Codex (which only provides session_id).
    let context_id = input
        .get("agent_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            input
                .get("session_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
        });

    let context_id = match context_id {
        Some(id) => id,
        None => return Ok(()), // nothing to inject — exit 0 silently
    };

    // Merge _so_session_id into the existing tool_input.
    let mut tool_input = input
        .get("tool_input")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    tool_input.insert(
        "_so_session_id".to_string(),
        serde_json::Value::String(context_id.to_string()),
    );

    let output = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "updatedInput": tool_input,
        }
    });

    println!("{}", output);
    Ok(())
}

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
#[tokio::main]
pub async fn run_post_compact_hook() -> Result<()> {
    let input: serde_json::Value = serde_json::from_reader(std::io::stdin())
        .unwrap_or(serde_json::Value::Null);

    let session_id = input
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());

    let connection_id = input
        .get("connection_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or(session_id); // fallback: use session_id as connection_id

    let (cid, sid) = match (connection_id, session_id) {
        (Some(c), Some(s)) => (c, s),
        _ => {
            // Missing IDs — nothing useful to reset, exit silently.
            return Ok(());
        }
    };

    if let Err(e) = crate::mcp::send_compact_reset(cid, sid).await {
        eprintln!("so-context compact hook: {e}");
    }

    Ok(())
}
