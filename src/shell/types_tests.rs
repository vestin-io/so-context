use super::{CompressionRenderStyle, CompressionSummary, ShellPattern};
use std::path::PathBuf;

use crate::shell::types::{CaptureMetadata, ShellInvocation, ShellResult};

fn sample_summary() -> CompressionSummary {
    CompressionSummary {
        pattern: ShellPattern::GitDiff,
        summary: "files=2; hunks=3; additions=10; deletions=4".into(),
        details: vec!["src/main.rs".into(), "README.md".into()],
        stderr_preview: vec!["warning: demo".into()],
        render_style: CompressionRenderStyle::Bulleted,
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
fn plain_render_omits_bullets_for_details() {
    let rendered = CompressionSummary::plain(
        ShellPattern::GitStatus,
        "* main",
        vec!["M  src/main.rs".into(), "?? docs/notes.md".into()],
        Vec::new(),
        &ShellResult {
            invocation: ShellInvocation::new(vec!["git".into(), "status".into()]),
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            capture: CaptureMetadata::default(),
        },
    )
    .render();

    assert_eq!(rendered, "* main\nM  src/main.rs\n?? docs/notes.md\n");
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
fn full_render_includes_newline_between_stdout_and_stderr() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["sh".into(), "-c".into(), "echo demo".into()]),
        stdout: "hello".into(),
        stderr: "warn".into(),
        exit_code: 7,
        capture: CaptureMetadata {
            stdout_bytes: 5,
            stderr_bytes: 4,
            capture_stdout_limit_bytes: 1024,
            capture_stderr_limit_bytes: 1024,
            ..CaptureMetadata::default()
        },
    };

    assert_eq!(result.render_full(), "hello\nwarn\n");
}

#[test]
fn raw_output_is_incomplete_when_command_timed_out() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["sh".into(), "-c".into(), "sleep 1".into()]),
        stdout: String::new(),
        stderr: "[shell] command timed out after 50ms\n".into(),
        exit_code: 124,
        capture: CaptureMetadata {
            stderr_bytes: 35,
            timed_out: true,
            capture_stdout_limit_bytes: 1024,
            capture_stderr_limit_bytes: 1024,
            ..CaptureMetadata::default()
        },
    };

    assert!(!result.raw_output_complete());
}
