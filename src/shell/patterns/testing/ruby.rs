use regex::Regex;

use super::super::super::types::ShellResult;
use super::super::text::{non_empty_lines, preferred_output};
use super::models::RspecOutput;
use super::parsing::{
    extract_json_object, format_test_totals, numbered_failure_marker, parse_duration_ms, strip_ansi,
};
use super::presentation::{FailureRecord, TestPresentation};

pub(super) fn summarize_rspec(result: &ShellResult) -> TestPresentation {
    let output = strip_rspec_noise(&strip_ansi(&preferred_output(result)));
    if let Some(json) = extract_json_object(&output)
        && let Ok(parsed) = serde_json::from_str::<RspecOutput>(json)
    {
        if parsed.summary.example_count == 0 && parsed.summary.errors_outside_of_examples_count == 0
        {
            return TestPresentation {
                summary: "RSpec: no examples found".to_string(),
                failures: Vec::new(),
                duration_ms: Some((parsed.summary.duration * 1000.0) as u64),
            };
        }

        if parsed.summary.example_count == 0 && parsed.summary.errors_outside_of_examples_count > 0
        {
            return TestPresentation {
                summary: format_test_totals(0, parsed.summary.errors_outside_of_examples_count, 0),
                failures: vec![FailureRecord {
                    name: "RSpec errors outside of examples".to_string(),
                    message_lines: vec![format!(
                        "{} loader/runtime {} prevented specs from running",
                        parsed.summary.errors_outside_of_examples_count,
                        noun(
                            parsed.summary.errors_outside_of_examples_count,
                            "error",
                            "errors"
                        )
                    )],
                }],
                duration_ms: Some((parsed.summary.duration * 1000.0) as u64),
            };
        }

        let failures = parsed
            .examples
            .iter()
            .filter(|example| example.status == "failed")
            .map(|example| {
                let mut message_lines =
                    vec![format!("{}:{}", example.file_path, example.line_number)];
                if let Some(exception) = &example.exception {
                    let short_class = exception
                        .class
                        .split("::")
                        .last()
                        .unwrap_or(&exception.class);
                    let first_line = exception.message.lines().next().unwrap_or_default();
                    message_lines.push(format!("{short_class}: {first_line}"));
                    if let Some(backtrace) = exception
                        .backtrace
                        .iter()
                        .map(|line| line.trim())
                        .find(|line| !is_gem_backtrace(line))
                    {
                        message_lines.push(backtrace.to_string());
                    }
                }
                FailureRecord {
                    name: example.full_description.clone(),
                    message_lines,
                }
            })
            .collect();
        let passed = parsed
            .summary
            .example_count
            .saturating_sub(parsed.summary.failure_count + parsed.summary.pending_count);

        return TestPresentation {
            summary: format_test_totals(
                passed,
                parsed.summary.failure_count,
                parsed.summary.pending_count,
            ),
            failures,
            duration_ms: Some((parsed.summary.duration * 1000.0) as u64),
        };
    }

    summarize_rspec_text(&output)
}

pub(super) fn summarize_minitest(result: &ShellResult) -> TestPresentation {
    let output = strip_ansi(&preferred_output(result));
    let lines = non_empty_lines(&output);
    let summary_re = Regex::new(
        r"(\d+)\s+runs,\s+\d+\s+assertions,\s+(\d+)\s+failures?,\s+(\d+)\s+errors?,\s+(\d+)\s+skips?",
    )
    .expect("valid minitest summary regex");
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;

    if let Some(caps) = summary_re.captures(&output) {
        let total = caps[1].parse::<usize>().unwrap_or(0);
        let failures_count = caps[2].parse::<usize>().unwrap_or(0);
        let errors_count = caps[3].parse::<usize>().unwrap_or(0);
        skipped = caps[4].parse::<usize>().unwrap_or(0);
        failed = failures_count + errors_count;
        passed = total.saturating_sub(failed + skipped);
    }

    let mut failures = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index].trim();
        if !numbered_failure_marker(line) {
            index += 1;
            continue;
        }

        let kind = if line.contains("Failure") {
            "Failure"
        } else if line.contains("Error") {
            "Error"
        } else {
            index += 1;
            continue;
        };
        let mut name = kind.to_string();
        let mut message_lines = Vec::new();
        if let Some(next) = lines.get(index + 1) {
            name = next.trim().to_string();
        }

        index += 2;
        while index < lines.len() {
            let next = lines[index].trim();
            if numbered_failure_marker(next) || summary_re.is_match(next) {
                break;
            }
            if !next.is_empty() {
                message_lines.push(next.to_string());
            }
            index += 1;
        }

        failures.push(FailureRecord {
            name,
            message_lines,
        });
    }

    TestPresentation {
        summary: if passed == 0 && failed == 0 && skipped == 0 {
            "Minitest: no tests found".to_string()
        } else {
            format_test_totals(passed, failed, skipped)
        },
        failures,
        duration_ms: parse_duration_ms(&output),
    }
}

