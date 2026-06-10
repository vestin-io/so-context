use super::*;
use crate::shell::types::{CaptureMetadata, ShellInvocation};

fn summarize_case(result: &ShellResult) -> CompressionSummary {
    summarize(result, super::super::classify_only(result))
}

#[test]
fn summarizes_tsc() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["tsc".into(), "--noEmit".into()]),
        stdout: String::new(),
        stderr: "src/main.ts(12,4): error TS2322: Type 'number' is not assignable to type 'string'.\nsrc/app.ts(4,1): error TS2304: Cannot find name 'windowish'.\n".into(),
        exit_code: 2,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::Tsc);
    assert_eq!(summary.summary, "TypeScript: 2 errors in 2 files");
}

#[test]
fn summarizes_next_build() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["next".into(), "build".into()]),
        stdout: "✓ Compiled successfully\nRoute (app)                                Size\n├ ○ /                                       1.2 kB\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::NextBuild);
    assert_eq!(summary.summary, "✓ Compiled successfully");
}

#[test]
fn summarizes_vite_build() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["vite".into(), "build".into()]),
        stdout:
            "vite v6.0.0 building for production...\n✓ built in 420ms\ndist/index.js  12.3 kB\n"
                .into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::ViteBuild);
    assert_eq!(summary.summary, "✓ built in 420ms");
}

#[test]
fn summarizes_make() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["make".into(), "test".into()]),
        stdout: "Nothing to be done for `test'.\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::Make);
    assert_eq!(summary.summary, "Nothing to be done for `test'.");
}

#[test]
fn summarizes_gradle() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["./gradlew".into(), "build".into()]),
        stdout: "> Task :app:compileJava\nBUILD SUCCESSFUL in 4s\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::Gradle);
    assert_eq!(summary.summary, "BUILD SUCCESSFUL in 4s");
}

#[test]
fn summarizes_maven() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["mvn".into(), "test".into()]),
        stdout: "[INFO] BUILD SUCCESS\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::Maven);
    assert_eq!(summary.summary, "[INFO] BUILD SUCCESS");
}

#[test]
fn summarizes_dotnet_build() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["dotnet".into(), "build".into()]),
        stdout: "Build succeeded.\n    0 Warning(s)\n    0 Error(s)\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::DotnetBuild);
    assert_eq!(summary.summary, "Build succeeded.");
}

#[test]
fn summarizes_dotnet_test() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["dotnet".into(), "test".into()]),
        stdout: "Passed!  - Failed: 0, Passed: 24, Skipped: 0, Total: 24, Duration: 1 s\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::DotnetTest);
    assert_eq!(
        summary.summary,
        "Passed!  - Failed: 0, Passed: 24, Skipped: 0, Total: 24, Duration: 1 s"
    );
}

#[test]
fn summarizes_dotnet_restore() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["dotnet".into(), "restore".into()]),
        stdout: "Restore completed in 1.2 sec for src/app/app.csproj.\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::DotnetRestore);
    assert_eq!(
        summary.summary,
        "Restore completed in 1.2 sec for src/app/app.csproj."
    );
}

#[test]
fn summarizes_dotnet_format() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["dotnet".into(), "format".into()]),
        stdout: "No files were formatted.\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::DotnetFormat);
    assert_eq!(summary.summary, "No files were formatted.");
}

#[test]
fn summarizes_cmake() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cmake".into(), "--build".into(), "build".into()]),
        stdout: "-- Configuring done\n-- Generating done\n-- Build files have been written to: /tmp/build\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_case(&result);
    assert_eq!(summary.pattern, ShellPattern::Cmake);
    assert_eq!(summary.summary, "cmake: done");
}
