use std::collections::BTreeSet;

use super::super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::super::text::{non_empty_lines, preferred_output, preview, sample_lines};
use super::diff;
use super::transport::summarize_change_stats;

pub(super) fn summarize_log(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let details = parse_log_entries(&lines);

    CompressionSummary::new(
        ShellPattern::GitLog,
        format!("{} commits", details.len()),
        sample_lines(details, 6),
        preview(&result.stderr),
        result,
    )
}

pub(super) fn summarize_branch(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let mut current = "unknown".to_string();
    let mut local = Vec::new();
    let mut remote = BTreeSet::new();

    for line in lines {
        if let Some(branch) = line.strip_prefix("* ") {
            let branch = branch.trim().to_string();
            current = branch.clone();
            local.push(format!("* {branch}"));
            continue;
        }

        let branch = line.trim().to_string();
        if branch.starts_with("remotes/") {
            if let Some(remote_branch) = normalize_remote_branch(&branch) {
                remote.insert(remote_branch);
            }
        } else if !branch.is_empty() {
            local.push(branch);
        }
    }

    let local_names: BTreeSet<String> = local
        .iter()
        .map(|branch| branch.trim_start_matches("* ").trim().to_string())
        .collect();
    let remote_only: Vec<String> = remote
        .into_iter()
        .filter(|branch| !local_names.contains(branch))
        .collect();

    let mut details = sample_lines(local, 6);
    if !remote_only.is_empty() {
        details.push(format!(
            "remote-only ({}): {}",
            remote_only.len(),
            remote_only.join(", ")
        ));
    }

    CompressionSummary::new(
        ShellPattern::GitBranch,
        format!(
            "current={current}; local={}; remote_only={}",
            local_names.len(),
            remote_only.len()
        ),
        details,
        preview(&result.stderr),
        result,
    )
}

pub(super) fn summarize_remote(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let mut rendered = Vec::new();
    let mut remotes = BTreeSet::new();

    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let name = parts[0].to_string();
        let url = parts[1].to_string();
        remotes.insert(name.clone());
        let rendered_line = format!("{name}: {url}");
        if !rendered.contains(&rendered_line) {
            rendered.push(rendered_line);
        }
    }

    CompressionSummary::new(
        ShellPattern::GitRemote,
        format!("{} remotes", remotes.len()),
        sample_lines(rendered, 6),
        preview(&result.stderr),
        result,
    )
}

pub(super) fn summarize_show(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let commit = parse_show_commit(&lines);
    let diff_like = diff::summarize(result);
    let show_summary = if diff_like.details.is_empty() {
        summarize_change_stats(&lines).unwrap_or_else(|| diff_like.summary.clone())
    } else {
        diff_like.summary.clone()
    };
    let summary = match commit {
        Some((hash, subject)) => format!("{hash} {subject}; {show_summary}"),
        None => show_summary,
    };

    CompressionSummary::new(
        ShellPattern::GitShow,
        summary,
        if diff_like.details.is_empty() {
            sample_lines(lines, 6)
        } else {
            diff_like.details
        },
        preview(&result.stderr),
        result,
    )
}

fn parse_log_entries(lines: &[String]) -> Vec<String> {
    if lines.iter().any(|line| line.starts_with("commit ")) {
        return parse_raw_log_entries(lines);
    }

    lines
        .iter()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            if !looks_like_hash(hash) {
                return None;
            }
            let subject = parts.collect::<Vec<_>>().join(" ");
            Some(if subject.is_empty() {
                hash.to_string()
            } else {
                format!("{hash} {subject}")
            })
        })
        .collect()
}

fn parse_raw_log_entries(lines: &[String]) -> Vec<String> {
    let mut entries = Vec::new();
    let mut current_hash: Option<String> = None;
    let mut current_subject: Option<String> = None;

    for line in lines {
        if let Some(hash) = line.strip_prefix("commit ") {
            if let Some(hash) = current_hash.take() {
                entries.push(
                    format!("{hash} {}", current_subject.take().unwrap_or_default())
                        .trim()
                        .to_string(),
                );
            }
            current_hash = Some(short_hash(hash.trim()));
            current_subject = None;
            continue;
        }

        if current_hash.is_some() && current_subject.is_none() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("Author:")
                || trimmed.starts_with("Date:")
                || trimmed.starts_with("Merge:")
            {
                continue;
            }
            current_subject = Some(trimmed.to_string());
        }
    }

    if let Some(hash) = current_hash {
        entries.push(
            format!("{hash} {}", current_subject.unwrap_or_default())
                .trim()
                .to_string(),
        );
    }

    entries
}

fn parse_show_commit(lines: &[String]) -> Option<(String, String)> {
    let mut hash = None;
    let mut subject = None;

    for line in lines {
        if hash.is_none()
            && let Some(value) = line.strip_prefix("commit ")
        {
            hash = Some(short_hash(value.trim()));
            continue;
        }

        if hash.is_some() && subject.is_none() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("Author:")
                || trimmed.starts_with("Date:")
                || trimmed.starts_with("Merge:")
                || trimmed.starts_with("diff --git ")
            {
                continue;
            }
            subject = Some(trimmed.to_string());
            break;
        }
    }

    Some((hash?, subject?))
}

fn normalize_remote_branch(branch: &str) -> Option<String> {
    let rest = branch.strip_prefix("remotes/")?;
    let (_, branch_name) = rest.split_once('/')?;
    if branch_name.contains("->") {
        return None;
    }
    Some(branch_name.to_string())
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(7).collect()
}

fn looks_like_hash(text: &str) -> bool {
    let len = text.len();
    (7..=40).contains(&len) && text.chars().all(|ch| ch.is_ascii_hexdigit())
}

#[cfg(test)]
#[path = "refs_tests.rs"]
mod tests;
