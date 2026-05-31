use std::time::Duration;

use super::*;
use crate::shell::types::ShellInvocation;

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
            timeout: Duration::from_secs(2),
        },
    )
    .unwrap();

    assert!(result.stdout.len() <= 64);
    assert!(
        result
            .stderr
            .contains("[shell] stdout truncated after 64 bytes")
    );
}

#[test]
fn times_out_long_running_command() {
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
    assert!(result.stderr.contains("[shell] command timed out after 0s"));
}

#[test]
fn decodes_non_utf8_output_lossily() {
    let result = execute_with_limits(
        ShellInvocation::new(vec!["sh".into(), "-c".into(), "printf '\\377'".into()]),
        ExecLimits {
            max_stdout_bytes: 64,
            max_stderr_bytes: 64,
            timeout: Duration::from_secs(2),
        },
    )
    .unwrap();

    assert_eq!(result.stdout, "�");
}
