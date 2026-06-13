use super::{HostKind, ToolNamespace};

#[test]
fn codex_uses_mcp_qualified_tool_names() {
    assert_eq!(
        ToolNamespace::for_host(HostKind::Codex, "mcp__so-context__so_read"),
        ToolNamespace::SoContextMcp
    );
    assert_eq!(
        ToolNamespace::for_host(HostKind::Codex, "mcp__other__search"),
        ToolNamespace::OtherMcp
    );
}

#[test]
fn opencode_uses_flattened_tool_names() {
    assert_eq!(
        ToolNamespace::for_host(HostKind::OpenCode, "so-context_so_read"),
        ToolNamespace::SoContextMcp
    );
    assert_eq!(
        ToolNamespace::for_host(HostKind::OpenCode, "github_list_prs"),
        ToolNamespace::OtherMcp
    );
    assert_eq!(
        ToolNamespace::for_host(HostKind::OpenCode, "Read"),
        ToolNamespace::Native
    );
}

#[test]
fn unknown_host_keeps_backward_compatible_detection() {
    assert_eq!(
        ToolNamespace::from_tool_name("mcp__so-context__so_search"),
        ToolNamespace::SoContextMcp
    );
    assert_eq!(
        ToolNamespace::from_tool_name("so-context_so_search"),
        ToolNamespace::SoContextMcp
    );
}
