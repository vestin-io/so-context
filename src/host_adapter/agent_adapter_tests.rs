use serde_json::json;

use super::{adapter_for_host, normalize_plugin_tool_call};
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
        Value::Object(request.routing_input),
        json!({ "path": "src/main.rs" })
    );
}

#[test]
fn codex_adapter_canonicalizes_native_shell_arguments_for_routing() {
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
    assert_eq!(
        Value::Object(request.routing_input),
        json!({ "command": "pwd" })
    );
}

#[test]
fn codex_adapter_canonicalizes_native_read_arguments_for_routing() {
    let adapter = adapter_for_host(HostKind::Codex);
    let session_service = SessionService::new();
    let request = adapter.normalize_pre_tool_call(
        &session_service,
        &json!({
            "tool_name": "Read",
            "tool_input": {
                "file_path": "src/lib.rs",
                "start_line": 10,
                "end_line": 20,
                "line_numbers": true
            }
        }),
    );

    assert_eq!(
        Value::Object(request.routing_input),
        json!({
            "path": "src/lib.rs",
            "start_line": 10,
            "end_line": 20,
            "line_numbers": true
        })
    );
}

#[test]
fn codex_adapter_canonicalizes_native_search_arguments_for_routing() {
    let adapter = adapter_for_host(HostKind::Codex);
    let session_service = SessionService::new();
    let request = adapter.normalize_pre_tool_call(
        &session_service,
        &json!({
            "tool_name": "Grep",
            "tool_input": {
                "pattern": "ConfigRepository",
                "directory": "/tmp/project",
                "limit": 5
            }
        }),
    );

    assert_eq!(
        Value::Object(request.routing_input),
        json!({
            "query": "ConfigRepository",
            "path": "/tmp/project",
            "limit": 5
        })
    );
}

#[test]
fn codex_adapter_marks_extra_search_semantics_in_routing_input() {
    let adapter = adapter_for_host(HostKind::Codex);
    let session_service = SessionService::new();
    let request = adapter.normalize_pre_tool_call(
        &session_service,
        &json!({
            "tool_name": "SearchFiles",
            "tool_input": {
                "query": "ConfigRepository",
                "path": "/tmp/project",
                "case_sensitive": true
            }
        }),
    );

    assert_eq!(
        Value::Object(request.routing_input),
        json!({
            "query": "ConfigRepository",
            "path": "/tmp/project",
            "_has_extra_search_semantics": true
        })
    );
}

#[test]
fn opencode_adapter_uses_profile_identity_paths() {
    let session_service = SessionService::new();
    let request = normalize_plugin_tool_call(
        HostKind::OpenCode,
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
