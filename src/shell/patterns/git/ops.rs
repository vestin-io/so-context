use std::collections::BTreeSet;

use super::super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::super::text::{non_empty_lines, preferred_output, preview, sample_lines};
use super::transport::{combined_lines, summarize_change_stats};

pub(super) fn summarize_add(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let count = explicit_path_count(result);
    let summary = if count == 0 {
        "ok staged".to_string()
    } else {
        format!("ok staged ({count} paths)")
    };

    CompressionSummary::new(
        ShellPattern::GitAdd,
        summary,
        sample_lines(lines, 6),
        preview(&result.stderr),
        result,
    )
}

pub(super) fn summarize_clone(result: &ShellResult) -> CompressionSummary {
    let lines = combined_lines(result);
    let summary = lines
        .iter()
        .find_map(|line| {
            line.strip_prefix("Cloning into ")
                .map(|repo| format!("ok cloned {repo}"))
        })
        .or_else(|| {
            lines.iter().find_map(|line| {
                line.strip_prefix("Checking connectivity... ")
                    .map(|_| "ok cloned".to_string())
            })
        })
        .unwrap_or_else(|| "ok cloned".to_string());

    CompressionSummary::new(
        ShellPattern::GitClone,
        summary,
        sample_lines(lines, 6),
        Vec::new(),
        result,
    )
}

pub(super) fn summarize_merge(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let has_conflict = lines.iter().any(|line| line.contains("CONFLICT"));
    let has_failure = lines
        .iter()
        .any(|line| line.contains("Automatic merge failed"));

    let summary = if has_conflict || has_failure {
        let conflicts = lines
            .iter()
            .filter(|line| line.contains("CONFLICT"))
            .count();
        format!("merge conflict ({conflicts} conflicts)")
    } else if let Some(stats) = summarize_change_stats(&lines) {
        stats
    } else if lines.iter().any(|line| line.contains("Already up to date")) {
        "ok merge (up-to-date)".to_string()
    } else {
        lines
            .first()
            .cloned()
            .unwrap_or_else(|| "ok merge".to_string())
    };

    CompressionSummary::new(
        ShellPattern::GitMerge,
        summary,
        sample_lines(lines, 6),
        preview(&result.stderr),
        result,
    )
}

pub(super) fn summarize_tag(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let summary = if lines.is_empty() {
        "ok tag".to_string()
    } else if lines.len() <= 10 {
        format!("{} ({} tags)", lines.join(", "), lines.len())
    } else {
        format!("{} ({} tags)", lines[..5].join(", "), lines.len())
    };

    CompressionSummary::new(
        ShellPattern::GitTag,
        summary,
        sample_lines(lines, 6),
        preview(&result.stderr),
        result,
    )
}

pub(super) fn summarize_reset(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let unstaged = collect_reset_paths(&lines);
    let summary = if unstaged.is_empty() {
        "reset ok".to_string()
    } else {
        format!("reset ok ({} files unstaged)", unstaged.len())
    };

    CompressionSummary::new(
        ShellPattern::GitReset,
        summary,
        sample_lines(unstaged, 6),
        preview(&result.stderr),
        result,
    )
}

pub(super) fn summarize_stash(result: &ShellResult) -> CompressionSummary {
    let args = result.invocation.args();
    let subcommand = args
        .iter()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .map(String::as_str);
    let lines = non_empty_lines(&preferred_output(result));
    let summary = match subcommand {
        Some("list") => format!("{} stash entries", lines.len()),
        Some("show") => format!("stash show ({})", describe_stash_show(&lines)),
        Some("pop" | "apply" | "drop") => format!("ok stash {}", subcommand.unwrap()),
        _ => "ok stashed".to_string(),
    };
    let details = match subcommand {
        Some("list") => sample_lines(clean_stash_list(lines), 6),
        _ => sample_lines(lines, 6),
    };

    CompressionSummary::new(
        ShellPattern::GitStash,
        summary,
        details,
        preview(&result.stderr),
        result,
    )
}

fn explicit_path_count(result: &ShellResult) -> usize {
    result
        .invocation
        .args()
        .iter()
        .skip(1)
        .filter(|arg| !arg.starts_with('-'))
        .count()
}

fn collect_reset_paths(lines: &[String]) -> Vec<String> {
    let mut files = BTreeSet::new();
    let mut capture = false;

    for line in lines {
        if line.starts_with("Unstaged changes after reset:") {
            capture = true;
            continue;
        }
        if capture {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            files.insert(trimmed.to_string());
        }
    }

    files.into_iter().collect()
}

fn clean_stash_list(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| {
            if let Some((prefix, rest)) = line.split_once(": ")
                && let Some((_, message)) = rest.split_once(": ")
            {
                return format!("{prefix}: {message}");
            }
            line
        })
        .collect()
}

fn describe_stash_show(lines: &[String]) -> String {
    if let Some(summary) = summarize_change_stats(lines) {
        return summary.trim_start_matches("ok ").to_string();
    }

    let diffs = lines
        .iter()
        .filter(|line| line.starts_with('+') || line.starts_with('-'))
        .count();

    if diffs > 0 {
        format!("{diffs} changes")
    } else {
        "no output".to_string()
    }
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
