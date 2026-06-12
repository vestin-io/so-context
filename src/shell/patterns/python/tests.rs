use super::*;
use crate::shell::types::{CaptureMetadata, ShellInvocation};

fn summarize_case(argv: &[&str], stdout: &str, stderr: &str, exit_code: i32) -> CompressionSummary {
    let result = ShellResult {
        invocation: ShellInvocation::new(argv.iter().map(|part| (*part).to_string()).collect()),
        stdout: stdout.into(),
        stderr: stderr.into(),
        exit_code,
        capture: CaptureMetadata::default(),
    };
    summarize(&result, super::super::classify_only(&result))
}

#[test]
fn summarizes_python_module_pip_output() {
    let summary = summarize_case(
        &["python", "-m", "pip", "install", "httpx"],
        "Collecting httpx\nInstalling collected packages: httpx\nSuccessfully installed httpx-0.27.0\n",
        "",
        0,
    );

    assert_eq!(summary.pattern, ShellPattern::PythonPip);
    assert_eq!(summary.summary, "Successfully installed httpx-0.27.0");
}

#[test]
fn summarizes_pytest_failures() {
    let summary = summarize_case(
        &["pytest", "-q"],
        "FAILED tests/test_api.py::test_health - AssertionError: expected 200\n==================== 1 failed, 4 passed in 0.82s ====================\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::PythonPytest);
    assert_eq!(summary.summary, "Pytest: 4 passed, 1 failed");
    assert_eq!(summary.details[0], "Failures:");
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("test_health"))
    );
}

#[test]
fn summarizes_ruff_output() {
    let summary = summarize_case(
        &["ruff", "check", "."],
        "Found 2 errors.\nsrc/app.py:1:1: F401 `os` imported but unused\nsrc/app.py:4:1: E402 module level import not at top of file\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::PythonRuff);
    assert_eq!(summary.summary, "Ruff: 2 issues in 1 file");
    assert!(summary.details.iter().any(|line| line == "Top rules:"));
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("F401 (1x)"))
    );
    assert!(summary.details.iter().any(|line| line == "Violations:"));
}

#[test]
fn classifies_poetry_run_inner_tool() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "poetry".into(),
            "run".into(),
            "mypy".into(),
            "src".into(),
        ]),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    assert_eq!(
        super::super::classify_only(&result),
        ShellPattern::PythonMypy
    );
}

#[test]
fn summarizes_pytest_xfail_and_xpass() {
    let summary = summarize_case(
        &["pytest", "-q"],
        "=== short test summary info ===\nXFAIL tests/test_math.py::test_division_by_zero - known bug\nXPASS tests/test_math.py::test_unexpected_pass - currently passes\n2 passed, 0 failed, 1 xfailed, 1 xpassed in 0.05s\n",
        "",
        0,
    );

    assert_eq!(
        summary.summary,
        "Pytest: 2 passed, 0 failed, 1 xfailed, 1 xpassed"
    );
    assert_eq!(summary.details[0], "Expected-failure outcomes:");
    assert!(summary.details.iter().any(|line| line.contains("XFAIL")));
    assert!(summary.details.iter().any(|line| line.contains("XPASS")));
}

#[test]
fn summarizes_pytest_no_tests_collected() {
    let summary = summarize_case(
        &["pytest"],
        "==================== no tests ran in 0.00s ====================\n",
        "",
        5,
    );

    assert_eq!(summary.summary, "Pytest: No tests collected");
}

#[test]
fn summarizes_ruff_format_check() {
    let summary = summarize_case(
        &["ruff", "format", "--check", "."],
        "Would reformat: src/main.py\nWould reformat: tests/test_utils.py\n2 files would be reformatted, 3 files left unchanged\n",
        "",
        1,
    );

    assert_eq!(summary.summary, "Ruff format: 2 files need formatting");
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("src/main.py"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("tests/test_utils.py"))
    );
}

#[test]
fn summarizes_mypy_grouped_errors() {
    let summary = summarize_case(
        &["mypy", "src"],
        "src/server/auth.py:12: error: Incompatible return value type  [return-value]\nsrc/server/auth.py:13: note: Expected type \"int\"\nsrc/models/user.py:8: error: Name \"foo\" is not defined  [name-defined]\nFound 2 errors in 2 files\n",
        "",
        1,
    );

    assert_eq!(summary.summary, "mypy: 2 errors in 2 files");
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("Top codes:"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("src/server/auth.py (1 errors)"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("[return-value]"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("Expected type \"int\""))
    );
}
