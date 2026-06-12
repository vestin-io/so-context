use std::collections::BTreeMap;

use super::super::super::types::ShellResult;
use super::super::text::{non_empty_lines, preferred_output};
use super::models::GoTestEvent;
use super::parsing::{format_test_totals, parse_duration_ms, result_failed, strip_ansi};
use super::presentation::{FailureRecord, TestPresentation};

pub(super) fn summarize_go_test(result: &ShellResult) -> TestPresentation {
    let output = preferred_output(result);
    if looks_like_go_json_stream(&output) {
        return summarize_go_json_stream(&output);
    }

    summarize_go_text(&strip_ansi(&output))
}

fn summarize_go_json_stream(output: &str) -> TestPresentation {
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut longest_elapsed = None::<u64>;
    let mut failures = Vec::new();
    let mut messages = BTreeMap::<(String, String), Vec<String>>::new();

    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(event) = serde_json::from_str::<GoTestEvent>(line) else {
            continue;
        };

        if let Some(elapsed) = event.elapsed {
            let elapsed_ms = (elapsed * 1000.0) as u64;
            if longest_elapsed.is_none_or(|current| elapsed_ms > current) {
                longest_elapsed = Some(elapsed_ms);
            }
        }

        match event.action.as_str() {
            "pass" if event.test.is_some() => passed += 1,
            "skip" if event.test.is_some() => skipped += 1,
            "output" if event.test.is_some() => {
                if let Some(output_line) = event.output {
                    let trimmed = output_line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with("=== RUN") {
                        let key = (
                            event.package.unwrap_or_default(),
                            event.test.unwrap_or_default(),
                        );
                        messages.entry(key).or_default().push(trimmed.to_string());
                    }
                }
            }
            "fail" if event.test.is_some() => {
                failed += 1;
                let package = event.package.unwrap_or_default();
                let test = event.test.unwrap_or_default();
                let key = (package.clone(), test.clone());
                failures.push(FailureRecord {
                    name: if package.is_empty() {
                        test
                    } else {
                        format!("{package}::{test}")
                    },
                    message_lines: messages.remove(&key).unwrap_or_default(),
                });
            }
            _ => {}
        }
    }

    TestPresentation {
        summary: if passed == 0 && failed == 0 && skipped == 0 {
            "Go test: no tests found".to_string()
        } else {
            format_test_totals(passed, failed, skipped)
        },
        failures,
        duration_ms: longest_elapsed,
    }
}

fn summarize_go_text(output: &str) -> TestPresentation {
    let lines = non_empty_lines(output);
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut failures = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index].trim();
        if line.starts_with("--- PASS: ") {
            passed += 1;
            index += 1;
            continue;
        }
        if line.starts_with("--- SKIP: ") {
            skipped += 1;
            index += 1;
            continue;
        }
        if let Some(name) = line.strip_prefix("--- FAIL: ") {
            failed += 1;
            index += 1;

            let mut message_lines = Vec::new();
            while index < lines.len() {
                let next = lines[index].trim();
                if next.starts_with("--- ") || next == "FAIL" || next.starts_with("ok ") {
                    break;
                }
                if !next.is_empty() {
                    message_lines.push(next.to_string());
                }
                index += 1;
            }

            failures.push(FailureRecord {
                name: name.split_whitespace().next().unwrap_or(name).to_string(),
                message_lines,
            });
            continue;
        }

        index += 1;
    }

    let summary = if passed == 0 && failed == 0 && skipped == 0 {
        if result_failed(output) {
            "Go test: failed".to_string()
        } else {
            "Go test: ok".to_string()
        }
    } else {
        format_test_totals(passed, failed, skipped)
    };

    TestPresentation {
        summary,
        failures,
        duration_ms: parse_duration_ms(output),
    }
}

fn looks_like_go_json_stream(output: &str) -> bool {
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(|line| line.starts_with('{') && line.contains("\"Action\""))
}
