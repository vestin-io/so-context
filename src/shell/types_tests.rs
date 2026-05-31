use super::{CompressionSummary, ShellPattern};
use crate::shell::types::{ShellInvocation, ShellResult};

fn sample_summary() -> CompressionSummary {
    CompressionSummary {
        pattern: ShellPattern::GitDiff,
        summary: "files=2; hunks=3; additions=10; deletions=4".into(),
        details: vec!["src/main.rs".into(), "README.md".into()],
        stderr_preview: vec!["warning: demo".into()],
    }
}

#[test]
fn render_includes_summary_details_and_stderr() {
    let rendered = sample_summary().render();

    assert!(rendered.starts_with("files=2; hunks=3; additions=10; deletions=4\n"));
    assert!(rendered.contains("- src/main.rs\n"));
    assert!(rendered.contains("stderr:\n- warning: demo\n"));
    assert!(!rendered.contains("pattern:"));
}

#[test]
fn command_line_quotes_shell_sensitive_args() {
    let invocation = ShellInvocation::new(vec![
        "git".into(),
        "-C".into(),
        "my repo".into(),
        "commit".into(),
        "-m".into(),
        "it's done".into(),
    ]);

    assert_eq!(
        invocation.command_line(),
        "git -C 'my repo' commit -m 'it'\\''s done'"
    );
}

#[test]
fn full_render_len_matches_rendered_output() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["sh".into(), "-c".into(), "echo demo".into()]),
        stdout: "hello".into(),
        stderr: "warn".into(),
        exit_code: 7,
    };

    assert_eq!(result.render_full().len(), result.render_full_len());
}
