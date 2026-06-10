use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::shell::types::{CompressionSummary, ShellPattern, ShellResult};

use super::super::text::{compact_whitespace, non_empty_lines, preferred_output, preview};

#[derive(Debug, Clone)]
struct RuffIssue {
    file: String,
    rule: String,
    line: usize,
    column: usize,
    message: String,
}

#[derive(Debug, Clone)]
struct MypyIssue {
    file: String,
    line: usize,
    code: String,
    message: String,
    notes: Vec<String>,
}

pub(super) fn summarize_ruff(result: &ShellResult) -> CompressionSummary {
    if is_ruff_format_invocation(result) || looks_like_ruff_format_output(result) {
        return summarize_ruff_format(result);
    }

    let lines = normalized_lines(result);
    let issues = lines
        .iter()
        .filter_map(|line| parse_ruff_issue(line))
        .collect::<Vec<_>>();

    if issues.is_empty() {
        let summary = lines
            .iter()
            .find(|line| line == &"All checks passed!")
            .cloned()
            .unwrap_or_else(|| {
                if result.exit_code == 0 {
                    "Ruff: No issues found".to_string()
                } else {
                    lines
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Ruff: failed".to_string())
                }
            });
        return CompressionSummary::plain(
            ShellPattern::PythonRuff,
            summary,
            Vec::new(),
            preview(&result.stderr),
            result,
        );
    }

    let file_count = issues
        .iter()
        .map(|issue| issue.file.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let summary = format!(
        "Ruff: {} issues in {} {}",
        issues.len(),
        file_count,
        pluralize(file_count, "file", "files")
    );
    let mut details = render_rule_and_file_details(
        &count_by_key(issues.iter().map(|issue| issue.rule.clone())),
        &group_rules_by_file(issues.iter().map(|issue| (&issue.file, &issue.rule))),
    );

    if !details.is_empty() {
        details.push(String::new());
    }
    details.push("Violations:".to_string());
    for issue in issues.iter().take(20) {
        details.push(format!(
            "  {}:{}:{} {} {}",
            compact_path(&issue.file),
            issue.line,
            issue.column,
            issue.rule,
            truncate_message(&issue.message, 100)
        ));
    }
    if issues.len() > 20 {
        details.push(format!("  ... +{} more", issues.len() - 20));
    }

    CompressionSummary::plain(
        ShellPattern::PythonRuff,
        summary,
        details,
        preview(&result.stderr),
        result,
    )
}

pub(super) fn summarize_mypy(result: &ShellResult) -> CompressionSummary {
    let output = preferred_output(result);
    let lines = non_empty_lines(&output);
    let (fileless_lines, issues) = parse_mypy_issues(&lines);

    if issues.is_empty() && fileless_lines.is_empty() {
        return CompressionSummary::plain(
            ShellPattern::PythonMypy,
            "mypy: No issues found",
            Vec::new(),
            preview(&result.stderr),
            result,
        );
    }

    let mut details = Vec::new();
    if !fileless_lines.is_empty() {
        details.extend(fileless_lines);
    }

    if !issues.is_empty() {
        if !details.is_empty() {
            details.push(String::new());
        }
        let file_count = issues
            .iter()
            .map(|issue| issue.file.as_str())
            .collect::<BTreeSet<_>>()
            .len();
        let summary = format!(
            "mypy: {} errors in {} {}",
            issues.len(),
            file_count,
            pluralize(file_count, "file", "files")
        );
        let code_counts = count_by_key(
            issues
                .iter()
                .filter(|issue| !issue.code.is_empty())
                .map(|issue| issue.code.clone()),
        );
        if code_counts.len() > 1 {
            let top_codes = sorted_counts(&code_counts)
                .into_iter()
                .take(5)
                .map(|(code, count)| format!("{code} ({count}x)"))
                .collect::<Vec<_>>()
                .join(", ");
            details.push(format!("Top codes: {top_codes}"));
            details.push(String::new());
        }

        let mut grouped = BTreeMap::<String, Vec<&MypyIssue>>::new();
        for issue in &issues {
            grouped.entry(issue.file.clone()).or_default().push(issue);
        }
        let mut files = grouped.into_iter().collect::<Vec<_>>();
        files.sort_by(|left, right| {
            right
                .1
                .len()
                .cmp(&left.1.len())
                .then_with(|| left.0.cmp(&right.0))
        });
        for (file, file_issues) in files {
            details.push(format!(
                "{} ({} errors)",
                compact_path(&file),
                file_issues.len()
            ));
            for issue in file_issues {
                if issue.code.is_empty() {
                    details.push(format!(
                        "  L{}: {}",
                        issue.line,
                        truncate_message(&issue.message, 120)
                    ));
                } else {
                    details.push(format!(
                        "  L{}: [{}] {}",
                        issue.line,
                        issue.code,
                        truncate_message(&issue.message, 120)
                    ));
                }
                for note in &issue.notes {
                    details.push(format!("    {}", truncate_message(note, 120)));
                }
            }
        }

        return CompressionSummary::plain(
            ShellPattern::PythonMypy,
            summary,
            details,
            preview(&result.stderr),
            result,
        );
    }

    CompressionSummary::plain(
        ShellPattern::PythonMypy,
        "mypy: No issues found",
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_ruff_format(result: &ShellResult) -> CompressionSummary {
    let lines = normalized_lines(result);
    let files = lines
        .iter()
        .filter_map(|line| {
            line.strip_prefix("Would reformat:")
                .map(str::trim)
                .or_else(|| line.strip_prefix("would reformat:").map(str::trim))
        })
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let summary = if files.is_empty() && lines.iter().any(|line| line.contains("left unchanged")) {
        "Ruff format: All files formatted correctly".to_string()
    } else if !files.is_empty() {
        format!(
            "Ruff format: {} {} need formatting",
            files.len(),
            pluralize(files.len(), "file", "files")
        )
    } else {
        lines
            .first()
            .cloned()
            .unwrap_or_else(|| "Ruff format: failed".to_string())
    };

    let mut details = files
        .iter()
        .take(10)
        .map(|file| compact_path(file))
        .collect::<Vec<_>>();
    if files.len() > 10 {
        details.push(format!("... +{} more files", files.len() - 10));
    }

    CompressionSummary::plain(
        ShellPattern::PythonRuff,
        summary,
        details,
        preview(&result.stderr),
        result,
    )
}

fn looks_like_ruff_format_output(result: &ShellResult) -> bool {
    let output = preferred_output(result).to_lowercase();
    output.contains("would reformat") || output.contains("left unchanged")
}

fn is_ruff_format_invocation(result: &ShellResult) -> bool {
    result
        .invocation
        .args()
        .iter()
        .any(|arg| arg == "format" || arg == "--check")
        && result.invocation.args().iter().any(|arg| arg == "format")
}

fn parse_ruff_issue(line: &str) -> Option<RuffIssue> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let regex = RE.get_or_init(|| {
        Regex::new(r"^(.+?):(\d+):(\d+):\s+([A-Z]\d+)\s+(.+)$").expect("valid ruff regex")
    });
    let captures = regex.captures(line)?;
    Some(RuffIssue {
        file: captures.get(1)?.as_str().to_string(),
        line: captures.get(2)?.as_str().parse().ok()?,
        column: captures.get(3)?.as_str().parse().ok()?,
        rule: captures.get(4)?.as_str().to_string(),
        message: captures.get(5)?.as_str().to_string(),
    })
}

fn parse_mypy_issues(lines: &[String]) -> (Vec<String>, Vec<MypyIssue>) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let regex = RE.get_or_init(|| {
        Regex::new(r"^(.+?):(\d+)(?::\d+)?:\s+(error|warning|note):\s+(.+?)(?:\s+\[(.+)\])?$")
            .expect("valid mypy regex")
    });

    let mut issues: Vec<MypyIssue> = Vec::new();
    let mut fileless = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        let line = &lines[index];
        if line.starts_with("Found ") || line.starts_with("Success:") {
            index += 1;
            continue;
        }

        if let Some(captures) = regex.captures(line) {
            let severity = captures.get(3).map(|value| value.as_str()).unwrap_or("");
            let file = captures
                .get(1)
                .map(|value| value.as_str().to_string())
                .unwrap_or_default();
            let line_num = captures
                .get(2)
                .and_then(|value| value.as_str().parse::<usize>().ok())
                .unwrap_or(0);
            let message = captures
                .get(4)
                .map(|value| value.as_str().to_string())
                .unwrap_or_default();
            let code = captures
                .get(5)
                .map(|value| value.as_str().to_string())
                .unwrap_or_default();

            if severity == "note" {
                if let Some(last) = issues.last_mut() {
                    if last.file == file {
                        last.notes.push(message);
                        index += 1;
                        continue;
                    }
                }
                fileless.push(line.clone());
                index += 1;
                continue;
            }

            let mut issue = MypyIssue {
                file,
                line: line_num,
                code,
                message,
                notes: Vec::new(),
            };
            index += 1;
            while index < lines.len() {
                if let Some(note_caps) = regex.captures(&lines[index]) {
                    if note_caps.get(3).map(|value| value.as_str()) == Some("note")
                        && note_caps.get(1).map(|value| value.as_str()) == Some(issue.file.as_str())
                    {
                        issue.notes.push(
                            note_caps
                                .get(4)
                                .map(|value| value.as_str().to_string())
                                .unwrap_or_default(),
                        );
                        index += 1;
                        continue;
                    }
                }
                break;
            }
            issues.push(issue);
            continue;
        }

        if line.contains("error:") {
            fileless.push(line.clone());
        }
        index += 1;
    }

    (fileless, issues)
}

