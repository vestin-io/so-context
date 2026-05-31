use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::argv::first_positional;
use super::text::{compact_whitespace, non_empty_lines, preview, sample_lines};

fn summarize(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    match pattern {
        ShellPattern::CargoTest | ShellPattern::CargoNextest => {
            summarize_test_like(result, pattern)
        }
        ShellPattern::CargoClippy => summarize_clippy(result),
        ShellPattern::CargoBuild | ShellPattern::CargoCheck | ShellPattern::CargoInstall => {
            summarize_build_like(result, pattern)
        }
        _ => CompressionSummary::new(
            pattern,
            compact_whitespace(&result.stdout),
            Vec::new(),
            preview(&result.stderr),
            result,
        ),
    }
}

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    if program != "cargo" {
        return None;
    }

    match first_positional(args, &["--config", "-Z"]) {
        Some("build") => Some(ShellPattern::CargoBuild),
        Some("test") => Some(ShellPattern::CargoTest),
        Some("clippy") => Some(ShellPattern::CargoClippy),
        Some("check") => Some(ShellPattern::CargoCheck),
        Some("install") => Some(ShellPattern::CargoInstall),
        Some("nextest") => Some(ShellPattern::CargoNextest),
        _ => None,
    }
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    matches!(
        pattern,
        ShellPattern::CargoBuild
            | ShellPattern::CargoTest
            | ShellPattern::CargoClippy
            | ShellPattern::CargoCheck
            | ShellPattern::CargoInstall
            | ShellPattern::CargoNextest
    )
    .then(|| summarize(result, pattern))
}

fn summarize_test_like(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let output = format!("{}\n{}", result.stdout, result.stderr);
    let lines = non_empty_lines(&output);
    let label = if pattern == ShellPattern::CargoNextest {
        "cargo nextest"
    } else {
        "cargo test"
    };
    let summary = if let Some(result) = aggregate_test_results(&lines) {
        format!("{label}: {}", result.summary())
    } else {
        let errors = count_error_lines(&lines);
        let warnings = count_warning_lines(&lines);
        format!("{label}: {errors} errors, {warnings} warnings")
    };
    let details = collect_failure_names(&lines);
    let stderr_preview = test_stderr_preview(result, &lines, &details);

    CompressionSummary::new(pattern, summary, details, stderr_preview, result)
}

fn summarize_clippy(result: &ShellResult) -> CompressionSummary {
    let output = format!("{}\n{}", result.stdout, result.stderr);
    let lines = non_empty_lines(&output);
    let errors = count_error_lines(&lines);
    let warnings = count_warning_lines(&lines);
    let summary = if errors == 0 && warnings == 0 && result.exit_code == 0 {
        "cargo clippy: No issues found".to_string()
    } else {
        format!("cargo clippy: {errors} errors, {warnings} warnings")
    };

    CompressionSummary::new(
        ShellPattern::CargoClippy,
        summary,
        build_like_details(&lines, errors, warnings),
        build_like_stderr_preview(result, &lines, errors, warnings),
        result,
    )
}

fn summarize_build_like(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let output = format!("{}\n{}", result.stdout, result.stderr);
    let lines = non_empty_lines(&output);
    let errors = count_error_lines(&lines);
    let warnings = count_warning_lines(&lines);
    let label = match pattern {
        ShellPattern::CargoBuild => "cargo build",
        ShellPattern::CargoCheck => "cargo check",
        ShellPattern::CargoInstall => "cargo install",
        _ => "cargo",
    };
    let summary = if pattern == ShellPattern::CargoInstall
        && result.exit_code == 0
        && let Some(package) = installed_package_name(&lines)
    {
        format!("{label}: installed {package}")
    } else if errors == 0 && warnings == 0 && result.exit_code == 0 {
        format!("{label}: ok")
    } else {
        format!("{label}: {errors} errors, {warnings} warnings")
    };

    CompressionSummary::new(
        pattern,
        summary,
        build_like_details(&lines, errors, warnings),
        build_like_stderr_preview(result, &lines, errors, warnings),
        result,
    )
}

fn installed_package_name(lines: &[String]) -> Option<String> {
    lines.iter().find_map(|line| {
        let remainder = line.strip_prefix("Installed package `")?;
        let package = remainder.split('`').next()?.trim();
        let package = package.split(" (").next().unwrap_or(package).trim();
        Some(package.to_string())
    })
}

#[derive(Default)]
struct TestRunTotals {
    passed: usize,
    failed: usize,
    ignored: usize,
    measured: usize,
    filtered_out: usize,
    finished_in: Option<String>,
    finished_in_seconds: Option<f64>,
}

