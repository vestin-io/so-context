use super::*;
use crate::shell::types::ShellInvocation;

#[test]
fn summarizes_porcelain_status() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "status".into(), "--short".into()]),
        stdout: "## main\nM  src/main.rs\n M README.md\n?? src/shell/mod.rs\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = status::summarize(&result);
    assert!(summary.summary.starts_with("main |"));
    assert!(summary.summary.contains("staged=1"));
    assert!(summary.summary.contains("unstaged=1"));
    assert!(summary.summary.contains("untracked=1"));
}

#[test]
fn summarizes_human_status() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "status".into()]),
        stdout:
            "On branch main\nChanges to be committed:\n  modified:   src/main.rs\nUntracked files:\n\tsrc/shell/mod.rs\n"
                .into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = status::summarize(&result);
    assert!(summary.summary.starts_with("main |"));
    assert!(summary.summary.contains("staged=1"));
    assert!(summary.summary.contains("untracked=1"));
    assert_eq!(summary.details[0], "staged: src/main.rs");
    assert_eq!(summary.details[1], "untracked: src/shell/mod.rs");
}

#[test]
fn summarizes_diff_stats() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "diff".into()]),
        stdout: "diff --git a/src/main.rs b/src/main.rs\n@@ -1,2 +1,3 @@\n-old\n+new\n+extra\n"
            .into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = diff::summarize(&result);
    assert_eq!(summary.summary, "1 files changed, 1 hunks, +2/-1");
    assert_eq!(
        summary.details[0],
        "src/main.rs (+2/-1, 1 hunk) — @@ -1,2 +1,3 @@ | -old | +new | +extra"
    );
}