fn normalized_lines(result: &ShellResult) -> Vec<String> {
    non_empty_lines(&preferred_output(result))
        .into_iter()
        .map(|line| compact_whitespace(&line))
        .collect()
}

fn count_by_key<I>(items: I) -> BTreeMap<String, usize>
where
    I: IntoIterator<Item = String>,
{
    let mut counts = BTreeMap::new();
    for item in items {
        *counts.entry(item).or_insert(0) += 1;
    }
    counts
}

fn group_rules_by_file<'a, I>(items: I) -> BTreeMap<String, BTreeMap<String, usize>>
where
    I: IntoIterator<Item = (&'a String, &'a String)>,
{
    let mut grouped = BTreeMap::new();
    for (file, rule) in items {
        *grouped
            .entry(file.clone())
            .or_insert_with(BTreeMap::new)
            .entry(rule.clone())
            .or_insert(0) += 1;
    }
    grouped
}

fn render_rule_and_file_details(
    rule_counts: &BTreeMap<String, usize>,
    file_rules: &BTreeMap<String, BTreeMap<String, usize>>,
) -> Vec<String> {
    let mut details = Vec::new();
    let sorted_rules = sorted_counts(rule_counts);
    if !sorted_rules.is_empty() {
        details.push("Top rules:".to_string());
        for (rule, count) in sorted_rules.into_iter().take(10) {
            details.push(format!("  {rule} ({count}x)"));
        }
    }

    let mut files = file_rules
        .iter()
        .map(|(file, rules)| (file.clone(), rules.values().sum::<usize>(), rules))
        .collect::<Vec<_>>();
    files.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    if !files.is_empty() {
        if !details.is_empty() {
            details.push(String::new());
        }
        details.push("Top files:".to_string());
        for (file, count, rules) in files.into_iter().take(10) {
            details.push(format!("  {} ({} issues)", compact_path(&file), count));
            for (rule, rule_count) in sorted_counts(rules).into_iter().take(3) {
                details.push(format!("    {rule} ({rule_count})"));
            }
        }
    }

    details
}

fn sorted_counts(counts: &BTreeMap<String, usize>) -> Vec<(String, usize)> {
    let mut values = counts
        .iter()
        .map(|(key, count)| (key.clone(), *count))
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    values
}

fn compact_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    if path.starts_with("src/") || path.starts_with("lib/") || path.starts_with("tests/") {
        path
    } else if let Some(position) = path.rfind("/src/") {
        format!("src/{}", &path[position + 5..])
    } else if let Some(position) = path.rfind("/lib/") {
        format!("lib/{}", &path[position + 5..])
    } else if let Some(position) = path.rfind("/tests/") {
        format!("tests/{}", &path[position + 7..])
    } else if let Some(position) = path.rfind('/') {
        path[position + 1..].to_string()
    } else {
        path
    }
}

fn truncate_message(message: &str, limit: usize) -> String {
    let compact = compact_whitespace(message);
    if compact.chars().count() <= limit {
        compact
    } else {
        let truncated = compact
            .chars()
            .take(limit.saturating_sub(1))
            .collect::<String>();
        format!("{truncated}…")
    }
}

fn pluralize<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 {
        singular
    } else {
        plural
    }
}
