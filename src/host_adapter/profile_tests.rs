use super::capability_profile;
use crate::host_adapter::HostKind;

#[test]
fn codex_profile_matches_current_hook_shape() {
    let profile = capability_profile(HostKind::Codex);

    assert_eq!(profile.kind, HostKind::Codex);
    assert_eq!(profile.display_name, "Codex");
    assert_eq!(profile.self_tool_naming.tool_prefix, "mcp__so-context__");
    assert_eq!(
        profile.self_tool_naming.hook_matcher(),
        "mcp__so-context__.*"
    );
    assert_eq!(profile.identity_paths.session_id_paths, &["session_id"]);
    assert_eq!(profile.identity_paths.agent_id_paths, &["agent_id"]);
}

#[test]
fn claude_profile_matches_current_runtime_contract() {
    let profile = capability_profile(HostKind::Claude);

    assert_eq!(profile.kind, HostKind::Claude);
    assert_eq!(profile.display_name, "Claude");
    assert_eq!(profile.self_tool_naming.tool_prefix, "mcp__so-context__");
    assert_eq!(profile.identity_paths.session_id_paths, &["session_id"]);
}

#[test]
fn opencode_profile_matches_plugin_based_integration() {
    let profile = capability_profile(HostKind::OpenCode);

    assert_eq!(profile.kind, HostKind::OpenCode);
    assert_eq!(profile.display_name, "OpenCode");
    assert_eq!(profile.self_tool_naming.tool_prefix, "so-context_");
    assert_eq!(profile.self_tool_naming.hook_matcher(), "so-context_*");
    assert_eq!(
        profile.identity_paths.session_id_paths,
        &["session.id", "sessionID"]
    );
    assert!(profile.identity_paths.connection_id_paths.is_empty());
}
