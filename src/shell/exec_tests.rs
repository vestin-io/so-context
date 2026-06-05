use super::*;
use crate::shell::types::ShellInvocation;
use std::time::Duration;

#[test]
fn truncates_large_stdout_and_marks_stderr() {
    let result = execute_with_limits(
        ShellInvocation::new(vec![
            "sh".into(),
            "-c".into(),
            "yes a | head -c 1024".into(),
        ]),
        ExecLimits {
            max_stdout_bytes: 64,
            max_stderr_bytes: 64,
            timeout: Duration::from_secs(1),
        },
    )
    .unwrap();

    assert!(result.stdout.len() <= 64);
    assert!(
        result
            .stderr
            .contains("[shell] stdout truncated after 64 bytes")
    );
    assert_eq!(result.stdout_total_bytes, 1024);
    assert_eq!(result.stderr_total_bytes, 0);
    assert!(result.stdout_truncated);
    assert!(!result.stderr_truncated);
    assert_eq!(result.capture_stdout_limit_bytes, 64);
    assert_eq!(result.capture_stderr_limit_bytes, 64);
}

#[test]
fn waits_for_long_running_command() {
    let result = execute_with_limits(
        ShellInvocation::new(vec!["sh".into(), "-c".into(), "sleep 0.2".into()]),
        ExecLimits {
            max_stdout_bytes: 64,
            max_stderr_bytes: 64,
            timeout: Duration::from_secs(1),
        },
    )
    .unwrap();

    assert_eq!(result.exit_code, 0);
    assert!(!result.stderr.contains("timed out"));
}

#[test]
fn kills_command_after_timeout() {
    let result = execute_with_limits(
        ShellInvocation::new(vec!["sh".into(), "-c".into(), "sleep 1".into()]),
        ExecLimits {
            max_stdout_bytes: 64,
            max_stderr_bytes: 64,
            timeout: Duration::from_millis(50),
        },
    )
    .unwrap();

    assert_eq!(result.exit_code, 124);
    assert!(
        result
            .stderr
            .contains("[shell] command timed out after 50ms")
    );
}

#[test]
fn decodes_non_utf8_output_lossily() {
    let result = execute_with_limits(
        ShellInvocation::new(vec!["sh".into(), "-c".into(), "printf '\\377'".into()]),
        ExecLimits {
            max_stdout_bytes: 64,
            max_stderr_bytes: 64,
            timeout: Duration::from_secs(1),
        },
    )
    .unwrap();

    assert_eq!(result.stdout, "\u{FFFD}");
}

#[test]
fn full_execution_uses_larger_capture_budget() {
    let result = execute_full(ShellInvocation::new(vec![
        "sh".into(),
        "-c".into(),
        "yes a | head -c 11534336".into(),
    ]))
    .unwrap();

    assert_eq!(result.exit_code, 0);
    assert!(!result.stderr.contains("stdout truncated"));
    assert_eq!(result.stdout.len(), 11_534_336);
}
