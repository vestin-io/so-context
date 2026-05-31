use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::argv::first_positional;
use super::text::{compact_whitespace, non_empty_lines, preview, sample_lines, truncate_text};

fn summarize(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let lines = non_empty_lines(&result.stdout);
    let details = sample_lines(compact_list(lines.clone(), pattern), 10);
    let summary = if details.is_empty() {
        empty_summary(pattern).to_string()
    } else if is_list_command(result) {
        match pattern {
            ShellPattern::GhPr => format!("{} PRs", details.len()),
            ShellPattern::GhIssue => format!("{} issues", details.len()),
            ShellPattern::GhRun => format!("{} runs", details.len()),
            _ => truncate_text(&details[0], 120),
        }
    } else {
        truncate_text(&details[0], 120)
    };

    CompressionSummary::new(
        pattern,
        summary,
        if is_list_command(result) {
            details
        } else {
            details.into_iter().skip(1).collect()
        },
        preview(&result.stderr),
        result,
    )
}

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    if program != "gh" {
        return None;
    }

    match first_positional(args, &["-R", "--repo", "--hostname"]) {
        Some("pr") => Some(ShellPattern::GhPr),
        Some("issue") => Some(ShellPattern::GhIssue),
        Some("run") => Some(ShellPattern::GhRun),
        _ => None,
    }
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    matches!(
        pattern,
        ShellPattern::GhPr | ShellPattern::GhIssue | ShellPattern::GhRun
    )
    .then(|| summarize(result, pattern))
}

fn compact_list(lines: Vec<String>, pattern: ShellPattern) -> Vec<String> {
    lines
        .into_iter()
        .filter(|line| !line.starts_with("Showing "))
        .filter_map(|line| match pattern {
            ShellPattern::GhPr => compact_pr_line(&line),
            ShellPattern::GhIssue => compact_issue_line(&line),
            ShellPattern::GhRun => Some(compact_run_line(&line)),
            _ => Some(compact_whitespace(&line)),
        })
        .collect()
}

fn compact_pr_line(line: &str) -> Option<String> {
    if line.trim().is_empty() {
        return None;
    }
    let columns: Vec<&str> = if line.contains('\t') {
        line.split('\t').collect()
    } else {
        line.split_whitespace().collect()
    };
    let id = columns.first()?.trim_start_matches('#');
    let title = columns.get(1).copied().unwrap_or(line);
    Some(format!("#{id} {title}"))
}

fn compact_issue_line(line: &str) -> Option<String> {
    if line.trim().is_empty() {
        return None;
    }
    let columns: Vec<&str> = if line.contains('\t') {
        line.split('\t').collect()
    } else {
        line.split_whitespace().collect()
    };
    let id = columns.first()?.trim_start_matches('#');
    let title = columns
        .get(2)
        .or_else(|| columns.get(1))
        .copied()
        .unwrap_or(line);
    Some(format!("#{id} {title}"))
}

fn compact_run_line(line: &str) -> String {
    if line.contains('\t') {
        let columns: Vec<&str> = line.split('\t').collect();
        let status = columns.first().copied().unwrap_or("");
        let conclusion = columns.get(1).copied().unwrap_or("");
        let title = columns.get(2).copied().unwrap_or(line);
        let run_id = columns
            .iter()
            .find(|value| value.chars().all(|ch| ch.is_ascii_digit()))
            .copied();
        let summary = if conclusion.is_empty() {
            format!("{title} ({status})")
        } else {
            format!("{title} ({conclusion})")
        };
        if let Some(run_id) = run_id {
            format!("{summary} [{run_id}]")
        } else {
            summary
        }
    } else {
        compact_whitespace(line)
    }
}

fn is_list_command(result: &ShellResult) -> bool {
    matches!(
        result.invocation.args().get(1).map(String::as_str),
        Some("list")
    )
}

fn empty_summary(pattern: ShellPattern) -> &'static str {
    match pattern {
        ShellPattern::GhPr => "No PRs",
        ShellPattern::GhIssue => "No issues",
        ShellPattern::GhRun => "No runs",
        _ => "No output",
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
