use serde_json::json;

use super::{RetryDirective, RetryTool, retry_policy_json};
use crate::host_adapter::HostKind;

#[test]
fn retry_reason_uses_mcp_qualified_tool_names_for_hook_hosts() {
    let retry = RetryDirective {
        tool: RetryTool::Read,
        argument_label: "arguments",
        arguments: json!({ "path": "src/main.rs", "mode": "full" }),
        rationale: "I routed this native file read through {tool} so the file content stays attributable and reusable in shared project context",
        usage_note: "Use `mode: \"outline\"` when you only need structure instead of full file text.",
    };

    let reason = retry.render_reason(HostKind::Codex);
    assert!(reason.contains("`mcp__so-context__so_read`"));
    assert!(reason.contains("\"path\":\"src/main.rs\""));
}

#[test]
fn retry_reason_uses_flattened_tool_names_for_opencode() {
    let retry = RetryDirective {
        tool: RetryTool::Search,
        argument_label: "arguments",
        arguments: json!({ "query": "ConfigRepository" }),
        rationale: "I routed this native search through {tool} so the hits stay attributable and reusable in shared project context",
        usage_note: "Keep native grep-style tools only when you need raw grep semantics or the project is not indexed.",
    };

    let reason = retry.render_reason(HostKind::OpenCode);
    assert!(reason.contains("`so-context_so_search`"));
    assert!(reason.contains("\"query\":\"ConfigRepository\""));
}

#[test]
fn retry_policy_json_renders_host_specific_tool_names() {
    let policy = retry_policy_json(HostKind::OpenCode);

    assert_eq!(
        policy
            .pointer("/read/tool_name")
            .and_then(|value| value.as_str()),
        Some("so-context_so_read")
    );
    assert_eq!(
        policy
            .pointer("/search/argument_label")
            .and_then(|value| value.as_str()),
        Some("arguments")
    );
}
