use super::super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::super::argv::first_positional;
use super::{diff, ops, refs, status, transport};

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    if program != "git" {
        return None;
    }

    match first_positional(args, &["-C", "-c", "--git-dir", "--work-tree"]) {
        Some("status") => Some(ShellPattern::GitStatus),
        Some("diff") => Some(ShellPattern::GitDiff),
        Some("log") => Some(ShellPattern::GitLog),
        Some("branch") => Some(ShellPattern::GitBranch),
        Some("remote") => Some(ShellPattern::GitRemote),
        Some("show") => Some(ShellPattern::GitShow),
        Some("fetch") => Some(ShellPattern::GitFetch),
        Some("pull") => Some(ShellPattern::GitPull),
        Some("push") => Some(ShellPattern::GitPush),
        Some("checkout") => Some(ShellPattern::GitCheckout),
        Some("switch") => Some(ShellPattern::GitSwitch),
        Some("commit") => Some(ShellPattern::GitCommit),
        Some("add") => Some(ShellPattern::GitAdd),
        Some("clone") => Some(ShellPattern::GitClone),
        Some("merge") => Some(ShellPattern::GitMerge),
        Some("tag") => Some(ShellPattern::GitTag),
        Some("reset") => Some(ShellPattern::GitReset),
        Some("stash") => Some(ShellPattern::GitStash),
        _ => None,
    }
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    Some(match pattern {
        ShellPattern::GitStatus => status::summarize(result),
        ShellPattern::GitDiff => diff::summarize(result),
        ShellPattern::GitLog => refs::summarize_log(result),
        ShellPattern::GitBranch => refs::summarize_branch(result),
        ShellPattern::GitRemote => refs::summarize_remote(result),
        ShellPattern::GitShow => refs::summarize_show(result),
        ShellPattern::GitFetch => transport::summarize_fetch(result),
        ShellPattern::GitPull => transport::summarize_pull(result),
        ShellPattern::GitPush => transport::summarize_push(result),
        ShellPattern::GitCheckout => transport::summarize_checkout(result),
        ShellPattern::GitSwitch => transport::summarize_switch(result),
        ShellPattern::GitCommit => transport::summarize_commit(result),
        ShellPattern::GitAdd => ops::summarize_add(result),
        ShellPattern::GitClone => ops::summarize_clone(result),
        ShellPattern::GitMerge => ops::summarize_merge(result),
        ShellPattern::GitTag => ops::summarize_tag(result),
        ShellPattern::GitReset => ops::summarize_reset(result),
        ShellPattern::GitStash => ops::summarize_stash(result),
        _ => return None,
    })
}
