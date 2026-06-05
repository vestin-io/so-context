use super::*;
use crate::shell::types::ShellInvocation;

#[test]
fn summarizes_rg_hits() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["rg".into(), "todo".into()]),
        stdout: "src/main.rs:10: TODO one\nsrc/lib.rs:7: TODO two\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_rg(&result);
    assert_eq!(summary.summary, "2 matches in 2 files");
    assert_eq!(summary.details[0], "src/main.rs:10 — TODO one");
    assert_eq!(summary.details[1], "src/lib.rs:7 — TODO two");
}

#[test]
fn summarizes_ls_entries() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["ls".into()]),
        stdout: "Cargo.toml\nREADME.md\nsrc\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_ls(&result);
    assert!(summary.summary.contains("entries=3"));
    assert_eq!(summary.details[0], "src/");
    assert_eq!(summary.details.len(), 3);
}

#[test]
fn summarizes_find_paths() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["find".into(), "src".into()]),
        stdout: "src/main.rs\nsrc/shell/mod.rs\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_find(&result);
    assert_eq!(summary.summary, "2 paths in 2 dirs (2 .rs)");
    assert_eq!(summary.details[0], "src/main.rs");
    assert_eq!(summary.details[1], "src/shell/mod.rs");
}

#[test]
fn summarizes_grep_hits() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "grep".into(),
            "-R".into(),
            "todo".into(),
            ".".into(),
        ]),
        stdout: "./src/main.rs:10: TODO one\n./src/lib.rs:7: TODO two\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_grep(&result);
    assert_eq!(summary.summary, "2 matches in 2 files");
    assert_eq!(summary.details[0], "./src/main.rs:10 — TODO one");
    assert_eq!(summary.details[1], "./src/lib.rs:7 — TODO two");
}

#[test]
fn summarizes_head_excerpt() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "head".into(),
            "-n".into(),
            "2".into(),
            "README.md".into(),
        ]),
        stdout: "# so-context\nintro line\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_head(&result);
    assert!(summary.summary.contains("head_lines=2"));
    assert_eq!(summary.details[0], "# so-context");
}

#[test]
fn summarizes_env_keys() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["env".into()]),
        stdout: "PATH=/usr/bin:/bin\nAWS_SECRET_ACCESS_KEY=super-secret\nRUST_LOG=debug\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_env(&result);
    assert!(summary.summary.contains("vars=3"));
    assert_eq!(summary.details[0], "PATH=2 entries");
    assert_eq!(summary.details[1], "RUST_LOG=debug");
    assert_eq!(summary.details[2], "AWS_SECRET_ACCESS_KEY=***");
}

#[test]
fn summarizes_curl_body() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "curl".into(),
            "-s".into(),
            "https://example.com/api".into(),
        ]),
        stdout: "line one\nline two\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_curl(&result);
    assert!(
        summary
            .summary
            .contains("https://example.com/api ok | 2 lines")
    );
    assert_eq!(summary.details[0], "line one");
}

#[test]
fn redacts_sensitive_curl_source_url() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "curl".into(),
            "-s".into(),
            "https://user:pass@example.com/data?access_token=abc123&foo=bar".into(),
        ]),
        stdout: "line one\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_curl(&result);
    assert!(
        summary
            .summary
            .contains("https://[REDACTED]@example.com/data?… ok | 1 lines")
    );
    assert!(!summary.summary.contains("abc123"));
    assert!(!summary.summary.contains("user:pass"));
}

#[test]
fn summarizes_curl_json_passthrough_shape() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "curl".into(),
            "-s".into(),
            "https://example.com/data".into(),
        ]),
        stdout: "{\"ok\":true,\"items\":[1,2]}".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_curl(&result);
    assert_eq!(summary.summary, "json response | 25B");
    assert_eq!(summary.details[0], "{\"ok\":true,\"items\":[1,2]}");
}

#[test]
fn summarizes_wget_download_result() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "wget".into(),
            "https://example.com/archive.tgz".into(),
        ]),
        stdout: String::new(),
        stderr: "Saving to: 'archive.tgz'\n‘archive.tgz’ saved [1024/1024]\n".into(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_wget(&result);
    assert_eq!(
        summary.summary,
        "https://example.com/archive.tgz ok | archive.tgz | 1024/1024"
    );
}

#[test]
fn redacts_sensitive_wget_source_url() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "wget".into(),
            "https://user:pass@example.com/archive.tgz?signature=secret".into(),
        ]),
        stdout: String::new(),
        stderr: "Saving to: 'archive.tgz'\n‘archive.tgz’ saved [1024/1024]\n".into(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_wget(&result);
    assert_eq!(
        summary.summary,
        "https://[REDACTED]@example.com/archive.tgz?… ok | archive.tgz | 1024/1024"
    );
}

#[test]
fn summarizes_cat_excerpt() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cat".into(), "README.md".into()]),
        stdout: "# so-context\nintro line\nsecond line\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_cat(&result);
    assert_eq!(summary.summary, "cat_lines=3; bytes=36");
    assert_eq!(summary.details[0], "# so-context");
}

#[test]
fn summarizes_tail_excerpt() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "tail".into(),
            "-n".into(),
            "2".into(),
            "README.md".into(),
        ]),
        stdout: "line one\nline two\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_tail(&result);
    assert_eq!(summary.summary, "tail_lines=2; bytes=18");
    assert_eq!(summary.details[1], "line two");
}

#[test]
fn fallback_keeps_more_unknown_lines() {
    let stdout = (0..12)
        .map(|index| format!("row {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["jq".into(), ".".into(), "data.json".into()]),
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

    let summary = summarize_unknown(&result);
    assert_eq!(summary.summary, "12 lines | 74B");
    assert_eq!(summary.details.len(), 12);
    assert_eq!(summary.details[0], "row 0");
    assert_eq!(summary.details[11], "row 11");
}

#[test]
fn fallback_keeps_full_jq_json_output() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["jq".into(), ".".into(), "data.json".into()]),
        stdout: "{\n  \"name\": \"so-context\",\n  \"enabled\": true\n}\n".into(),
        stderr: String::new(),
        exit_code: 0,
        stdout_total_bytes: 0,
        stderr_total_bytes: 0,
        stdout_truncated: false,
        stderr_truncated: false,
        capture_stdout_limit_bytes: 1024,
        capture_stderr_limit_bytes: 1024,
    };

    let summary = summarize_unknown(&result);
    assert_eq!(summary.summary, "JSON output | 4 lines | 46B");
    assert_eq!(summary.details.len(), 4);
    assert_eq!(summary.details[0], "{");
    assert_eq!(summary.details[3], "}");
}

#[test]
fn fallback_keeps_more_sed_lines_before_truncating() {
    let stdout = (0..75)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["sed".into(), "-n".into(), "1,75p".into()]),
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

    let summary = summarize_unknown(&result);
    assert_eq!(summary.summary, "75 lines | 590B");
    assert_eq!(summary.details.len(), 75);
    assert_eq!(summary.details[0], "line 0");
    assert_eq!(summary.details[74], "line 74");
}

#[test]
fn fallback_keeps_more_shell_script_lines_before_truncating() {
    let stdout = (0..45)
        .map(|index| format!("step {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["bash".into(), "script.sh".into()]),
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

    let summary = summarize_unknown(&result);
    assert_eq!(summary.summary, "45 lines | 350B");
    assert_eq!(summary.details.len(), 45);
    assert_eq!(summary.details[0], "step 0");
    assert_eq!(summary.details[44], "step 44");
}
