mod docker;
mod generic;
mod git;
mod helpers;

use super::types::{CompressionSummary, ShellPattern, ShellResult};

pub fn compress(result: &ShellResult) -> CompressionSummary {
    match classify(result) {
        ShellPattern::GitStatus => git::summarize_status(result),
        ShellPattern::GitDiff => git::summarize_diff(result),
        ShellPattern::GitLog => git::summarize_log(result),
        ShellPattern::GitBranch => git::summarize_branch(result),
        ShellPattern::GitRemote => git::summarize_remote(result),
        ShellPattern::GitShow => git::summarize_show(result),
        ShellPattern::GitFetch => git::summarize_fetch(result),
        ShellPattern::GitPull => git::summarize_pull(result),
        ShellPattern::GitPush => git::summarize_push(result),
        ShellPattern::GitCheckout => git::summarize_checkout(result),
        ShellPattern::GitSwitch => git::summarize_switch(result),
        ShellPattern::GitCommit => git::summarize_commit(result),
        ShellPattern::DockerPs => docker::summarize_ps(result),
        ShellPattern::DockerImages => docker::summarize_images(result),
        ShellPattern::DockerCompose => docker::summarize_compose(result),
        ShellPattern::DockerLogs => docker::summarize_logs(result),
        ShellPattern::DockerBuild => docker::summarize_build(result),
        ShellPattern::DockerInspect => docker::summarize_inspect(result),
        ShellPattern::DockerPull => docker::summarize_pull(result),
        ShellPattern::Ls => generic::summarize_ls(result),
        ShellPattern::Find => generic::summarize_find(result),
        ShellPattern::Rg => generic::summarize_rg(result),
        ShellPattern::Grep => generic::summarize_grep(result),
        ShellPattern::Curl => generic::summarize_curl(result),
        ShellPattern::Wget => generic::summarize_wget(result),
        ShellPattern::Env => generic::summarize_env(result),
        ShellPattern::Cat => generic::summarize_cat(result),
        ShellPattern::Head => generic::summarize_head(result),
        ShellPattern::Tail => generic::summarize_tail(result),
        ShellPattern::Unknown => generic::summarize_unknown(result),
    }
}

fn classify(result: &ShellResult) -> ShellPattern {
    let program = result.invocation.program();
    let args = result.invocation.args();

    match (
        program,
        args.first().map(String::as_str),
        args.get(1).map(String::as_str),
    ) {
        ("git", Some("status"), _) => ShellPattern::GitStatus,
        ("git", Some("diff"), _) => ShellPattern::GitDiff,
        ("git", Some("log"), _) => ShellPattern::GitLog,
        ("git", Some("branch"), _) => ShellPattern::GitBranch,
        ("git", Some("remote"), _) => ShellPattern::GitRemote,
        ("git", Some("show"), _) => ShellPattern::GitShow,
        ("git", Some("fetch"), _) => ShellPattern::GitFetch,
        ("git", Some("pull"), _) => ShellPattern::GitPull,
        ("git", Some("push"), _) => ShellPattern::GitPush,
        ("git", Some("checkout"), _) => ShellPattern::GitCheckout,
        ("git", Some("switch"), _) => ShellPattern::GitSwitch,
        ("git", Some("commit"), _) => ShellPattern::GitCommit,
        ("docker", Some("ps"), _) => ShellPattern::DockerPs,
        ("docker", Some("images"), _) => ShellPattern::DockerImages,
        ("docker", Some("compose"), Some("logs")) => ShellPattern::DockerLogs,
        ("docker", Some("compose"), Some("build")) => ShellPattern::DockerBuild,
        ("docker", Some("compose"), Some("pull")) => ShellPattern::DockerPull,
        ("docker", Some("compose"), Some("ps")) => ShellPattern::DockerCompose,
        ("docker", Some("compose"), _) => ShellPattern::DockerCompose,
        ("docker", Some("logs"), _) => ShellPattern::DockerLogs,
        ("docker", Some("build"), _) => ShellPattern::DockerBuild,
        ("docker", Some("inspect"), _) => ShellPattern::DockerInspect,
        ("docker", Some("pull"), _) => ShellPattern::DockerPull,
        ("docker-compose", _, _) => ShellPattern::DockerCompose,
        ("ls", _, _) => ShellPattern::Ls,
        ("find", _, _) => ShellPattern::Find,
        ("rg", _, _) => ShellPattern::Rg,
        ("grep", _, _) => ShellPattern::Grep,
        ("curl", _, _) => ShellPattern::Curl,
        ("wget", _, _) => ShellPattern::Wget,
        ("env", _, _) => ShellPattern::Env,
        ("cat", _, _) => ShellPattern::Cat,
        ("head", _, _) => ShellPattern::Head,
        ("tail", _, _) => ShellPattern::Tail,
        _ => ShellPattern::Unknown,
    }
}
