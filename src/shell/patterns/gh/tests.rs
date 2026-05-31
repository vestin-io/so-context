use super::*;
use crate::shell::types::ShellInvocation;

fn summarize_case(result: &ShellResult) -> CompressionSummary {
    summarize(result, super::super::classify_only(result))
}

#[test]
fn summarizes_gh_pr_list() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["gh".into(), "pr".into(), "list".into()]),
        stdout: "123\tRefine shell compression\tmain\tOPEN\t2026-05-29\n124\tFix docker summaries\tmain\tOPEN\t2026-05-29\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::GhPr);
    assert_eq!(summary.summary, "2 PRs");
    assert_eq!(summary.details[0], "#123 Refine shell compression");
}

#[test]
fn summarizes_gh_issue_list() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["gh".into(), "issue".into(), "list".into()]),
        stdout: "456\tOPEN\tTrack shell metrics\tneeds-triage\t2026-05-29\n".into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::GhIssue);
    assert_eq!(summary.summary, "1 issues");
    assert_eq!(summary.details[0], "#456 Track shell metrics");
}

#[test]
fn summarizes_gh_run_list() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["gh".into(), "run".into(), "list".into()]),
        stdout: "completed\tsuccess\tshell-ci\tCI\tmain\tpush\t123456\t8s\t2026-05-30T08:58:47Z\n"
            .into(),
        stderr: String::new(),
        exit_code: 0,
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::GhRun);
    assert_eq!(summary.summary, "1 runs");
    assert_eq!(summary.details[0], "shell-ci (success) [123456]");
}
