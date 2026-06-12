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
fn classifies_wrapped_eslint() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["npx".into(), "eslint".into(), ".".into()]),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    assert_eq!(
        super::super::classify_only(&result),
        ShellPattern::LintEslint
    );
}

#[test]
fn classifies_go_tool_golangci_lint() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "go".into(),
            "tool".into(),
            "golangci-lint".into(),
            "run".into(),
            "./...".into(),
        ]),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    assert_eq!(
        super::super::classify_only(&result),
        ShellPattern::LintGolangci
    );
}

#[test]
fn classifies_go_tool_golangci_lint_with_flags() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "go".into(),
            "-C".into(),
            "tools".into(),
            "tool".into(),
            "-modfile".into(),
            "tools.mod".into(),
            "golangci-lint".into(),
            "run".into(),
            "./...".into(),
        ]),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    assert_eq!(
        super::super::classify_only(&result),
        ShellPattern::LintGolangci
    );
}

#[test]
fn summarizes_eslint_problems() {
    let summary = summarize_case(
        &["eslint", "."],
        "/tmp/app.js:1:7: 'x' is assigned a value but never used. [Error/no-unused-vars]\n✖ 1 problem (1 error, 0 warnings)\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::LintEslint);
    assert_eq!(summary.summary, "ESLint: 1 error, 0 warnings in 1 file");
    assert_eq!(summary.details[0], "Top rules:");
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("no-unused-vars"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("Top files:"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("app.js (1 issues)"))
    );
}

#[test]
fn summarizes_prettier_check() {
    let summary = summarize_case(
        &["prettier", "--check", "."],
        "[warn] src/app.ts\n[warn] Code style issues found in the above file. Run Prettier with --write to fix.\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::LintPrettier);
    assert_eq!(summary.summary, "Prettier: 1 file needs formatting");
    assert_eq!(summary.details[0], "src/app.ts");
}

#[test]
fn prettier_check_ignores_non_path_warn_lines() {
    let summary = summarize_case(
        &["prettier", "--check", "."],
        "[warn] Code style issues found in the above file. Run Prettier with --write to fix.\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::LintPrettier);
    assert_eq!(
        summary.summary,
        "Code style issues found in the above file. Run Prettier with --write to fix."
    );
    assert!(summary.details.is_empty());
}

#[test]
fn summarizes_prettier_write_mode() {
    let summary = summarize_case(
        &["prettier", "--write", "."],
        "src/app.ts\nsrc/lib.ts\n",
        "",
        0,
    );

    assert_eq!(summary.pattern, ShellPattern::LintPrettier);
    assert_eq!(summary.summary, "Prettier: 2 files formatted");
    assert!(summary.details.iter().any(|line| line == "src/app.ts"));
    assert!(summary.details.iter().any(|line| line == "src/lib.ts"));
}

#[test]
fn prettier_empty_output_is_not_treated_as_success() {
    let summary = summarize_case(&["prettier", "--check", "."], "", "", 0);

    assert_eq!(summary.pattern, ShellPattern::LintPrettier);
    assert_eq!(summary.summary, "Prettier: no output");
}

#[test]
fn summarizes_golangci_lint_success() {
    let summary = summarize_case(&["golangci-lint", "run"], "", "", 0);

    assert_eq!(summary.pattern, ShellPattern::LintGolangci);
    assert_eq!(summary.summary, "golangci-lint: No issues found");
}

#[test]
fn summarizes_biome_issues() {
    let summary = summarize_case(
        &["biome", "check", "."],
        "src/app.ts:5:7 lint/correctness/noUnusedVariables  This variable is unused.\nsrc/lib.ts:2:1 lint/style/noNamespace  Avoid namespaces.\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::LintBiome);
    assert_eq!(summary.summary, "Biome: 2 issues in 2 files");
    assert_eq!(summary.details[0], "Top rules:");
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("lint/correctness/noUnusedVariables"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("Top files:"))
    );
}

#[test]
fn summarizes_golangci_lint_with_navigation_details() {
    let summary = summarize_case(
        &["golangci-lint", "run"],
        "internal/api/server.go:12:3: Error return value not checked (errcheck)\ninternal/api/server.go:24:7: S1000 should use plain channel send (gosimple)\ncmd/app/main.go:8:1: exported function Main should have comment (revive)\n",
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::LintGolangci);
    assert_eq!(summary.summary, "golangci-lint: 3 issues in 2 files");
    assert_eq!(summary.details[0], "Top linters:");
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("errcheck (1x)"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("Top files:"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("internal/api/server.go (2 issues)"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("-> Error return value not checked"))
    );
}

#[test]
fn summarizes_golangci_json_with_source_lines() {
    let summary = summarize_case(
        &["golangci-lint", "run", "--output.json.path", "stdout"],
        r#"{"Issues":[{"FromLinter":"errcheck","Text":"Error return value not checked","Severity":"error","SourceLines":["    if err := foo(); err != nil {"],"Pos":{"Filename":"internal/api/server.go","Line":12,"Column":3,"Offset":42}},{"FromLinter":"gosimple","Text":"should use plain channel send","SourceLines":[],"Pos":{"Filename":"cmd/app/main.go","Line":8,"Column":1,"Offset":7}}]}"#,
        "",
        1,
    );

    assert_eq!(summary.pattern, ShellPattern::LintGolangci);
    assert_eq!(summary.summary, "golangci-lint: 2 issues in 2 files");
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("errcheck (1x)"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("internal/api/server.go (1 issues)"))
    );
    assert!(
        summary
            .details
            .iter()
            .any(|line| line.contains("if err := foo(); err != nil {"))
    );
}

#[test]
fn classifies_run_wrapped_prettier() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "pnpm".into(),
            "run".into(),
            "prettier".into(),
            "--check".into(),
            ".".into(),
        ]),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    assert_eq!(
        super::super::classify_only(&result),
        ShellPattern::LintPrettier
    );
}
