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

    let summary = summarize_status(&result);
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

    let summary = summarize_status(&result);
    assert!(summary.summary.contains("branch=main"));
    assert!(summary.summary.contains("staged=1"));
    assert!(summary.summary.contains("untracked=1"));
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

    let summary = summarize_diff(&result);
    assert!(summary.summary.contains("files=1"));
    assert!(summary.summary.contains("hunks=1"));
    assert!(summary.summary.contains("additions=2"));
    assert!(summary.summary.contains("deletions=1"));
    assert_eq!(summary.details[0], "src/main.rs (+2/-1, 1 hunk)");
}

#[test]
fn summarizes_log_entries() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "log".into(), "--oneline".into()]),
        stdout: "abc123 first\nbcd234 second\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_log(&result);
    assert!(summary.summary.contains("commits=2"));
    assert_eq!(summary.details[0], "abc123 first");
}

#[test]
fn summarizes_branch_list() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "branch".into()]),
        stdout: "* main\n  feat-shell\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_branch(&result);
    assert!(summary.summary.contains("current=main"));
    assert!(summary.summary.contains("branches=2"));
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
    assert!(summary.summary.contains("remotes=2"));
}

#[test]
fn summarizes_show_output() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "show".into(), "HEAD".into()]),
        stdout: "diff --git a/src/main.rs b/src/main.rs\n@@ -1 +1 @@\n-old\n+new\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_show(&result);
    assert!(summary.summary.contains("files=1"));
    assert!(
        summary
            .summary
            .contains("preview=diff --git a/src/main.rs b/src/main.rs")
    );
}

#[test]
fn summarizes_fetch_transport() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "fetch".into()]),
        stdout: "abc123..def456  main -> origin/main\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_fetch(&result);
    assert!(summary.summary.contains("fetch_lines=1"));
    assert!(summary.summary.contains("updated_refs=1"));
}

#[test]
fn summarizes_pull_transport() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "pull".into()]),
        stdout: "Fast-forward\n src/main.rs | 2 +-\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_pull(&result);
    assert!(summary.summary.contains("pull_lines=2"));
    assert!(summary.summary.contains("updated_refs=1"));
}

#[test]
fn summarizes_push_transport() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "push".into()]),
        stdout: "main -> main\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_push(&result);
    assert!(summary.summary.contains("push_lines=1"));
    assert!(summary.summary.contains("updated_refs=1"));
}

#[test]
fn summarizes_checkout_transition() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "git".into(),
            "checkout".into(),
            "feat-shell".into(),
        ]),
        stdout: "Switched to branch 'feat-shell'\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_checkout(&result);
    assert!(
        summary
            .summary
            .contains("transition=Switched to branch 'feat-shell'")
    );
}

#[test]
fn summarizes_switch_transition() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "switch".into(), "main".into()]),
        stdout: "Already on 'main'\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_switch(&result);
    assert!(summary.summary.contains("transition=Already on 'main'"));
}

#[test]
fn summarizes_commit_result() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "git".into(),
            "commit".into(),
            "-m".into(),
            "msg".into(),
        ]),
        stdout: "[main abc123] msg\n 1 file changed, 2 insertions(+)\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_commit(&result);
    assert!(summary.summary.contains("commit_result=[main abc123] msg"));
}
