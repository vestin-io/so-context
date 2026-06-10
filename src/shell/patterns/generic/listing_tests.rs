use crate::shell::types::{CaptureMetadata, ShellInvocation, ShellResult};

use super::super::{
    summarize_find, summarize_grep, summarize_ls, summarize_rg, summarize_rg_files,
};

#[test]
fn summarizes_rg_hits() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["rg".into(), "todo".into()]),
        stdout: "src/main.rs:10: TODO one\nsrc/lib.rs:7: TODO two\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_rg(&result);
    assert_eq!(summary.summary, "");
    assert_eq!(summary.details[0], "src/main.rs:10: TODO one");
    assert_eq!(summary.details[1], "src/lib.rs:7: TODO two");
}

#[test]
fn summarizes_ls_entries() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["ls".into()]),
        stdout: "Cargo.toml\nREADME.md\nsrc\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
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
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_find(&result);
    assert_eq!(summary.summary, "2 paths in 2 dirs (2 .rs)");
    assert_eq!(summary.details[0], "src/main.rs");
    assert_eq!(summary.details[1], "src/shell/mod.rs");
}

#[test]
fn preserves_exact_find_paths() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "find".into(),
            "/Users/jiatwork/spotto/ui/dist/apps/plugin".into(),
            "-maxdepth".into(),
            "2".into(),
            "-type".into(),
            "f".into(),
        ]),
        stdout: "/Users/jiatwork/spotto/ui/dist/apps/plugin/assets/generated/locales/enterprise/en.json\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_find(&result);
    assert!(summary.summary.contains("1 paths in 1 dirs"));
    assert_eq!(summary.details.len(), 1);
    assert_eq!(
        summary.details[0],
        "/Users/jiatwork/spotto/ui/dist/apps/plugin/assets/generated/locales/enterprise/en.json"
    );
}

#[test]
fn preserves_find_paths_with_significant_whitespace() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["find".into(), ".".into()]),
        stdout: "  spaced name.txt  \n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_find(&result);
    assert_eq!(summary.details, vec!["  spaced name.txt  "]);
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
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_grep(&result);
    assert_eq!(summary.summary, "");
    assert_eq!(summary.details[0], "./src/main.rs:10: TODO one");
    assert_eq!(summary.details[1], "./src/lib.rs:7: TODO two");
}

#[test]
fn summarizes_rg_files_as_plain_listing() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["rg".into(), "--files".into(), "src".into()]),
        stdout: "src/main.rs\nsrc/lib.rs\nsrc/shell/mod.rs\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_rg_files(&result);
    assert_eq!(summary.summary, "");
    assert_eq!(summary.details[0], "src/main.rs");
    assert_eq!(summary.details[2], "src/shell/mod.rs");
}
