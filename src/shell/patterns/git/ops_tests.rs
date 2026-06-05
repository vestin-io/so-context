use super::*;
use crate::shell::types::{CaptureMetadata, ShellInvocation};

#[test]
fn summarizes_add_result() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "git".into(),
            "add".into(),
            "src/main.rs".into(),
            "README.md".into(),
        ]),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_add(&result);
    assert_eq!(summary.summary, "ok staged (2 paths)");
    assert!(summary.details.is_empty());
}

#[test]
fn summarizes_clone_result() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "clone".into(), "repo".into()]),
        stdout: String::new(),
        stderr: "Cloning into 'repo'...\nremote: Enumerating objects: 5, done.\n".into(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_clone(&result);
    assert_eq!(summary.summary, "ok cloned 'repo'...");
    assert!(summary.stderr_preview.is_empty());
}

#[test]
fn summarizes_merge_with_stats() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "merge".into(), "feat".into()]),
        stdout:
            "Fast-forward\n src/main.rs | 3 ++-\n 1 file changed, 2 insertions(+), 1 deletion(-)\n"
                .into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_merge(&result);
    assert_eq!(summary.summary, "ok 1 files, +2/-1");
}

#[test]
fn summarizes_merge_conflicts() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "merge".into(), "feat".into()]),
        stdout: "Auto-merging src/main.rs\nCONFLICT (content): Merge conflict in src/main.rs\nAutomatic merge failed; fix conflicts and then commit.\n".into(),
        stderr: String::new(),
        exit_code: 1,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_merge(&result);
    assert_eq!(summary.summary, "merge conflict (1 conflicts)");
}

#[test]
fn summarizes_tag_list() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "tag".into()]),
        stdout: "v1.0.0\nv1.1.0\nv2.0.0\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_tag(&result);
    assert_eq!(summary.summary, "v1.0.0, v1.1.0, v2.0.0 (3 tags)");
}

#[test]
fn summarizes_reset_result() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "reset".into(), "HEAD~1".into()]),
        stdout: "Unstaged changes after reset:\nM\tsrc/main.rs\nD\tsrc/old.rs\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_reset(&result);
    assert_eq!(summary.summary, "reset ok (2 files unstaged)");
    assert_eq!(summary.details[0], "D\tsrc/old.rs");
}

#[test]
fn summarizes_stash_save() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "stash".into()]),
        stdout: "Saved working directory and index state WIP on main: abc1234 fix\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_stash(&result);
    assert_eq!(summary.summary, "ok stashed");
}

#[test]
fn summarizes_stash_list() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "stash".into(), "list".into()]),
        stdout: "stash@{0}: WIP on main: abc1234 fix\nstash@{1}: WIP on main: def4567 chore\n"
            .into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_stash(&result);
    assert_eq!(summary.summary, "2 stash entries");
    assert_eq!(summary.details[0], "stash@{0}: abc1234 fix");
}
