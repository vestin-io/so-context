use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::Deserialize;

use super::super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::super::text::{compact_whitespace, non_empty_lines, preferred_output, preview};

const SECTION_LIMIT: usize = 10;

#[derive(Debug, Clone)]
pub(super) struct EslintIssue {
    pub(super) file: String,
    pub(super) rule: String,
    pub(super) severity: String,
}

#[derive(Debug, Clone)]
pub(super) struct BiomeIssue {
    pub(super) file: String,
    pub(super) rule: String,
}

#[derive(Debug, Clone)]
pub(super) struct GolangciIssue {
    pub(super) file: String,
    pub(super) linter: String,
    pub(super) message: String,
    pub(super) source_line: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GolangciJsonOutput {
    #[serde(rename = "Issues", default)]
    issues: Vec<GolangciJsonIssue>,
}

#[derive(Debug, Deserialize, Default)]
struct GolangciJsonIssue {
    #[serde(rename = "FromLinter", default)]
    from_linter: String,
    #[serde(rename = "Text", default)]
    text: String,
    #[serde(rename = "SourceLines", default)]
    source_lines: Vec<String>,
    #[serde(rename = "Pos", default)]
    pos: GolangciJsonPos,
}

#[derive(Debug, Deserialize, Default)]
struct GolangciJsonPos {
    #[serde(rename = "Filename", default)]
    filename: String,
}

pub(super) fn normalized_lines(result: &ShellResult) -> Vec<String> {
    non_empty_lines(&preferred_output(result))
        .into_iter()
        .map(|line| compact_whitespace(&line))
        .collect()
}

pub(super) fn fallback_lint_summary(
    result: &ShellResult,
    pattern: ShellPattern,
    clean_summary: &str,
    failed_summary: &str,
) -> CompressionSummary {
    let lines = normalized_lines(result);
    let summary = if result.exit_code == 0 {
        clean_summary.to_string()
    } else {
        lines
            .first()
            .cloned()
            .unwrap_or_else(|| failed_summary.to_string())
    };

    CompressionSummary::plain(
        pattern,
        summary,
        Vec::new(),
        preview(&result.stderr),
        result,
    )
}

pub(super) fn parse_eslint_issue(line: &str) -> Option<EslintIssue> {
    static ISSUE_RE: OnceLock<Regex> = OnceLock::new();
    let regex = ISSUE_RE.get_or_init(|| {
        Regex::new(r"^(.+?):\d+:\d+:\s+.+\s+\[(Error|Warning)/([^\]]+)\]$")
            .expect("valid eslint regex")
    });
    let captures = regex.captures(line)?;

    Some(EslintIssue {
        file: captures.get(1)?.as_str().to_string(),
        severity: captures.get(2)?.as_str().to_string(),
        rule: captures.get(3)?.as_str().to_string(),
    })
}

pub(super) fn parse_biome_issue(line: &str) -> Option<BiomeIssue> {
    static ISSUE_RE: OnceLock<Regex> = OnceLock::new();
    let regex = ISSUE_RE.get_or_init(|| {
        Regex::new(r"^(.+?):\d+:\d+\s+([[:alnum:]/_-]+)\s+.+$").expect("valid biome regex")
    });
    let captures = regex.captures(line)?;

    Some(BiomeIssue {
        file: captures.get(1)?.as_str().to_string(),
        rule: captures.get(2)?.as_str().to_string(),
    })
}

pub(super) fn parse_golangci_issue(line: &str) -> Option<GolangciIssue> {
    static ISSUE_RE: OnceLock<Regex> = OnceLock::new();
    let regex = ISSUE_RE.get_or_init(|| {
        Regex::new(r"^(.+?):\d+:\d+:\s+(.+?)(?:\s+\(([^()]+)\))?$").expect("valid golangci regex")
    });
    let captures = regex.captures(line)?;

    Some(GolangciIssue {
        file: captures.get(1)?.as_str().to_string(),
        message: captures.get(2)?.as_str().to_string(),
        linter: captures
            .get(3)
            .map(|value| value.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        source_line: None,
    })
}

pub(super) fn parse_golangci_json_output(output: &str) -> Option<Vec<GolangciIssue>> {
    let json = parse_json_payload(output)?;
    let parsed = serde_json::from_str::<GolangciJsonOutput>(json).ok()?;
    Some(
        parsed
            .issues
            .into_iter()
            .map(|issue| GolangciIssue {
                file: issue.pos.filename,
                linter: issue.from_linter,
                message: issue.text,
                source_line: issue
                    .source_lines
                    .first()
                    .map(|line| compact_whitespace(line))
                    .filter(|line| !line.is_empty()),
            })
            .collect(),
    )
}

pub(super) fn count_by_key<I>(items: I) -> BTreeMap<String, usize>
where
    I: IntoIterator<Item = String>,
{
    let mut counts = BTreeMap::new();
    for item in items {
        *counts.entry(item).or_insert(0) += 1;
    }
    counts
}

pub(super) fn group_rules_by_file<'a, I>(items: I) -> BTreeMap<String, BTreeMap<String, usize>>
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

pub(super) fn render_top_rules_and_files(
    rule_counts: &BTreeMap<String, usize>,
    file_rules: &BTreeMap<String, BTreeMap<String, usize>>,
) -> Vec<String> {
    let mut details = Vec::new();

    let sorted_rules = sorted_counts(rule_counts);
    if !sorted_rules.is_empty() {
        details.push("Top rules:".to_string());
        for (rule, count) in sorted_rules.into_iter().take(SECTION_LIMIT) {
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
        for (file, issue_count, rules) in files.iter().take(SECTION_LIMIT) {
            details.push(format!("  {} ({} issues)", compact_path(file), issue_count));
            for (rule, count) in sorted_counts(rules).into_iter().take(3) {
                details.push(format!("    {rule} ({count})"));
            }
        }
        if files.len() > SECTION_LIMIT {
            details.push(String::new());
            details.push(format!("... +{} more files", files.len() - SECTION_LIMIT));
        }
    }

    details
}

pub(super) fn render_golangci_details(issues: &[GolangciIssue]) -> Vec<String> {
    let mut details = Vec::new();

    let linter_counts = count_by_key(issues.iter().map(|issue| issue.linter.clone()));
    let sorted_linters = sorted_counts(&linter_counts);
    if !sorted_linters.is_empty() {
        details.push("Top linters:".to_string());
        for (linter, count) in sorted_linters.into_iter().take(SECTION_LIMIT) {
            details.push(format!("  {linter} ({count}x)"));
        }
    }

    let mut by_file = BTreeMap::<String, Vec<&GolangciIssue>>::new();
    for issue in issues {
        by_file.entry(issue.file.clone()).or_default().push(issue);
    }
    let mut files = by_file.into_iter().collect::<Vec<_>>();
    files.sort_by(|left, right| {
        right
            .1
            .len()
            .cmp(&left.1.len())
            .then_with(|| left.0.cmp(&right.0))
    });

    if !files.is_empty() {
        if !details.is_empty() {
            details.push(String::new());
        }
        details.push("Top files:".to_string());
        for (file, file_issues) in files.iter().take(SECTION_LIMIT) {
            details.push(format!(
                "  {} ({} issues)",
                compact_path(file),
                file_issues.len()
            ));

            let mut by_linter = BTreeMap::<String, Vec<&GolangciIssue>>::new();
            for issue in file_issues {
                by_linter
                    .entry(issue.linter.clone())
                    .or_default()
                    .push(*issue);
            }
            let mut linter_groups = by_linter.into_iter().collect::<Vec<_>>();
            linter_groups.sort_by(|left, right| {
                right
                    .1
                    .len()
                    .cmp(&left.1.len())
                    .then_with(|| left.0.cmp(&right.0))
            });
            for (linter, linter_issues) in linter_groups.into_iter().take(3) {
                details.push(format!("    {linter} ({})", linter_issues.len()));
                if let Some(first_issue) = linter_issues.first() {
                    if let Some(source_line) = &first_issue.source_line {
                        details.push(format!("      -> {}", truncate_source_line(source_line)));
                    } else {
                        details.push(format!("      -> {}", truncate_issue(&first_issue.message)));
                    }
                }
            }
        }
        if files.len() > SECTION_LIMIT {
            details.push(String::new());
            details.push(format!("... +{} more files", files.len() - SECTION_LIMIT));
        }
    }

    details
}

pub(super) fn noun<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 {
        singular
    } else {
        plural
    }
}

fn sorted_counts(counts: &BTreeMap<String, usize>) -> Vec<(String, usize)> {
    let mut items = counts
        .iter()
        .map(|(key, count)| (key.clone(), *count))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    items
}

fn compact_path(path: &str) -> String {
    let path = path.replace('\\', "/");
    if path.starts_with("src/")
        || path.starts_with("lib/")
        || path.starts_with("pkg/")
        || path.starts_with("cmd/")
        || path.starts_with("internal/")
    {
        path
    } else if let Some(position) = path.rfind("/src/") {
        format!("src/{}", &path[position + 5..])
    } else if let Some(position) = path.rfind("/lib/") {
        format!("lib/{}", &path[position + 5..])
    } else if let Some(position) = path.rfind("/pkg/") {
        format!("pkg/{}", &path[position + 5..])
    } else if let Some(position) = path.rfind("/cmd/") {
        format!("cmd/{}", &path[position + 5..])
    } else if let Some(position) = path.rfind("/internal/") {
        format!("internal/{}", &path[position + 10..])
    } else if let Some(position) = path.rfind('/') {
        path[position + 1..].to_string()
    } else {
        path
    }
}

fn truncate_issue(message: &str) -> String {
    let compact = compact_whitespace(message);
    if compact.chars().count() <= 80 {
        compact
    } else {
        let truncated = compact.chars().take(79).collect::<String>();
        format!("{truncated}…")
    }
}

fn truncate_source_line(message: &str) -> String {
    let compact = compact_whitespace(message);
    if compact.chars().count() <= 80 {
        compact
    } else {
        let truncated = compact.chars().take(79).collect::<String>();
        format!("{truncated}…")
    }
}

fn parse_json_payload(output: &str) -> Option<&str> {
    if serde_json::from_str::<serde_json::Value>(output).is_ok() {
        return Some(output);
    }

    let start = output.find('{')?;
    let end = output.rfind('}')?;
    let payload = &output[start..=end];
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .map(|_| payload)
}
