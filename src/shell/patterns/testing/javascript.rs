use regex::Regex;

use super::super::super::types::ShellResult;
use super::super::text::{exact_non_empty_lines, non_empty_lines, preferred_output};
use super::common::{
    collect_block_failures, collect_message_lines, extract_json_object, find_keyword_counts,
    find_test_counts, format_failure_block, format_test_totals, parse_duration_ms, result_failed,
    strip_ansi, FailureRecord, TestPresentation,
};
use super::models::{JsTestJsonOutput, PlaywrightJsonOutput, PlaywrightSuite};

const PLAYWRIGHT_TEXT_LINE_LIMIT: usize = 12;

pub(super) fn summarize_js_test(result: &ShellResult, label: &str) -> TestPresentation {
    let output = preferred_output(result);
    if let Some(json) = extract_json_object(&output) {
        if let Ok(parsed) = serde_json::from_str::<JsTestJsonOutput>(json) {
            let failures = parsed
                .test_results
                .iter()
                .flat_map(|file| {
                    file.assertion_results
                        .iter()
                        .filter(|test| test.status == "failed")
                        .map(|test| FailureRecord {
                            name: test.full_name.clone(),
                            message_lines: collect_message_lines(&format_failure_block(
                                &file.name,
                                &test.failure_messages,
                            )),
                        })
                })
                .collect();

            return TestPresentation {
                summary: format_test_totals(
                    parsed.num_passed_tests,
                    parsed.num_failed_tests,
                    parsed.num_pending_tests,
                ),
                failures,
                duration_ms: None,
            };
        }
    }

    summarize_js_test_text(&strip_ansi(&output), label)
}

pub(super) fn summarize_playwright_test(result: &ShellResult) -> TestPresentation {
    let output = preferred_output(result);
    if let Some(json) = extract_json_object(&output) {
        if let Ok(parsed) = serde_json::from_str::<PlaywrightJsonOutput>(json) {
            let mut failures = Vec::new();
            collect_playwright_failures(&parsed.suites, &mut failures);
            return TestPresentation {
                summary: format_test_totals(
                    parsed.stats.expected,
                    parsed.stats.unexpected,
                    parsed.stats.skipped,
                ),
                failures,
                duration_ms: Some(parsed.stats.duration as u64),
            };
        }
    }

    summarize_playwright_text(&strip_ansi(&output))
}

fn summarize_js_test_text(output: &str, label: &str) -> TestPresentation {
    let lines = non_empty_lines(output);
    let patterns = [
        Regex::new(r"Tests?:\s*(?:(\d+)\s+failed,\s*)?(\d+)\s+passed(?:,\s*(\d+)\s+skipped)?")
            .expect("valid js test count regex"),
        Regex::new(
            r"Tests?\s+(?:(\d+)\s+failed\s+\|\s+)?(\d+)\s+passed(?:\s+\|\s+(\d+)\s+skipped)?",
        )
        .expect("valid js test count regex"),
    ];
    let (passed, failed, skipped) =
        find_test_counts(&lines, &patterns).unwrap_or((0, usize::from(result_failed(output)), 0));
    let failures = collect_block_failures(
        &lines,
        |line| {
            line.starts_with("FAIL ")
                || line.starts_with('×')
                || line.starts_with('✖')
                || line.starts_with('❯')
        },
        |line| {
            line.starts_with("Tests")
                || line.starts_with("Test Files")
                || line.starts_with("Snapshots")
                || line.starts_with("Duration")
                || line.starts_with("Time:")
        },
    );

    TestPresentation {
        summary: if passed == 0 && failed == 0 && skipped == 0 {
            format!("{label}: no tests found")
        } else {
            format_test_totals(passed, failed, skipped)
        },
        failures,
        duration_ms: parse_duration_ms(output),
    }
}

fn summarize_playwright_text(output: &str) -> TestPresentation {
    let counts = find_keyword_counts(output, &["passed", "failed", "skipped"]);
    let failures = extract_playwright_failures_regex(output);

    TestPresentation {
        summary: if counts.0 == 0 && counts.1 == 0 && counts.2 == 0 {
            "Playwright: no tests found".to_string()
        } else {
            format_test_totals(counts.0, counts.1, counts.2)
        },
        failures,
        duration_ms: parse_duration_ms(output),
    }
}

fn collect_playwright_failures(suites: &[PlaywrightSuite], failures: &mut Vec<FailureRecord>) {
    for suite in suites {
        let file_path = suite.file.clone().unwrap_or_else(|| suite.title.clone());
        for spec in &suite.specs {
            if spec.ok {
                continue;
            }

            let message_lines = spec
                .tests
                .iter()
                .filter(|execution| execution.status == "unexpected")
                .flat_map(|execution| {
                    execution.results.iter().flat_map(|attempt| {
                        if attempt.status == "failed" || attempt.status == "timedOut" {
                            attempt
                                .errors
                                .iter()
                                .map(|error| error.message.clone())
                                .collect::<Vec<_>>()
                        } else {
                            Vec::new()
                        }
                    })
                })
                .flat_map(|message| collect_message_lines(&message))
                .collect::<Vec<_>>();

            failures.push(FailureRecord {
                name: format!("{file_path} > {}", spec.title),
                message_lines,
            });
        }

        collect_playwright_failures(&suite.suites, failures);
    }
}

