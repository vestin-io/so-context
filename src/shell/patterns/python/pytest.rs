use regex::Regex;

use crate::shell::types::{CompressionSummary, ShellPattern, ShellResult};

use super::super::text::{non_empty_lines, preferred_output, preview, truncate_text};

const MAX_FAILURES: usize = 5;
const MAX_XFAIL: usize = 10;

#[derive(Default)]
struct PytestCounts {
    passed: usize,
    failed: usize,
    skipped: usize,
    xfailed: usize,
    xpassed: usize,
}

pub(super) fn summarize_pytest(result: &ShellResult) -> CompressionSummary {
    let output = preferred_output(result);
    let summary_line = find_summary_line(&output);
    let counts = summary_line
        .as_deref()
        .map(parse_summary_line)
        .unwrap_or_default();
    let failure_lines = collect_failure_lines(&output);
    let xfail_lines = collect_xfail_lines(&output);

    let summary = if counts.passed == 0
        && counts.failed == 0
        && counts.skipped == 0
        && counts.xfailed == 0
        && counts.xpassed == 0
    {
        if output.contains("no tests ran") || output.contains("collected 0 items") {
            "Pytest: No tests collected".to_string()
        } else if result.exit_code == 0 {
            "Pytest: passed".to_string()
        } else {
            "Pytest: failed".to_string()
        }
    } else if counts.failed == 0
        && counts.skipped == 0
        && counts.xfailed == 0
        && counts.xpassed == 0
    {
        format!("Pytest: {} passed", counts.passed)
    } else {
        let mut summary = format!("Pytest: {} passed, {} failed", counts.passed, counts.failed);
        if counts.skipped > 0 {
            summary.push_str(&format!(", {} skipped", counts.skipped));
        }
        if counts.xfailed > 0 {
            summary.push_str(&format!(", {} xfailed", counts.xfailed));
        }
        if counts.xpassed > 0 {
            summary.push_str(&format!(", {} xpassed", counts.xpassed));
        }
        summary
    };

    let mut details = Vec::new();
    if !xfail_lines.is_empty() {
        details.push("Expected-failure outcomes:".to_string());
        for line in xfail_lines.iter().take(MAX_XFAIL) {
            details.push(format!("  {}", truncate_text(line, 140)));
        }
        if xfail_lines.len() > MAX_XFAIL {
            details.push(format!("  ... +{} more", xfail_lines.len() - MAX_XFAIL));
        }
    }

    if !failure_lines.is_empty() {
        if !details.is_empty() {
            details.push(String::new());
        }
        details.push("Failures:".to_string());
        for (index, line) in failure_lines.iter().take(MAX_FAILURES).enumerate() {
            details.push(format!("{}. {}", index + 1, truncate_text(line, 140)));
        }
        if failure_lines.len() > MAX_FAILURES {
            details.push(format!(
                "... +{} more failures",
                failure_lines.len() - MAX_FAILURES
            ));
        }
    }

    CompressionSummary::plain(
        ShellPattern::PythonPytest,
        summary,
        details,
        preview(&result.stderr),
        result,
    )
}

fn find_summary_line(output: &str) -> Option<String> {
    output
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| is_summary_line(line))
        .map(|line| line.to_string())
}

fn is_summary_line(line: &str) -> bool {
    (line.contains(" passed")
        || line.contains(" failed")
        || line.contains(" skipped")
        || line.contains(" xfailed")
        || line.contains(" xpassed"))
        && line.contains(" in ")
}

fn parse_summary_line(summary: &str) -> PytestCounts {
    let mut counts = PytestCounts::default();
    for part in summary.split(',') {
        let words = part.split_whitespace().collect::<Vec<_>>();
        for index in 1..words.len() {
            let Ok(value) = words[index - 1].parse::<usize>() else {
                continue;
            };
            let word = words[index];
            if word.contains("xpassed") {
                counts.xpassed = value;
            } else if word.contains("xfailed") {
                counts.xfailed = value;
            } else if word.contains("passed") {
                counts.passed = value;
            } else if word.contains("failed") {
                counts.failed = value;
            } else if word.contains("skipped") {
                counts.skipped = value;
            }
        }
    }
    counts
}

fn collect_xfail_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("XFAIL") || line.starts_with("XPASS"))
        .map(|line| line.to_string())
        .collect()
}

fn collect_failure_lines(output: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let mut current_name = None::<String>;
    let mut current_lines = Vec::new();
    let mut in_failures = false;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("=== FAILURES") {
            in_failures = true;
            continue;
        }
        if in_failures && trimmed.starts_with("=== short test summary") {
            if let Some(name) = current_name.take() {
                failures.push(format_failure(name, &current_lines));
            }
            current_lines.clear();
            in_failures = false;
            continue;
        }
        if in_failures {
            if is_failure_block_header(trimmed) {
                if let Some(name) = current_name.take() {
                    failures.push(format_failure(name, &current_lines));
                    current_lines.clear();
                }
                current_name = Some(trimmed.trim_matches('_').trim().to_string());
                continue;
            }
            if !trimmed.is_empty() {
                current_lines.push(trimmed.to_string());
            }
        }
    }

    if let Some(name) = current_name.take() {
        failures.push(format_failure(name, &current_lines));
    }

    if failures.is_empty() {
        failures.extend(
            non_empty_lines(output)
                .into_iter()
                .filter(|line| line.starts_with("FAILED ") || line.starts_with("ERROR "))
                .map(|line| line.trim().to_string()),
        );
    }

    failures
}

fn is_failure_block_header(line: &str) -> bool {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let regex =
        RE.get_or_init(|| Regex::new(r"^_+\s+.+\s+_+$").expect("valid pytest failure regex"));
    regex.is_match(line)
}

fn format_failure(name: String, lines: &[String]) -> String {
    let mut parts = vec![name];
    for line in lines.iter().filter(|line| {
        line.starts_with("E")
            || line.starts_with('>')
            || line.contains("assert")
            || line.contains(".py:")
            || line.contains("AssertionError")
    }) {
        parts.push(line.clone());
    }
    parts.join(" | ")
}
