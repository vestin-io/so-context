use super::super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::super::text::{
    compact_whitespace, non_empty_lines, preview, sample_lines, truncate_text,
};
use super::classify::logs_target;

pub(super) fn summarize(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    match pattern {
        ShellPattern::KubectlPods => summarize_pods(result),
        ShellPattern::KubectlServices => summarize_services(result),
        ShellPattern::KubectlLogs => summarize_logs(result),
        ShellPattern::KubectlDescribe => summarize_describe(result),
        ShellPattern::KubectlApply => summarize_apply(result),
        _ => CompressionSummary::new(
            pattern,
            truncate_text(&compact_whitespace(&result.stdout), 120),
            Vec::new(),
            preview(&result.stderr),
            result,
        ),
    }
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    matches!(
        pattern,
        ShellPattern::KubectlPods
            | ShellPattern::KubectlServices
            | ShellPattern::KubectlLogs
            | ShellPattern::KubectlDescribe
            | ShellPattern::KubectlApply
    )
    .then(|| summarize(result, pattern))
}

fn summarize_pods(result: &ShellResult) -> CompressionSummary {
    let rows = table_rows(&result.stdout);
    let running = rows
        .iter()
        .filter(|cols| cols.get(2).is_some_and(|s| *s == "Running"))
        .count();
    let pending = rows
        .iter()
        .filter(|cols| cols.get(2).is_some_and(|s| *s == "Pending"))
        .count();
    let failed = rows
        .iter()
        .filter(|cols| {
            cols.get(2).is_some_and(|s| {
                matches!(
                    *s,
                    "Error" | "Failed" | "CrashLoopBackOff" | "ImagePullBackOff"
                )
            })
        })
        .count();
    let restarts: usize = rows
        .iter()
        .filter_map(|cols| cols.get(3).and_then(|v| v.parse::<usize>().ok()))
        .sum();
    let details = rows
        .iter()
        .take(8)
        .map(|cols| {
            let name = cols.first().copied().unwrap_or("pod");
            let status = cols.get(2).copied().unwrap_or("unknown");
            let restart_text = cols
                .get(3)
                .filter(|value| **value != "0")
                .map(|value| format!(" ({value} restarts)"))
                .unwrap_or_default();
            format!("{name} {status}{restart_text}")
        })
        .collect();

    CompressionSummary::new(
        ShellPattern::KubectlPods,
        format!(
            "{} pods | running={running} pending={pending} failed={failed} restarts={restarts}",
            rows.len()
        ),
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_services(result: &ShellResult) -> CompressionSummary {
    let rows = table_rows(&result.stdout);
    let details = rows
        .iter()
        .take(8)
        .map(|cols| {
            let name = cols.first().copied().unwrap_or("service");
            let service_type = cols.get(1).copied().unwrap_or("ClusterIP");
            let ports = cols.get(4).copied().unwrap_or("-");
            format!("{name} {service_type} [{ports}]")
        })
        .collect();

    CompressionSummary::new(
        ShellPattern::KubectlServices,
        format!("{} services", rows.len()),
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_logs(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&result.stdout);
    let target = logs_target(result.invocation.args()).unwrap_or_else(|| "pod".to_string());
    let errors: Vec<String> = lines
        .iter()
        .filter(|line| line.to_ascii_lowercase().contains("error"))
        .cloned()
        .collect();
    let warnings = lines
        .iter()
        .filter(|line| line.to_ascii_lowercase().contains("warn"))
        .count();
    let error_count = errors.len();
    let details = sample_lines(lines.iter().cloned(), 8);
    let summary = if error_count > 0 || warnings > 0 {
        format!("logs for {target}: {error_count} errors, {warnings} warnings")
    } else {
        format!("logs for {target}: {} lines", lines.len())
    };

    CompressionSummary::new(
        ShellPattern::KubectlLogs,
        summary,
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_describe(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&result.stdout);
    let mut details: Vec<String> = lines
        .iter()
        .filter(|line| {
            line.starts_with("Name:")
                || line.starts_with("Namespace:")
                || line.starts_with("Status:")
                || line.starts_with("Node:")
                || line.starts_with("Replicas:")
                || line.starts_with("Image:")
                || line.starts_with("Reason:")
        })
        .map(|line| compact_whitespace(line))
        .take(8)
        .collect();
    details.extend(describe_event_lines(&lines));
    let name = describe_value(&details, "Name:");
    let status = describe_value(&details, "Status:");
    let summary = match (name, status) {
        (Some(name), Some(status)) => format!("{name} {status}"),
        _ => details
            .first()
            .cloned()
            .unwrap_or_else(|| format!("describe: {} lines", lines.len())),
    };

    CompressionSummary::new(
        ShellPattern::KubectlDescribe,
        summary,
        details.into_iter().skip(1).collect(),
        preview(&result.stderr),
        result,
    )
}

fn summarize_apply(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&result.stdout);
    let mut created = 0usize;
    let mut configured = 0usize;
    let mut unchanged = 0usize;
    let mut deleted = 0usize;
    for line in &lines {
        if line.ends_with(" created") {
            created += 1;
        } else if line.ends_with(" configured") {
            configured += 1;
        } else if line.ends_with(" unchanged") {
            unchanged += 1;
        } else if line.ends_with(" deleted") {
            deleted += 1;
        }
    }
    let mut parts = Vec::new();
    if created > 0 {
        parts.push(format!("{created} created"));
    }
    if configured > 0 {
        parts.push(format!("{configured} configured"));
    }
    if unchanged > 0 {
        parts.push(format!("{unchanged} unchanged"));
    }
    if deleted > 0 {
        parts.push(format!("{deleted} deleted"));
    }

    CompressionSummary::new(
        ShellPattern::KubectlApply,
        if parts.is_empty() {
            format!("kubectl apply: {} lines", lines.len())
        } else {
            format!("kubectl apply: {}", parts.join(", "))
        },
        sample_lines(lines, 8),
        preview(&result.stderr),
        result,
    )
}

fn describe_value<'a>(details: &'a [String], key: &str) -> Option<&'a str> {
    details
        .iter()
        .find_map(|line| line.strip_prefix(key).map(str::trim))
}

fn describe_event_lines(lines: &[String]) -> Vec<String> {
    let mut events = Vec::new();
    let mut in_events = false;
    for line in lines {
        if line.starts_with("Events:") {
            in_events = true;
            continue;
        }
        if !in_events {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Type") || trimmed.starts_with("----") {
            continue;
        }
        events.push(compact_whitespace(line));
    }
    let events: Vec<String> = events.into_iter().rev().take(2).collect();
    events.into_iter().rev().collect()
}

fn table_rows(stdout: &str) -> Vec<Vec<&str>> {
    stdout
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.split_whitespace().collect())
        .collect()
}