fn extract_playwright_failures_regex(output: &str) -> Vec<FailureRecord> {
    let lines = exact_non_empty_lines(output);
    let mut failures = Vec::new();
    let mut current_name = None::<String>;
    let mut current_lines = Vec::new();

    for raw_line in lines {
        let trimmed = raw_line.trim();
        if let Some(name) = parse_playwright_failure_header(trimmed) {
            if let Some(existing) = current_name.take() {
                failures.push(FailureRecord {
                    name: existing,
                    message_lines: sanitize_playwright_text_lines(&current_lines),
                });
                current_lines.clear();
            }
            current_name = Some(name);
            continue;
        }

        if is_playwright_summary_line(trimmed) {
            break;
        }

        if current_name.is_some() {
            current_lines.push(trimmed.to_string());
        }
    }

    if let Some(existing) = current_name.take() {
        failures.push(FailureRecord {
            name: existing,
            message_lines: sanitize_playwright_text_lines(&current_lines),
        });
    }

    if failures.is_empty() {
        let simple_lines = non_empty_lines(output);
        return collect_block_failures(
            &simple_lines,
            |line| line.starts_with('✘') || line.starts_with('×') || line.starts_with("Error:"),
            |line| is_playwright_summary_line(line),
        );
    }

    failures
}

fn parse_playwright_failure_header(line: &str) -> Option<String> {
    static HEADER_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let regex = HEADER_RE.get_or_init(|| {
        Regex::new(
            r"^[✘×]\s+\d+\s+(?:\[[^\]]+\]\s+)?›\s+(.+?\.spec\.[tj]sx?)(?::\d+:\d+)?\s+›\s+(.+)$",
        )
        .expect("valid playwright failure regex")
    });
    let captures = regex.captures(line)?;
    let file = captures.get(1)?.as_str();
    let title = captures.get(2)?.as_str();
    Some(format!("{file} > {title}"))
}

fn sanitize_playwright_text_lines(lines: &[String]) -> Vec<String> {
    let mut cleaned = Vec::new();
    let mut in_call_log = false;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('[') && trimmed.ends_with(']') {
            continue;
        }

        if trimmed.starts_with("Call log:") {
            push_playwright_text_line(&mut cleaned, trimmed);
            in_call_log = true;
            continue;
        }

        if in_call_log {
            if is_playwright_call_log_line(trimmed) {
                push_playwright_text_line(&mut cleaned, trimmed);
                continue;
            }
            in_call_log = false;
        }

        if is_playwright_signal_line(trimmed)
            || is_playwright_artifact_line(trimmed)
            || is_playwright_code_frame_line(trimmed)
        {
            push_playwright_text_line(&mut cleaned, trimmed);
        }
    }

    if !contains_playwright_assertion_details(&cleaned) {
        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with('[') && trimmed.ends_with(']')
                || trimmed.starts_with("at ")
            {
                continue;
            }

            push_playwright_text_line(&mut cleaned, trimmed);
            if cleaned.len() >= PLAYWRIGHT_TEXT_LINE_LIMIT {
                break;
            }
        }
    }

    cleaned
}

fn is_playwright_summary_line(line: &str) -> bool {
    line.contains(" passed")
        || line.contains(" failed")
        || line.contains(" skipped")
        || line.contains("flaky")
}

fn is_playwright_call_log_line(line: &str) -> bool {
    line.starts_with('-')
        || line.starts_with("waiting for ")
        || line.starts_with("locator(")
        || line.starts_with("attempting ")
        || line.starts_with("navigating to ")
        || line.starts_with("clicking ")
}

fn is_playwright_signal_line(line: &str) -> bool {
    line.contains("Error:")
        || line.contains("AssertionError")
        || line.contains("Timeout")
        || line.contains("Timed out")
        || line.contains("waiting for ")
        || line.contains("locator(")
        || line.starts_with("expect(")
        || line.starts_with("Expected:")
        || line.starts_with("Received:")
        || line.starts_with("Expected ")
        || line.starts_with("Received ")
        || line.starts_with("- Expected")
        || line.starts_with("+ Received")
        || line.starts_with('-')
        || line.starts_with('+')
}

fn is_playwright_artifact_line(line: &str) -> bool {
    line.starts_with("attachment #")
        || line.starts_with("screenshot #")
        || line.contains(".png")
        || line.contains(".jpg")
        || line.contains(".jpeg")
        || line.contains(".webm")
        || line.contains(".zip")
        || line.contains(".trace")
}

fn is_playwright_code_frame_line(line: &str) -> bool {
    line.starts_with('>')
        || line.starts_with('|')
        || line
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_digit() && line.contains(" |"))
}

fn contains_playwright_assertion_details(lines: &[String]) -> bool {
    lines.iter().any(|line| {
        line.contains("Error:")
            || line.contains("AssertionError")
            || line.starts_with("expect(")
            || line.starts_with("Expected:")
            || line.starts_with("Received:")
            || line.starts_with("Expected ")
            || line.starts_with("Received ")
    })
}

fn push_playwright_text_line(cleaned: &mut Vec<String>, line: &str) {
    if cleaned.len() >= PLAYWRIGHT_TEXT_LINE_LIMIT || cleaned.iter().any(|item| item == line) {
        return;
    }
    cleaned.push(line.to_string());
}
