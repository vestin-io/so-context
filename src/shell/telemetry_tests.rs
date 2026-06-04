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
        rendered: "compressed output".into(),
        full_output: "full output".into(),
        exit_code: 0,
        output_mode: ShellOutputMode::Compressed,
        requested_full: false,
        stdout_bytes: 128,
        stderr_bytes: 8,
    }
}

#[test]
fn shell_event_redacts_sensitive_argv_in_params() {
    let event = build_shell_event(
        ShellEventContext::cli_shell("run-1".into(), Some(PathBuf::from("/tmp/demo"))),
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
fn cli_shell_context_uses_hook_env_when_present() {
    unsafe {
        std::env::set_var("SO_CONTEXT_CLIENT", "codex");
        std::env::set_var("SO_CONTEXT_SESSION_ID", "session-123");
        std::env::set_var("SO_CONTEXT_SESSION_SOURCE", "hook");
    }

    let context =
        ShellEventContext::cli_shell("run-fallback".into(), Some(PathBuf::from("/tmp/demo")));

    assert_eq!(context.client.as_deref(), Some("codex"));
    assert_eq!(context.session_id, "session-123");
    assert_eq!(context.session_source, "hook");

    unsafe {
        std::env::remove_var("SO_CONTEXT_CLIENT");
        std::env::remove_var("SO_CONTEXT_SESSION_ID");
        std::env::remove_var("SO_CONTEXT_SESSION_SOURCE");
    }
}

#[test]
fn shell_error_event_includes_basic_failure_context() {
    let event = build_shell_error_event(
        ShellEventContext::cli_shell("run-2".into(), Some(PathBuf::from("/tmp/demo"))),
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
