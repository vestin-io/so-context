use crate::shell::types::ShellResult;

pub(super) fn preview(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(5)
        .map(|line| line.trim().to_string())
        .collect()
}

pub(super) fn sample_lines<I>(lines: I, limit: usize) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    lines.into_iter().take(limit).collect()
}

pub(super) fn append_omitted_line(
    details: &mut Vec<String>,
    total: usize,
    shown: usize,
    label: &str,
) {
    if total > shown {
        details.push(format!("+ {} more {label}", total - shown));
    }
}

pub(super) fn truncate_text(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }

    let truncated: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{truncated}…")
}

pub(super) fn compact_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn non_empty_lines(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect()
}

pub(super) fn preferred_output(result: &ShellResult) -> String {
    match (
        result.stdout.trim().is_empty(),
        result.stderr.trim().is_empty(),
    ) {
        (false, true) => result.stdout.clone(),
        (true, false) => result.stderr.clone(),
        (false, false) => format!("{}\n{}", result.stdout, result.stderr),
        (true, true) => String::new(),
    }
}
