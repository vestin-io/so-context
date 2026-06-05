use super::*;
use crate::shell::types::{CaptureMetadata, ShellInvocation};

fn summarize_case(result: &ShellResult) -> CompressionSummary {
    summarize(result, super::super::classify_only(result))
}

#[test]
fn summarizes_cargo_build() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cargo".into(), "build".into()]),
        stdout: "Compiling so-context v0.1.0\nFinished `dev` profile [unoptimized + debuginfo] target(s) in 0.72s\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::CargoBuild);
    assert_eq!(summary.summary, "cargo build: ok");
}

#[test]
fn suppresses_success_stderr_noise_for_cargo_build() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cargo".into(), "build".into()]),
        stdout: String::new(),
        stderr: concat!(
            "   Compiling demo_fixture v0.1.0 (/tmp/demo)\n",
            "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.75s\n",
        )
        .into(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.summary, "cargo build: ok");
    assert_eq!(
        summary.details,
        vec!["Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.75s".to_string()]
    );
    assert!(summary.stderr_preview.is_empty());
}

#[test]
fn summarizes_cargo_test() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cargo".into(), "test".into()]),
        stdout: "running 2 tests\ntest a ... ok\ntest b ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(
        summary.summary,
        "cargo test: 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s"
    );
    assert!(summary.stderr_preview.is_empty());
}

#[test]
fn summarizes_cargo_test_across_multiple_targets() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cargo".into(), "test".into()]),
        stdout: concat!(
            "running 1 test\n",
            "test lib_test ... ok\n\n",
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s\n\n",
            "running 0 tests\n\n",
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n\n",
            "running 1 test\n",
            "test integration_test ... ok\n\n",
            "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\n",
            "running 0 tests\n\n",
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
        )
        .into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(
        summary.summary,
        "cargo test: 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s"
    );
}

#[test]
fn summarizes_cargo_clippy() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cargo".into(), "clippy".into()]),
        stdout: String::new(),
        stderr: "warning: this can be simplified\n".into(),
        exit_code: 1,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::CargoClippy);
    assert_eq!(summary.summary, "cargo clippy: 0 errors, 1 warnings");
}

#[test]
fn summarizes_cargo_check() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cargo".into(), "check".into()]),
        stdout: "Checking so-context v0.1.0\nFinished `dev` profile [unoptimized + debuginfo] target(s) in 0.33s\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.summary, "cargo check: ok");
}

#[test]
fn summarizes_cargo_install() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cargo".into(), "install".into(), "ripgrep".into()]),
        stdout: "Installed package `ripgrep v14.0.0` (executable `rg`)\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.summary, "cargo install: installed ripgrep v14.0.0");
    assert!(summary.stderr_preview.is_empty());
}

#[test]
fn keeps_only_warning_preview_for_successful_cargo_install() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cargo".into(), "install".into(), "--path".into(), ".".into()]),
        stdout: String::new(),
        stderr: concat!(
            "  Installing demo_fixture v0.1.0 (/tmp/demo)\n",
            "   Compiling demo_fixture v0.1.0 (/tmp/demo)\n",
            "    Finished `release` profile [optimized] target(s) in 0.22s\n",
            "  Installing /tmp/demo/install-root/bin/demo_fixture\n",
            "   Installed package `demo_fixture v0.1.0 (/tmp/demo)` (executable `demo_fixture`)\n",
            "warning: be sure to add `/tmp/demo/install-root/bin` to your PATH to be able to run the installed binaries\n",
        )
        .into(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(
        summary.summary,
        "cargo install: installed demo_fixture v0.1.0"
    );
    assert_eq!(summary.stderr_preview.len(), 1);
    assert!(summary.stderr_preview[0].starts_with("warning:"));
    assert!(
        !summary
            .stderr_preview
            .iter()
            .any(|line| line.contains("Compiling"))
    );
}

#[test]
fn summarizes_cargo_nextest() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cargo".into(), "nextest".into(), "run".into()]),
        stdout: "running 3 tests\n\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(
        summary.summary,
        "cargo nextest: 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s"
    );
}

#[test]
fn keeps_actionable_stderr_for_failing_cargo_test() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cargo".into(), "test".into()]),
        stdout: "running 1 test\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n".into(),
        stderr: "Compiling demo v0.1.0\nerror: test failed\nwarning: retrying\n".into(),
        exit_code: 101,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(
        summary.summary,
        "cargo test: 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s"
    );
    assert_eq!(summary.stderr_preview[0], "error: test failed");
    assert_eq!(summary.stderr_preview[1], "warning: retrying");
    assert!(
        !summary
            .stderr_preview
            .iter()
            .any(|line| line.contains("Compiling"))
    );
}
