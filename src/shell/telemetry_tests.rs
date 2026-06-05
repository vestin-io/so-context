use std::path::PathBuf;

use super::{ShellEventContext, build_shell_error_event, build_shell_event};
use crate::shell::types::{RunOutput, ShellInvocation, ShellOutputMode, ShellPattern};

fn sample_output() -> RunOutput {
    RunOutput {
        run_id: "run-1".into(),
        invocation: ShellInvocation::with_cwd(
            vec![
                "curl".into(),
                "-H".into(),
                "Authorization: Bearer super-secret".into(),
                "https://api.example.test/data?token=abc123".into(),
            ],
            PathBuf::from("/tmp/demo"),
        ),
        pattern: ShellPattern::Curl,
        rendered: Some("compressed output".into()),
        full_output: "full output".into(),
        exit_code: 0,
        output_mode: ShellOutputMode::Compressed,
        requested_full: false,
        stdout_bytes: 128,
        stderr_bytes: 8,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
        raw_output_complete: true,
    }
}

#[test]
fn shell_event_redacts_sensitive_argv_in_params() {
    let event = build_shell_event(
        ShellEventContext::mcp_shell(
            Some("codex".into()),
            Some("1.0.0".into()),
            "session-123".into(),
            "hook",
            PathBuf::from("/tmp/demo"),
        ),
        &sample_output(),
        "compressed output",
        12,
    );

    let params = event.params.unwrap();
    assert!(params.contains("[REDACTED]"));
    assert!(!params.contains("super-secret"));
    assert!(!params.contains("abc123"));
}

#[test]
fn shell_error_event_includes_basic_failure_context() {
    let event = build_shell_error_event(
        ShellEventContext::mcp_shell(
            Some("codex".into()),
            Some("1.0.0".into()),
            "session-123".into(),
            "hook",
            PathBuf::from("/tmp/demo"),
        ),
        &["git".into(), "diff".into()],
        Some(PathBuf::from("/tmp/demo").as_path()),
        false,
        9,
        "spawn failed",
    );

    assert!(!event.result_ok);
    assert_eq!(event.duration_ms, Some(9));
    assert!(event.params.unwrap().contains("spawn failed"));
}
