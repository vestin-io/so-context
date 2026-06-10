use super::prefers_plain_text_tool_output;

#[test]
fn codex_clients_prefer_plain_text_tool_output() {
    assert!(prefers_plain_text_tool_output(Some("Codex")));
    assert!(prefers_plain_text_tool_output(Some("codex-desktop")));
    assert!(prefers_plain_text_tool_output(Some("OpenAI Codex CLI")));
}

#[test]
fn non_codex_clients_keep_structured_content() {
    assert!(!prefers_plain_text_tool_output(None));
    assert!(!prefers_plain_text_tool_output(Some("claude-code")));
    assert!(!prefers_plain_text_tool_output(Some("cursor")));
}
