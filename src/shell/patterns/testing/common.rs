use std::sync::OnceLock;

use regex::Regex;

use crate::shell::types::{CompressionSummary, ShellPattern, ShellResult};

use super::super::text::{compact_whitespace, non_empty_lines, truncate_text};

const FAILURE_LIMIT: usize = 5;

#[derive(Debug, Clone)]
pub(super) struct FailureRecord {
    pub(super) name: String,
    pub(super) message_lines: Vec<String>,
}

pub(super) struct TestPresentation {
    pub(super) summary: String,
    pub(super) failures: Vec<FailureRecord>,
    pub(super) duration_ms: Option<u64>,
}

pub(super) fn render_test_presentation(
    pattern: ShellPattern,
    presentation: TestPresentation,
    result: &ShellResult,
) -> CompressionSummary {
    let mut details = Vec::new();

    for (index, failure) in presentation.failures.iter().take(FAILURE_LIMIT).enumerate() {
        details.push(format!(
            "{}. {}",
            index + 1,
            truncate_text(&failure.name, 140)
        ));
        for line in &failure.message_lines {
            details.push(format!("   {line}"));
        }
    }

    if presentation.failures.len() > FAILURE_LIMIT {
        details.push(format!(
            "... +{} more failures",
            presentation.failures.len() - FAILURE_LIMIT
        ));
    }

    if let Some(duration_ms) = presentation.duration_ms {
        if !details.is_empty() {
            details.push(String::new());
        }
        details.push(format!("Time: {duration_ms}ms"));
    }

    CompressionSummary::plain(pattern, presentation.summary, details, Vec::new(), result)
}

pub(super) fn collect_block_failures<FStart, FStop>(
    lines: &[String],
    is_start: FStart,
    is_stop: FStop,
) -> Vec<FailureRecord>
where
    FStart: Fn(&str) -> bool,
    FStop: Fn(&str) -> bool,
{
    let mut failures = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = lines[index].trim();
        if !is_start(line) {
            index += 1;
            continue;
        }

        let mut message_lines = Vec::new();
        index += 1;
        while index < lines.len() {
            let next = lines[index].trim();
            if next.is_empty() || is_start(next) || is_stop(next) {
                break;
            }
            message_lines.push(next.to_string());
            index += 1;
        }

        failures.push(FailureRecord {
            name: line.to_string(),
            message_lines,
        });
    }

    failures
}

pub(super) fn numbered_failure_marker(line: &str) -> bool {
    line.chars().next().is_some_and(|ch| ch.is_ascii_digit()) && line.contains(')')
}

pub(super) fn extract_json_object(output: &str) -> Option<&str> {
    let start = output.find('{')?;
    let end = output.rfind('}')?;
    (end > start).then_some(&output[start..=end])
}

pub(super) fn collect_message_lines(block: &str) -> Vec<String> {
    non_empty_lines(block)
        .into_iter()
        .map(|line| compact_whitespace(&line))
        .filter(|line| !line.starts_with("FAIL "))
        .collect()
}

pub(super) fn format_failure_block(file_name: &str, failure_messages: &[String]) -> String {
    let mut parts = Vec::new();
    if !file_name.is_empty() {
        parts.push(file_name.to_string());
    }
    for message in failure_messages {
        parts.push(strip_ansi(message));
    }
    parts.join("\n")
}

pub(super) fn format_test_totals(passed: usize, failed: usize, skipped: usize) -> String {
    let mut summary = format!("PASS ({passed}) FAIL ({failed})");
    if skipped > 0 {
        summary.push_str(&format!(" skipped ({skipped})"));
    }
    summary
}

pub(super) fn find_test_counts(
    lines: &[String],
    patterns: &[Regex],
) -> Option<(usize, usize, usize)> {
    for line in lines {
        for pattern in patterns {
            if let Some(caps) = pattern.captures(line) {
                let failed = caps
                    .get(1)
                    .and_then(|value| value.as_str().parse::<usize>().ok())
                    .unwrap_or(0);
                let passed = caps
                    .get(2)
                    .and_then(|value| value.as_str().parse::<usize>().ok())
                    .unwrap_or(0);
                let skipped = caps
                    .get(3)
                    .and_then(|value| value.as_str().parse::<usize>().ok())
                    .unwrap_or(0);
                return Some((passed, failed, skipped));
            }
        }
    }

    None
}

pub(super) fn find_keyword_counts(output: &str, keywords: &[&str]) -> (usize, usize, usize) {
    let count_for = |keyword: &str| {
        let regex = Regex::new(&format!(r"(\d+)\s+{keyword}")).expect("valid keyword regex");
        regex
            .captures_iter(output)
            .filter_map(|caps| caps[1].parse::<usize>().ok())
            .last()
            .unwrap_or(0)
    };

    (
        count_for(keywords[0]),
        count_for(keywords[1]),
        count_for(keywords[2]),
    )
}

pub(super) fn parse_duration_ms(output: &str) -> Option<u64> {
    static DURATION_RE: OnceLock<Regex> = OnceLock::new();
    let regex = DURATION_RE.get_or_init(|| {
        Regex::new(r"(?i)(\d+(?:\.\d+)?)\s*(ms|s|m)\b").expect("valid duration regex")
    });

    regex
        .captures_iter(output)
        .filter_map(|caps| {
            let value = caps[1].parse::<f64>().ok()?;
            match &caps[2] {
                "ms" | "MS" => Some(value as u64),
                "s" | "S" => Some((value * 1000.0) as u64),
                "m" | "M" => Some((value * 60_000.0) as u64),
                _ => None,
            }
        })
        .last()
}

pub(super) fn strip_ansi(text: &str) -> String {
    static ANSI_RE: OnceLock<Regex> = OnceLock::new();
    let regex =
        ANSI_RE.get_or_init(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").expect("valid ansi regex"));
    regex.replace_all(text, "").into_owned()
}

pub(super) fn result_failed(output: &str) -> bool {
    output.contains("FAIL") || output.contains("failed")
}
