use super::{CompressionSummary, ShellPattern};
use std::path::PathBuf;

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
fn invocation_can_carry_working_directory() {
    let invocation = ShellInvocation::with_cwd(
        vec!["git".into(), "status".into()],
        PathBuf::from("/tmp/demo"),
    );

    assert_eq!(invocation.cwd(), Some(PathBuf::from("/tmp/demo").as_path()));
}

#[test]
fn shell_command_invocation_prefers_original_command_line() {
    let invocation = ShellInvocation::shell_command(
        vec!["ls".into(), "*.rs".into()],
        "/bin/zsh".into(),
        "ls *.rs".into(),
    );

    assert_eq!(invocation.command_line(), "ls *.rs");
    assert_eq!(invocation.execution_program(), "/bin/zsh");
    assert_eq!(
        invocation.execution_args(),
        vec!["-lc".to_string(), "ls *.rs".to_string()]
    );
}

#[test]
fn full_render_includes_newline_between_stdout_and_stderr() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["sh".into(), "-c".into(), "echo demo".into()]),
        stdout: "hello".into(),
        stderr: "warn".into(),
        exit_code: 7,
    };

    assert_eq!(result.render_full(), "hello\nwarn\n");
}
