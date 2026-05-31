use super::*;
use crate::shell::types::ShellInvocation;

#[test]
fn summarizes_log_entries() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "log".into(), "--oneline".into()]),
        stdout: "abc1234 first\nbcd2345 second\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_log(&result);
    assert_eq!(summary.summary, "2 commits");
    assert_eq!(summary.details[0], "abc1234 first");
}

#[test]
fn summarizes_raw_log_entries() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "log".into()]),
        stdout: "commit abc1234567890\nAuthor: Dev <dev@example.com>\nDate: Thu May 29 10:00:00 2026 +1200\n\n    improve shell diff\n\ncommit def4567890123\nAuthor: Dev <dev@example.com>\nDate: Thu May 29 09:00:00 2026 +1200\n\n    fix branch output\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_log(&result);
    assert_eq!(summary.summary, "2 commits");
    assert_eq!(summary.details[0], "abc1234 improve shell diff");
    assert_eq!(summary.details[1], "def4567 fix branch output");
}

#[test]
fn summarizes_branch_list() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "branch".into(), "-a".into()]),
        stdout: "* main\n  feat-shell\n  remotes/origin/HEAD -> origin/main\n  remotes/origin/feat-remote\n  remotes/origin/main\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_branch(&result);
    assert!(summary.summary.contains("current=main"));
    assert!(summary.summary.contains("local=2"));
    assert!(summary.summary.contains("remote_only=1"));
    assert_eq!(summary.details[2], "remote-only (1): feat-remote");
}

#[test]
fn summarizes_remote_list() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "remote".into(), "-v".into()]),
        stdout: "origin git@github.com:vestin-io/so-context.git (fetch)\norigin git@github.com:vestin-io/so-context.git (push)\nupstream git@github.com:example/upstream.git (fetch)\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_remote(&result);
    assert_eq!(summary.summary, "2 remotes");
    assert_eq!(
        summary.details[0],
        "origin: git@github.com:vestin-io/so-context.git"
    );
}

#[test]
fn summarizes_show_output() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "show".into(), "HEAD".into()]),
        stdout: "commit abc1234567890\nAuthor: Dev <dev@example.com>\nDate: Thu May 29 10:00:00 2026 +1200\n\n    improve shell diff\n\ndiff --git a/src/main.rs b/src/main.rs\n@@ -1 +1 @@\n-old\n+new\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_show(&result);
    assert!(summary.summary.contains("abc1234 improve shell diff"));
    assert!(summary.summary.contains("1 files changed"));
    assert_eq!(
        summary.details[0],
        "src/main.rs (+1/-1, 1 hunk) — @@ -1 +1 @@ | -old | +new"
    );
}

#[test]
fn summarizes_show_stat_without_patch() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "git".into(),
            "show".into(),
            "--stat".into(),
            "HEAD".into(),
        ]),
        stdout: "commit abc1234567890\nAuthor: Dev <dev@example.com>\nDate: Thu May 29 10:00:00 2026 +1200\n\n    improve shell diff\n\n src/main.rs | 2 +-\n 1 file changed, 1 insertion(+), 1 deletion(-)\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_show(&result);
    assert_eq!(
        summary.summary,
        "abc1234 improve shell diff; ok 1 files, +1/-1"
    );
    assert_eq!(summary.details[0], "commit abc1234567890");
    assert_eq!(
        summary.details[5],
        "1 file changed, 1 insertion(+), 1 deletion(-)"
    );
}
