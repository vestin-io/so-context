use super::*;
use crate::shell::types::ShellInvocation;

fn summarize_case(result: &ShellResult) -> CompressionSummary {
    summarize(result, super::super::classify_only(result))
}

#[test]
fn summarizes_npm_and_strips_boilerplate() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["npm".into(), "build".into()]),
        stdout: "\n> app@1.0.0 build\n> next build\n\nnpm WARN deprecated old-lib\nnpm notice\nCreating an optimized production build...\n✓ Build completed\n".into(),
        stderr: String::new(),
        exit_code: 0,
    stdout_total_bytes: 0,
    stderr_total_bytes: 0,
    stdout_truncated: false,
    stderr_truncated: false,
    capture_stdout_limit_bytes: 1024,
    capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::NodeNpm);
    assert_eq!(summary.summary, "Creating an optimized production build...");
    assert_eq!(summary.details[0], "✓ Build completed");
}

#[test]
fn keeps_more_meaningful_node_lines_before_truncating() {
    let stdout = (0..12)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["npm".into(), "run".into(), "demo".into()]),
        stdout: format!("{stdout}\n"),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.summary, "line 0");
    assert_eq!(summary.details.len(), 11);
    assert_eq!(summary.details[0], "line 1");
    assert_eq!(summary.details[10], "line 11");
}

#[test]
fn summarizes_pnpm() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["pnpm".into(), "install".into()]),
        stdout:
            "Progress: resolved 0, reused 0, downloaded 0, added 0\nPackages: +12\nDone in 2.3s\n"
                .into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::NodePnpm);
    assert_eq!(summary.summary, "Done in 2.3s");
}

#[test]
fn summarizes_yarn() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["yarn".into(), "install".into()]),
        stdout: "[1/4] Resolving packages...\n[2/4] Fetching packages...\nDone in 0.45s.\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::NodeYarn);
    assert_eq!(summary.summary, "Done in 0.45s.");
}

#[test]
fn summarizes_bun() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["bun".into(), "install".into()]),
        stdout: "bun install v1.2.0\nSaved lockfile\n3 packages installed\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::NodeBun);
    assert_eq!(summary.summary, "bun install v1.2.0");
    assert_eq!(summary.details[0], "Saved lockfile");
}

#[test]
fn strips_shell_echo_from_node_stderr_preview() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["bun".into(), "run".into(), "demo".into()]),
        stdout: "Creating an optimized production build...\n✓ Build completed\n".into(),
        stderr: "$ node scripts/demo.js\n".into(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert!(summary.stderr_preview.is_empty());
}

#[test]
fn summarizes_npx() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["npx".into(), "cowsay".into(), "hello".into()]),
        stdout: " _______\n< hello >\n -------\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::NodeNpx);
    assert_eq!(summary.summary, "_______");
    assert_eq!(summary.details[0], "< hello >");
}