fn summarize_rspec_text(output: &str) -> TestPresentation {
    let summary_re = Regex::new(r"(\d+)\s+examples?,\s+(\d+)\s+failures?(?:,\s+(\d+)\s+pending)?")
        .expect("valid rspec summary regex");
    let stripped = strip_rspec_noise(output);
    let lines = non_empty_lines(&stripped);
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;

    if let Some(caps) = summary_re.captures(&stripped) {
        let total = caps[1].parse::<usize>().unwrap_or(0);
        failed = caps[2].parse::<usize>().unwrap_or(0);
        skipped = caps
            .get(3)
            .and_then(|value| value.as_str().parse::<usize>().ok())
            .unwrap_or(0);
        passed = total.saturating_sub(failed + skipped);
    }

    let failures = collect_rspec_failures(&lines, |line| {
        summary_re.is_match(line) || line == "Failed examples:"
    });
    TestPresentation {
        summary: if passed == 0 && failed == 0 && skipped == 0 {
            "RSpec: no examples found".to_string()
        } else {
            format_test_totals(passed, failed, skipped)
        },
        failures,
        duration_ms: parse_duration_ms(&stripped),
    }
}

fn collect_rspec_failures(
    lines: &[String],
    should_stop: impl Fn(&str) -> bool,
) -> Vec<FailureRecord> {
    let mut failures = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index].trim();
        if !numbered_failure_marker(line) {
            index += 1;
            continue;
        }

        let name = line
            .split_once(')')
            .map(|(_, rest)| rest.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| line.to_string());
        let mut message_lines = Vec::new();
        index += 1;

        while index < lines.len() {
            let next = lines[index].trim();
            if numbered_failure_marker(next) || should_stop(next) {
                break;
            }
            if !next.is_empty() {
                message_lines.push(next.to_string());
            }
            index += 1;
        }

        failures.push(FailureRecord {
            name,
            message_lines: sanitize_rspec_failure_lines(&message_lines),
        });
    }

    failures
}

fn sanitize_rspec_failure_lines(lines: &[String]) -> Vec<String> {
    let mut cleaned = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("rspec ") || is_gem_backtrace(trimmed) {
            continue;
        }
        if let Some(path) = screenshot_path(trimmed) {
            cleaned.push(format!("[screenshot: {path}]"));
            continue;
        }
        cleaned.push(trimmed.to_string());
    }
    cleaned
}

fn strip_rspec_noise(output: &str) -> String {
    let mut cleaned = Vec::new();
    let mut in_simplecov_block = false;

    for line in output.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();

        if lower.contains("running via spring preloader") {
            continue;
        }
        if trimmed.starts_with("DEPRECATION WARNING:") || trimmed.starts_with("Finished in ") {
            continue;
        }
        if is_simplecov_line(&lower) {
            in_simplecov_block = true;
            continue;
        }
        if in_simplecov_block {
            if trimmed.is_empty() {
                in_simplecov_block = false;
            }
            continue;
        }
        if let Some(path) = screenshot_path(trimmed) {
            cleaned.push(format!("[screenshot: {path}]"));
            continue;
        }

        cleaned.push(line.to_string());
    }

    cleaned.join("\n")
}

fn is_simplecov_line(line: &str) -> bool {
    line.contains("coverage report")
        || line.contains("simplecov")
        || line.contains(".simplecov")
        || line.contains("coverage/")
        || (line.contains("all files") && line.contains("lines"))
}

fn screenshot_path(line: &str) -> Option<&str> {
    let (_, path) = line.split_once("saved screenshot to ")?;
    Some(path.trim())
}

fn is_gem_backtrace(line: &str) -> bool {
    line.contains("/gems/")
        || line.contains("lib/rspec")
        || line.contains("vendor/bundle")
        || line.contains("lib/ruby/")
}

fn noun<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}
