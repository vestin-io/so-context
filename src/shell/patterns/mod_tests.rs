use super::*;
use crate::shell::types::ShellInvocation;

fn result(argv: &[&str]) -> ShellResult {
    ShellResult {
        invocation: ShellInvocation::new(argv.iter().map(|part| (*part).to_string()).collect()),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
    }
}

#[test]
fn classifies_docker_compose_alias_subcommands() {
    assert_eq!(
        classify_only(&result(&["docker-compose", "ps"])),
        ShellPattern::DockerCompose
    );
    assert_eq!(
        classify_only(&result(&["docker-compose", "logs", "web"])),
        ShellPattern::DockerLogs
    );
    assert_eq!(
        classify_only(&result(&["docker-compose", "build", "web"])),
        ShellPattern::DockerBuild
    );
    assert_eq!(
        classify_only(&result(&["docker-compose", "pull", "web"])),
        ShellPattern::DockerPull
    );
}

#[test]
fn classifies_new_shell_families() {
    assert_eq!(
        classify_only(&result(&["npm", "install"])),
        ShellPattern::NodeNpm
    );
    assert_eq!(
        classify_only(&result(&["pnpm", "build"])),
        ShellPattern::NodePnpm
    );
    assert_eq!(
        classify_only(&result(&["cargo", "test"])),
        ShellPattern::CargoTest
    );
    assert_eq!(
        classify_only(&result(&["gh", "pr", "list"])),
        ShellPattern::GhPr
    );
    assert_eq!(
        classify_only(&result(&["kubectl", "get", "pods"])),
        ShellPattern::KubectlPods
    );
    assert_eq!(
        classify_only(&result(&["kubectl", "get", "-n", "default", "pods"])),
        ShellPattern::KubectlPods
    );
    assert_eq!(
        classify_only(&result(&["tsc", "--noEmit"])),
        ShellPattern::Tsc
    );
    assert_eq!(
        classify_only(&result(&["./gradlew", "build"])),
        ShellPattern::Gradle
    );
    assert_eq!(
        classify_only(&result(&["dotnet", "test"])),
        ShellPattern::DotnetTest
    );
    assert_eq!(
        classify_only(&result(&["git", "add", "."])),
        ShellPattern::GitAdd
    );
    assert_eq!(
        classify_only(&result(&["git", "clone", "repo"])),
        ShellPattern::GitClone
    );
    assert_eq!(
        classify_only(&result(&["git", "merge", "main"])),
        ShellPattern::GitMerge
    );
    assert_eq!(
        classify_only(&result(&["git", "tag"])),
        ShellPattern::GitTag
    );
    assert_eq!(
        classify_only(&result(&["git", "reset", "--soft"])),
        ShellPattern::GitReset
    );
    assert_eq!(
        classify_only(&result(&["git", "stash", "list"])),
        ShellPattern::GitStash
    );
    assert_eq!(
        classify_only(&result(&["git", "-C", "repo", "status"])),
        ShellPattern::GitStatus
    );
    assert_eq!(
        classify_only(&result(&[
            "docker",
            "compose",
            "-f",
            "compose.yml",
            "logs",
            "web",
        ])),
        ShellPattern::DockerLogs
    );
}

#[test]
fn classifies_positionals_after_double_dash() {
    assert_eq!(
        classify_only(&result(&[
            "cargo",
            "test",
            "--",
            "--exact",
            "parser::works"
        ])),
        ShellPattern::CargoTest
    );
}

#[test]
fn classifies_simple_shell_wrapped_commands_by_inner_program() {
    assert_eq!(
        classify_only(&result(&["sh", "-c", "git status --short"])),
        ShellPattern::GitStatus
    );
    assert_eq!(
        classify_only(&result(&["bash", "-lc", "rg -n todo src"])),
        ShellPattern::Rg
    );
    assert_eq!(
        classify_only(&result(&["zsh", "-c", "FOO=bar find src -name '*.rs'"])),
        ShellPattern::Find
    );
}

#[test]
fn classifies_shell_wrapped_text_excerpt_and_rg_files() {
    assert_eq!(
        classify_only(&result(&[
            "sh",
            "-c",
            "nl -ba /tmp/demo.rs | sed -n '1,120p'"
        ])),
        ShellPattern::TextExcerpt
    );
    assert_eq!(
        classify_only(&result(&["rg", "--files", "src"])),
        ShellPattern::RgFiles
    );
}
