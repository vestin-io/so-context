use std::collections::BTreeSet;

use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::argv::first_positional;
use super::text::{compact_whitespace, non_empty_lines, preview, sample_lines, truncate_text};

fn summarize(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    match pattern {
        ShellPattern::Tsc => summarize_tsc(result),
        ShellPattern::NextBuild | ShellPattern::ViteBuild => summarize_web_build(result, pattern),
        ShellPattern::Make => summarize_make(result),
        ShellPattern::Gradle | ShellPattern::Maven | ShellPattern::Cmake => {
            summarize_build_tool(result, pattern)
        }
        ShellPattern::DotnetBuild
        | ShellPattern::DotnetTest
        | ShellPattern::DotnetRestore
        | ShellPattern::DotnetFormat => summarize_dotnet(result, pattern),
        _ => CompressionSummary::new(
            pattern,
            truncate_text(&compact_whitespace(&result.stdout), 120),
            Vec::new(),
            preview(&result.stderr),
            result,
        ),
    }
}

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    match program {
        "tsc" => Some(ShellPattern::Tsc),
        "next" if first_positional(args, &[]).is_some_and(|arg| arg == "build") => {
            Some(ShellPattern::NextBuild)
        }
        "vite" if first_positional(args, &[]).is_some_and(|arg| arg == "build") => {
            Some(ShellPattern::ViteBuild)
        }
        "make" => Some(ShellPattern::Make),
        "gradle" | "gradlew" | "./gradlew" => Some(ShellPattern::Gradle),
        "mvn" | "mvnw" | "./mvnw" => Some(ShellPattern::Maven),
        "cmake" => Some(ShellPattern::Cmake),
        "dotnet" => {
            match first_positional(args, &["--configuration", "--framework", "--runtime"]) {
                Some("build") => Some(ShellPattern::DotnetBuild),
                Some("test") => Some(ShellPattern::DotnetTest),
                Some("restore") => Some(ShellPattern::DotnetRestore),
                Some("format") => Some(ShellPattern::DotnetFormat),
                _ => None,
            }
        }
        _ => None,
    }
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    matches!(
        pattern,
        ShellPattern::Tsc
            | ShellPattern::NextBuild
            | ShellPattern::ViteBuild
            | ShellPattern::Make
            | ShellPattern::Gradle
            | ShellPattern::Maven
            | ShellPattern::DotnetBuild
            | ShellPattern::DotnetTest
            | ShellPattern::DotnetRestore
            | ShellPattern::DotnetFormat
            | ShellPattern::Cmake
    )
    .then(|| summarize(result, pattern))
}

fn summarize_tsc(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&format!("{}\n{}", result.stdout, result.stderr));
    let errors: Vec<String> = lines
        .iter()
        .filter(|line| line.contains(" error TS"))
        .map(|line| compact_whitespace(line))
        .collect();
    let files: BTreeSet<String> = errors
        .iter()
        .filter_map(|line| line.split('(').next().map(str::to_string))
        .collect();
    let summary = if errors.is_empty() {
        "TypeScript: No errors found".to_string()
    } else {
        format!(
            "TypeScript: {} errors in {} files",
            errors.len(),
            files.len()
        )
    };

    CompressionSummary::new(
        ShellPattern::Tsc,
        summary,
        sample_lines(errors, 8),
        preview(&result.stderr),
        result,
    )
}

fn summarize_web_build(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let lines = non_empty_lines(&format!("{}\n{}", result.stdout, result.stderr));
    let summary = lines
        .iter()
        .find(|line| line.contains("built in") || line.contains("Compiled successfully"))
        .map(|line| truncate_text(line, 120))
        .unwrap_or_else(|| {
            if pattern == ShellPattern::NextBuild {
                "Next build: complete".to_string()
            } else {
                "Vite build: complete".to_string()
            }
        });
    let details = lines
        .iter()
        .filter(|line| {
            line.starts_with("dist/")
                || line.starts_with("Route")
                || line.starts_with("├")
                || line.starts_with("└")
        })
        .take(8)
        .cloned()
        .collect();

    CompressionSummary::new(pattern, summary, details, preview(&result.stderr), result)
}

fn summarize_make(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&format!("{}\n{}", result.stdout, result.stderr));
    let summary = lines
        .iter()
        .find(|line| line.contains("Nothing to be done") || line.contains("Error"))
        .cloned()
        .unwrap_or_else(|| "make: ok".to_string());

    CompressionSummary::new(
        ShellPattern::Make,
        summary,
        sample_lines(lines, 6),
        preview(&result.stderr),
        result,
    )
}

fn summarize_build_tool(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let lines = non_empty_lines(&format!("{}\n{}", result.stdout, result.stderr));
    let summary = lines
        .iter()
        .find(|line| line.contains("BUILD SUCCESSFUL") || line.contains("BUILD FAILED"))
        .cloned()
        .or_else(|| {
            lines
                .iter()
                .find(|line| line.contains("SUCCESS") || line.contains("FAILURE"))
                .cloned()
        })
        .unwrap_or_else(|| format!("{}: done", tool_label(pattern)));

    CompressionSummary::new(
        pattern,
        summary,
        sample_lines(
            lines.into_iter().filter(|line| {
                line.starts_with("> Task") || line.contains("ERROR") || line.contains("FAILURE")
            }),
            8,
        ),
        preview(&result.stderr),
        result,
    )
}

fn summarize_dotnet(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let lines = non_empty_lines(&format!("{}\n{}", result.stdout, result.stderr));
    let summary = lines
        .iter()
        .find(|line| {
            line.contains("Build succeeded")
                || line.contains("Build FAILED")
                || line.contains("Passed!")
                || line.contains("Restore completed")
                || line.contains("No files were formatted")
        })
        .cloned()
        .unwrap_or_else(|| format!("{}: done", tool_label(pattern)));

    CompressionSummary::new(
        pattern,
        summary,
        sample_lines(
            lines.into_iter().filter(|line| {
                line.starts_with("error")
                    || line.starts_with("warning")
                    || line.contains("Passed!")
                    || line.contains("Failed!")
            }),
            8,
        ),
        preview(&result.stderr),
        result,
    )
}

fn tool_label(pattern: ShellPattern) -> &'static str {
    match pattern {
        ShellPattern::Gradle => "gradle",
        ShellPattern::Maven => "maven",
        ShellPattern::Cmake => "cmake",
        ShellPattern::DotnetBuild => "dotnet build",
        ShellPattern::DotnetTest => "dotnet test",
        ShellPattern::DotnetRestore => "dotnet restore",
        ShellPattern::DotnetFormat => "dotnet format",
        _ => "build",
    }
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
