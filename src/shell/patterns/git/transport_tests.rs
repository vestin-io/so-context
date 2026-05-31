use super::*;
use crate::shell::types::ShellInvocation;

#[test]
fn summarizes_fetch_transport() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "fetch".into()]),
        stdout: String::new(),
        stderr: "From ../remote\n * [new branch]      feat/fetch -> origin/feat/fetch\n * [new tag]         v1.1.0     -> v1.1.0\n".into(),
        exit_code: 0,
    };

    let summary = summarize_fetch(&result);
    assert_eq!(summary.summary, "ok fetched (2 new refs)");
    assert_eq!(summary.details[0], "origin/feat/fetch");
    assert!(summary.stderr_preview.is_empty());
}

#[test]
fn summarizes_pull_transport() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "pull".into()]),
        stdout: "Updating abc1234..def5678\nFast-forward\n src/main.rs | 2 +-\n 1 file changed, 1 insertion(+)\n".into(),
        stderr: "From ../remote\n abc1234..def5678  main -> origin/main\n".into(),
        exit_code: 0,
    };

    let summary = summarize_pull(&result);
    assert_eq!(summary.summary, "ok 1 files, +1/-0");
    assert_eq!(summary.details[0], "1 file changed, 1 insertion(+)");
    assert!(summary.stderr_preview.is_empty());
}

#[test]
fn summarizes_push_transport() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "push".into()]),
        stdout: String::new(),
        stderr: "To ../remote.git\n   abc1234..def5678  main -> main\n".into(),
        exit_code: 0,
    };

    let summary = summarize_push(&result);
    assert_eq!(summary.summary, "ok main");
    assert_eq!(summary.details[0], "abc1234..def5678  main -> main");
    assert!(summary.stderr_preview.is_empty());
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
        stdout: String::new(),
        stderr: "Already on 'main'\n".into(),
        exit_code: 0,
    };

    let summary = summarize_switch(&result);
    assert!(summary.summary.contains("transition=Already on 'main'"));
    assert_eq!(summary.details[0], "Already on 'main'");
    assert!(summary.stderr_preview.is_empty());
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
    assert_eq!(summary.summary, "[main abc123] msg");
}
