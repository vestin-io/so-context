use super::SessionService;
use crate::host_adapter::HostKind;
use serde_json::json;

#[test]
fn standard_hook_hosts_extract_layered_identity() {
    let service = SessionService::new();
    let identity = service.identity_from_hook_input(
        HostKind::Codex,
        &json!({
            "session_id": "session-123",
            "agent_id": "agent-456",
            "connection_id": "conn-789"
        }),
    );

    assert_eq!(identity.session_id.as_deref(), Some("session-123"));
    assert_eq!(identity.agent_id.as_deref(), Some("agent-456"));
    assert_eq!(identity.connection_id.as_deref(), Some("conn-789"));
}

#[test]
fn opencode_identity_reads_nested_session_path() {
    let service = SessionService::new();
    let identity = service.identity_from_hook_input(
        HostKind::OpenCode,
        &json!({
            "session": {
                "id": "session-nested"
            }
        }),
    );

    assert_eq!(identity.session_id.as_deref(), Some("session-nested"));
    assert_eq!(identity.agent_id, None);
    assert_eq!(identity.connection_id, None);
}

#[test]
fn opencode_identity_falls_back_to_flat_session_id() {
    let service = SessionService::new();
    let identity = service.identity_from_hook_input(
        HostKind::OpenCode,
        &json!({
            "sessionID": "session-flat"
        }),
    );

    assert_eq!(identity.session_id.as_deref(), Some("session-flat"));
}

#[test]
fn compact_reset_falls_back_to_session_id_without_connection_id() {
    let service = SessionService::new();
    let ids = service.compact_reset_ids(
        HostKind::Claude,
        &json!({
            "session_id": "session-123"
        }),
    );

    assert_eq!(
        ids,
        Some(("session-123".to_string(), "session-123".to_string()))
    );
}

#[test]
fn compact_reset_uses_nested_opencode_session_id() {
    let service = SessionService::new();
    let ids = service.compact_reset_ids(
        HostKind::OpenCode,
        &json!({
            "session": {
                "id": "session-nested"
            }
        }),
    );

    assert_eq!(
        ids,
        Some(("session-nested".to_string(), "session-nested".to_string()))
    );
}
