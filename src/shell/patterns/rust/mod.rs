mod diagnostics;

use self::diagnostics::{
    build_like_details, build_like_stderr_preview, count_error_lines, count_warning_lines,
    filter_test_actionable_lines,
};
use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::argv::first_positional;
use super::text::append_omitted_line;
use super::text::{compact_whitespace, non_empty_lines, preview, sample_lines};

const SUCCESS_TEST_NAME_LIMIT: usize = 20;
const COMPILE_ERROR_BLOCK_LIMIT: usize = 15;

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
    let raw_lines = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    let label = if pattern == ShellPattern::CargoNextest {
        "cargo nextest"
    } else {
        "cargo test"
    };
    let compile_error_blocks = collect_compile_error_blocks(&raw_lines);
    let has_compile_errors = !compile_error_blocks.is_empty();
    let summary = if let Some(result) = aggregate_test_results(&lines) {
        format!("{label}: {}", result.summary())
    } else if has_compile_errors {
        compile_error_summary(label, &lines, &raw_lines, compile_error_blocks.len())
    } else {
        let errors = count_error_lines(&lines);
        let warnings = count_warning_lines(&lines);
        format!("{label}: {errors} errors, {warnings} warnings")
    };
    let details = collect_test_details(result, &lines, &compile_error_blocks);
    let stderr_preview = test_stderr_preview(result, &lines, &compile_error_blocks, &details);

    if has_compile_errors {
        CompressionSummary::plain(pattern, summary, details, stderr_preview, result)
    } else {
        CompressionSummary::new(pattern, summary, details, stderr_preview, result)
    }
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

fn collect_test_details(
    result: &ShellResult,
    lines: &[String],
    compile_error_blocks: &[String],
) -> Vec<String> {
    if result.exit_code == 0 {
        let success_names = collect_success_test_names(lines);
        if !success_names.is_empty() && success_names.len() <= SUCCESS_TEST_NAME_LIMIT {
            return success_names;
        }
    }

    let failure_names = collect_failure_names(lines);
    if !failure_names.is_empty() {
        return failure_names;
    }

    if !compile_error_blocks.is_empty() {
        let shown = compile_error_blocks.len().min(COMPILE_ERROR_BLOCK_LIMIT);
        let mut details = sample_lines(compile_error_blocks.iter().cloned(), shown);
        append_omitted_line(&mut details, compile_error_blocks.len(), shown, "issues");
        return details;
    }

    sample_lines(collect_error_blocks(lines), 6)
}

fn collect_success_test_names(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| {
            let rest = line.strip_prefix("test ")?;
            let (name, status) = rest.rsplit_once(" ... ")?;
            (status == "ok").then(|| name.to_string())
        })
        .collect()
}

fn collect_error_blocks(lines: &[String]) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = &lines[index];
        if !(line.starts_with("error[") || line.starts_with("error:")) {
            index += 1;
            continue;
        }

        let mut block = vec![line.clone()];
        let mut lookahead = index + 1;
        while lookahead < lines.len() {
            let candidate = &lines[lookahead];
            if candidate.starts_with("error[")
                || candidate.starts_with("error:")
                || candidate.starts_with("warning")
                || candidate.starts_with("test result:")
            {
                break;
            }

            if candidate.trim_start().starts_with("--> ")
                || candidate.contains("could not compile")
                || candidate.starts_with("For more information")
            {
                block.push(candidate.clone());
            }

            if candidate.contains("could not compile") {
                break;
            }
            lookahead += 1;
        }

        blocks.push(block.join(" | "));
        index = lookahead;
    }

    blocks
}

fn collect_compile_error_blocks(raw_lines: &[String]) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current_block = Vec::new();
    let mut in_block = false;

    for line in raw_lines {
        let trimmed = line.trim_start();

        if is_compile_error_start(trimmed) {
            if !current_block.is_empty() {
                blocks.push(current_block.join("\n"));
                current_block.clear();
            }
            in_block = true;
            current_block.push(trimmed.to_string());
            continue;
        }

        if !in_block {
            continue;
        }

        if is_compile_error_terminal(trimmed) {
            current_block.push(trimmed.to_string());
            blocks.push(current_block.join("\n"));
            current_block.clear();
            in_block = false;
            continue;
        }

        if is_compile_error_continuation(line, trimmed) {
            current_block.push(line.to_string());
            continue;
        }

        blocks.push(current_block.join("\n"));
        current_block.clear();
        in_block = false;
    }

    if !current_block.is_empty() {
        blocks.push(current_block.join("\n"));
    }

    blocks
}

fn compile_error_summary(
    label: &str,
    lines: &[String],
    raw_lines: &[String],
    error_count: usize,
) -> String {
    let warnings = count_warning_lines(lines);
    let compiled = count_compiling_crates(raw_lines);
    if compiled > 0 {
        format!("{label}: {error_count} errors, {warnings} warnings ({compiled} crates)")
    } else {
        format!("{label}: {error_count} errors, {warnings} warnings")
    }
}

fn count_compiling_crates(raw_lines: &[String]) -> usize {
    raw_lines
        .iter()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("Compiling") || trimmed.starts_with("Checking")
        })
        .count()
}

fn is_compile_error_start(line: &str) -> bool {
    line.starts_with("error[")
}

fn is_compile_error_terminal(line: &str) -> bool {
    line.contains("could not compile") || line.contains("aborting due to")
}

fn is_compile_error_continuation(raw_line: &str, trimmed: &str) -> bool {
    raw_line.starts_with(' ')
        || raw_line.starts_with('\t')
        || trimmed.starts_with("--> ")
        || trimmed.starts_with('|')
        || trimmed.starts_with("= note:")
        || trimmed.starts_with("= help:")
        || trimmed.starts_with("note:")
        || trimmed.starts_with("help:")
        || trimmed.starts_with("For more information")
}

fn test_stderr_preview(
    result: &ShellResult,
    lines: &[String],
    compile_error_blocks: &[String],
    details: &[String],
) -> Vec<String> {
    if result.exit_code == 0 && details.is_empty() {
        return Vec::new();
    }

    if !compile_error_blocks.is_empty() {
        return Vec::new();
    }

    let actionable = filter_test_actionable_lines(lines);
    if actionable.is_empty() {
        preview(&result.stderr)
    } else {
        sample_lines(actionable, 6)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