impl TestRunTotals {
    fn update_finished_in(&mut self, value: &str) {
        let seconds = parse_duration_seconds(value);
        match (self.finished_in_seconds, seconds) {
            (Some(current), Some(candidate)) if current >= candidate => return,
            (Some(_), None) => return,
            _ => {}
        }
        self.finished_in = Some(value.to_string());
        self.finished_in_seconds = seconds;
    }

    fn summary(&self) -> String {
        let mut summary = format!(
            "{} passed; {} failed; {} ignored; {} measured; {} filtered out",
            self.passed, self.failed, self.ignored, self.measured, self.filtered_out
        );
        if let Some(duration) = &self.finished_in {
            summary.push_str("; finished in ");
            summary.push_str(duration);
        }
        summary
    }
}

fn aggregate_test_results(lines: &[String]) -> Option<TestRunTotals> {
    let mut totals = TestRunTotals::default();
    let mut found_any = false;

    for line in lines
        .iter()
        .filter(|line| line.starts_with("test result:"))
        .map(String::as_str)
    {
        let Some(parsed) = parse_test_result_line(line) else {
            continue;
        };
        found_any = true;
        totals.passed += parsed.passed;
        totals.failed += parsed.failed;
        totals.ignored += parsed.ignored;
        totals.measured += parsed.measured;
        totals.filtered_out += parsed.filtered_out;
        if let Some(duration) = parsed.finished_in.as_deref() {
            totals.update_finished_in(duration);
        }
    }

    found_any.then_some(totals)
}

fn parse_test_result_line(line: &str) -> Option<TestRunTotals> {
    let mut totals = TestRunTotals::default();
    let normalized = line
        .trim_start_matches("test result:")
        .trim()
        .trim_end_matches('.')
        .replace("ok. ", "")
        .replace("FAILED. ", "");

    for field in normalized.split(';').map(str::trim) {
        if let Some(value) = field.strip_suffix(" passed") {
            totals.passed = value.parse().ok()?;
        } else if let Some(value) = field.strip_suffix(" failed") {
            totals.failed = value.parse().ok()?;
        } else if let Some(value) = field.strip_suffix(" ignored") {
            totals.ignored = value.parse().ok()?;
        } else if let Some(value) = field.strip_suffix(" measured") {
            totals.measured = value.parse().ok()?;
        } else if let Some(value) = field.strip_suffix(" filtered out") {
            totals.filtered_out = value.parse().ok()?;
        } else if let Some(value) = field.strip_prefix("finished in ") {
            totals.update_finished_in(value);
        }
    }

    Some(totals)
}

fn parse_duration_seconds(value: &str) -> Option<f64> {
    value.strip_suffix('s')?.parse().ok()
}

fn collect_failure_names(lines: &[String]) -> Vec<String> {
    let failures: Vec<String> = lines
        .iter()
        .filter_map(|line| line.strip_prefix("---- "))
        .map(|line| line.trim_end_matches(" stdout ----").to_string())
        .collect();
    sample_lines(failures, 6)
}

fn test_stderr_preview(
    result: &ShellResult,
    lines: &[String],
    failure_names: &[String],
) -> Vec<String> {
    if result.exit_code == 0 && failure_names.is_empty() {
        return Vec::new();
    }

    let actionable = filter_actionable_lines(lines);
    if actionable.is_empty() {
        preview(&result.stderr)
    } else {
        sample_lines(actionable, 6)
    }
}

fn build_like_details(lines: &[String], errors: usize, warnings: usize) -> Vec<String> {
    if errors == 0 && warnings == 0 {
        lines
            .iter()
            .find(|line| line.contains("Finished"))
            .cloned()
            .into_iter()
            .collect()
    } else {
        sample_lines(filter_actionable_lines(lines), 6)
    }
}

fn build_like_stderr_preview(
    result: &ShellResult,
    lines: &[String],
    errors: usize,
    warnings: usize,
) -> Vec<String> {
    if result.exit_code == 0 && errors == 0 && warnings == 0 {
        return Vec::new();
    }

    let actionable: Vec<String> = lines
        .iter()
        .filter(|line| line.starts_with("error") || line.starts_with("warning"))
        .cloned()
        .collect();
    if actionable.is_empty() {
        preview(&result.stderr)
    } else {
        sample_lines(actionable, 6)
    }
}

fn filter_actionable_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| {
            line.starts_with("error")
                || line.starts_with("warning")
                || line.contains("Finished")
                || line.contains("Installed package")
        })
        .cloned()
        .collect()
}

fn count_error_lines(lines: &[String]) -> usize {
    lines
        .iter()
        .filter(|line| line.starts_with("error") || line.contains(": error["))
        .count()
}

fn count_warning_lines(lines: &[String]) -> usize {
    lines
        .iter()
        .filter(|line| line.starts_with("warning") || line.contains(": warning["))
        .count()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
