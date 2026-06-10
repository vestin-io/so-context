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
fn summarizes_vitest_json_failures() {
    let summary = summarize_case(
        &["vitest", "run"],
        r#"{"numTotalTests":5,"numPassedTests":4,"numFailedTests":1,"numPendingTests":0,"testResults":[{"name":"src/auth.test.ts","assertionResults":[{"fullName":"auth login works","status":"failed","failureMessages":["AssertionError: expected 200 to be 500\nExpected: 200\nReceived: 500\nCall log:\n  - POST /login"]}]}]}"#,
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::Vitest);
    assert_eq!(summary.summary, "PASS (4) FAIL (1)");
    assert_eq!(summary.details[0], "1. auth login works");
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("Call log:")));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("- POST /login")));
}

#[test]
fn summarizes_playwright_json_failures() {
    let summary = summarize_case(
        &["playwright", "test"],
        r#"{"stats":{"expected":2,"unexpected":1,"skipped":0,"duration":3519.7},"suites":[{"title":"tests/auth.spec.ts","specs":[{"title":"login works","ok":false,"tests":[{"status":"unexpected","results":[{"status":"failed","errors":[{"message":"Expected true to be false"},{"message":"Call log:\n  - waiting for locator('text=Login')"}]}]}]}],"suites":[]}]} "#,
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::PlaywrightTest);
    assert_eq!(summary.summary, "PASS (2) FAIL (1)");
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("login works")));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("Expected true to be false")));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("waiting for locator")));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("Time: 3519ms")));
}

#[test]
fn summarizes_playwright_text_failures() {
    let summary = summarize_case(
        &["playwright", "test"],
        "Running 3 tests using 1 worker\n\n  ✘  1 [chromium] › tests/auth.spec.ts:3:1 › login works (5.0s)\n    Error: expect(received).toContain(expected)\n    Expected substring: \"Login\"\n    Received string: \"Sign in\"\n      12 | await page.goto('/login')\n    > 13 | await expect(page.getByRole('button')).toContainText('Login')\n         |                                        ^\n      14 |\n    Call log:\n      - waiting for locator('text=Login')\n    attachment #1: screenshot (image/png)\n    /tmp/playwright/auth-failure.png\n\n  2 passed\n  1 failed\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::PlaywrightTest);
    assert_eq!(summary.summary, "PASS (2) FAIL (1)");
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("tests/auth.spec.ts > login works")));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("expect(received).toContain(expected)")));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("Expected substring: \"Login\"")));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("Received string: \"Sign in\"")));
    assert!(summary.details.iter().any(|line| {
        line.contains("> 13 | await expect(page.getByRole('button')).toContainText('Login')")
    }));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("waiting for locator('text=Login')")));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("/tmp/playwright/auth-failure.png")));
}

#[test]
fn summarizes_go_test_text_failures() {
    let summary = summarize_case(
        &["go", "test", "./..."],
        "--- FAIL: TestHealth (0.00s)\n    api_test.go:14: expected 200 got 500\nFAIL\nFAIL github.com/example/app 0.123s\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::GoTest);
    assert_eq!(summary.summary, "PASS (0) FAIL (1)");
    assert_eq!(summary.details[0], "1. TestHealth");
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("Time: 123ms")));
}

#[test]
fn summarizes_rspec_text_failures() {
    let summary = summarize_case(
        &["rspec"],
        "Running via Spring preloader in process 12345\n1) Auth login works\nFailure/Error: expect(response.status).to eq(200)\nexpected: 200\n     got: 500\n# /usr/local/lib/ruby/gems/3.2.0/gems/rspec-expectations-3.12.0/lib/rspec/expectations/fail_with.rb:37\n# ./spec/requests/auth_spec.rb:12\nsaved screenshot to /tmp/capybara/auth-failure.png\n\nCoverage report generated for RSpec to /app/coverage.\n142 / 200 LOC (71.0%) covered.\n\n2 examples, 1 failures\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::Rspec);
    assert_eq!(summary.summary, "PASS (1) FAIL (1)");
    assert_eq!(summary.details[0], "1. Auth login works");
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("Failure/Error: expect(response.status).to eq(200)")));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("# ./spec/requests/auth_spec.rb:12")));
    assert!(!summary
        .details
        .iter()
        .any(|line| line.contains("gems/rspec-expectations")));
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("[screenshot: /tmp/capybara/auth-failure.png]")));
}

#[test]
fn summarizes_minitest_failures() {
    let summary = summarize_case(
        &["ruby", "-Itest", "test/models/user_test.rb"],
        "  1) Failure:\nUserTest#test_valid [test/models/user_test.rb:12]:\nExpected false to be truthy.\n\n3 runs, 5 assertions, 1 failures, 0 errors, 0 skips\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::Minitest);
    assert_eq!(summary.summary, "PASS (2) FAIL (1)");
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("UserTest#test_valid")));
}

#[test]
fn classifies_wrapped_test_tools() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["npx".into(), "vitest".into(), "run".into()]),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };
    assert_eq!(super::super::classify_only(&result), ShellPattern::Vitest);

    let bundle = ShellResult {
        invocation: ShellInvocation::new(vec!["bundle".into(), "exec".into(), "rspec".into()]),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };
    assert_eq!(super::super::classify_only(&bundle), ShellPattern::Rspec);
}

#[test]
fn classifies_bundle_exec_ruby_minitest() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "bundle".into(),
            "exec".into(),
            "ruby".into(),
            "-Itest".into(),
            "test/models/user_test.rb".into(),
        ]),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    assert_eq!(super::super::classify_only(&result), ShellPattern::Minitest);
}

#[test]
fn summarizes_rspec_errors_outside_examples() {
    let summary = summarize_case(
        &["rspec"],
        r#"{"examples":[],"summary":{"duration":0.02,"example_count":0,"failure_count":0,"pending_count":0,"errors_outside_of_examples_count":1}}"#,
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::Rspec);
    assert_eq!(summary.summary, "PASS (0) FAIL (1)");
    assert_eq!(summary.details[0], "1. RSpec errors outside of examples");
    assert!(summary
        .details
        .iter()
        .any(|line| line.contains("loader/runtime error prevented specs from running")));
}
