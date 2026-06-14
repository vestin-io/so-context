use serde_json::json;

use super::adapter_for_host;
use crate::host_adapter::{HostKind, ToolNamespace};
use crate::routing_session::SessionService;
use serde_json::{Map, Value};

#[test]
fn codex_adapter_normalizes_pre_tool_calls() {
    let adapter = adapter_for_host(HostKind::Codex);
    let session_service = SessionService::new();
    let request = adapter.normalize_pre_tool_call(
        &session_service,
        &json!({
            "tool_name": "mcp__so-context__so_read",
            "session_id": "session-123",
            "tool_input": { "path": "src/main.rs" }
        }),
    );

    assert_eq!(request.host_kind, HostKind::Codex);
    assert_eq!(request.tool_namespace, ToolNamespace::SoContextMcp);
    assert_eq!(request.identity.session_id.as_deref(), Some("session-123"));
    assert_eq!(
        Value::Object(request.tool_input),
        json!({ "path": "src/main.rs" })
    );
}

#[test]
fn codex_adapter_preserves_native_shell_arguments() {
    let adapter = adapter_for_host(HostKind::Codex);
    let session_service = SessionService::new();
    let request = adapter.normalize_pre_tool_call(
        &session_service,
        &json!({
            "tool_name": "exec_command",
            "tool_input": { "cmd": "pwd", "tty": true }
        }),
    );

    assert_eq!(request.tool_namespace, ToolNamespace::Native);
    assert_eq!(
        Value::Object(request.tool_input),
        json!({ "cmd": "pwd", "tty": true })
    );
}

#[test]
fn opencode_adapter_uses_profile_identity_paths() {
    let adapter = adapter_for_host(HostKind::OpenCode);
    let session_service = SessionService::new();
    let request = adapter.normalize_plugin_tool_call(
        &session_service,
        &json!({
            "tool": "so-context_so_search",
            "session": { "id": "session-nested" },
        }),
        &Map::from_iter([("query".to_string(), Value::String("foo".to_string()))]),
    );

    assert_eq!(request.host_kind, HostKind::OpenCode);
    assert_eq!(request.tool_namespace, ToolNamespace::SoContextMcp);
    assert_eq!(
        request.identity.session_id.as_deref(),
        Some("session-nested")
    );
}
