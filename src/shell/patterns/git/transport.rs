use super::super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::super::text::{non_empty_lines, preferred_output, preview, sample_lines};

pub(super) fn summarize_fetch(result: &ShellResult) -> CompressionSummary {
    let lines = combined_lines(result);
    let new_refs = collect_new_refs(&lines);
    let updated_refs = collect_updated_refs(&lines);
    let summary = if !new_refs.is_empty() {
        format!("ok fetched ({} new refs)", new_refs.len())
    } else if !updated_refs.is_empty() {
        format!("ok fetched ({} updated refs)", updated_refs.len())
    } else {
        "ok fetched".to_string()
    };

    let details = if !new_refs.is_empty() {
        sample_lines(new_refs, 6)
    } else {
        sample_lines(updated_refs, 6)
    };

    CompressionSummary::new(ShellPattern::GitFetch, summary, details, Vec::new(), result)
}

pub(super) fn summarize_pull(result: &ShellResult) -> CompressionSummary {
    let lines = combined_lines(result);
    let summary = if lines.iter().any(|line| line.contains("Already up to date")) {
        "ok (up-to-date)".to_string()
    } else if let Some(stats) = summarize_change_stats(&lines) {
        stats
    } else {
        "ok pull".to_string()
    };

    CompressionSummary::new(
        ShellPattern::GitPull,
        summary,
        summarize_stat_lines(&lines),
        Vec::new(),
        result,
    )
}

pub(super) fn summarize_push(result: &ShellResult) -> CompressionSummary {
    let lines = combined_lines(result);
    let summary = push_summary(&lines).unwrap_or_else(|| "ok push".to_string());
    let details = sample_lines(lines.into_iter().filter(|line| line.contains("->")), 3);

    CompressionSummary::new(ShellPattern::GitPush, summary, details, Vec::new(), result)
}

pub(super) fn summarize_checkout(result: &ShellResult) -> CompressionSummary {
    summarize_transition(result, ShellPattern::GitCheckout)
}

pub(super) fn summarize_switch(result: &ShellResult) -> CompressionSummary {
    summarize_transition(result, ShellPattern::GitSwitch)
}

pub(super) fn summarize_commit(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let headline = lines
        .first()
        .cloned()
        .unwrap_or_else(|| "no output".to_string());

    CompressionSummary::new(
        ShellPattern::GitCommit,
        headline,
        sample_lines(lines, 6),
        preview(&result.stderr),
        result,
    )
}

pub(crate) fn summarize_change_stats(lines: &[String]) -> Option<String> {
    lines.iter().find_map(|line| {
        let mut files = None;
        let mut insertions = 0usize;
        let mut deletions = 0usize;

        for part in line.split(',') {
            let trimmed = part.trim();
            if let Some(value) = trimmed.strip_suffix(" files changed") {
                files = value.parse::<usize>().ok();
            } else if let Some(value) = trimmed.strip_suffix(" file changed") {
                files = value.parse::<usize>().ok();
            } else if let Some(value) = trimmed.strip_suffix(" insertions(+)") {
                insertions = value.parse::<usize>().ok()?;
            } else if let Some(value) = trimmed.strip_suffix(" insertion(+)") {
                insertions = value.parse::<usize>().ok()?;
            } else if let Some(value) = trimmed.strip_suffix(" deletions(-)") {
                deletions = value.parse::<usize>().ok()?;
            } else if let Some(value) = trimmed.strip_suffix(" deletion(-)") {
                deletions = value.parse::<usize>().ok()?;
            }
        }

        files.map(|files| format!("ok {files} files, +{insertions}/-{deletions}"))
    })
}

fn summarize_transition(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let lines = combined_lines(result);
    let target = lines
        .first()
        .cloned()
        .unwrap_or_else(|| "no output".to_string());

    CompressionSummary::new(
        pattern,
        format!("transition={target}"),
        sample_lines(lines, 6),
        Vec::new(),
        result,
    )
}

fn summarize_stat_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| line.contains("file changed") || line.contains("files changed"))
        .cloned()
        .collect()
}

pub(crate) fn combined_lines(result: &ShellResult) -> Vec<String> {
    let mut lines = non_empty_lines(&result.stdout);
    if lines.is_empty() {
        return non_empty_lines(&result.stderr);
    }

    for line in non_empty_lines(&result.stderr) {
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    lines
}

fn collect_new_refs(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| {
            if !line.contains("[new ") {
                return None;
            }
            line.split("->")
                .nth(1)
                .map(|ref_name| ref_name.trim().to_string())
        })
        .collect()
}

fn collect_updated_refs(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| {
            if !line.contains("->") || line.contains("[new ") {
                return None;
            }
            Some(line.trim().to_string())
        })
        .collect()
}

fn push_summary(lines: &[String]) -> Option<String> {
    if lines
        .iter()
        .any(|line| line.contains("Everything up-to-date"))
    {
        return Some("ok (up-to-date)".to_string());
    }

    lines.iter().find_map(|line| {
        let ref_update = line.split("->").nth(1)?.trim();
        let branch = ref_update.split_whitespace().last().unwrap_or(ref_update);
        Some(format!("ok {branch}"))
    })
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
