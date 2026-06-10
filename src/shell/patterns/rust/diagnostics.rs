use super::collect_error_blocks;
use crate::shell::types::ShellResult;

use super::super::text::{preview, sample_lines};

pub(super) fn build_like_details(lines: &[String], errors: usize, warnings: usize) -> Vec<String> {
    if errors == 0 && warnings == 0 {
        lines
            .iter()
            .find(|line| line.contains("Finished"))
            .cloned()
            .into_iter()
            .collect()
    } else {
        sample_lines(filter_actionable_lines(lines), 6)
    }
}

pub(super) fn build_like_stderr_preview(
    result: &ShellResult,
    lines: &[String],
    errors: usize,
    warnings: usize,
) -> Vec<String> {
    if result.exit_code == 0 && errors == 0 && warnings == 0 {
        return Vec::new();
    }

    let actionable: Vec<String> = lines
        .iter()
        .filter(|line| line.starts_with("error") || line.starts_with("warning"))
        .cloned()
        .collect();
    if actionable.is_empty() {
        preview(&result.stderr)
    } else {
        sample_lines(actionable, 6)
    }
}

fn filter_actionable_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| {
            line.starts_with("error")
                || line.starts_with("warning")
                || line.contains("Finished")
                || line.contains("Installed package")
        })
        .cloned()
        .collect()
}

pub(super) fn filter_test_actionable_lines(lines: &[String]) -> Vec<String> {
    let blocks = collect_error_blocks(lines);
    if !blocks.is_empty() {
        return blocks;
    }

    filter_actionable_lines(lines)
}

pub(super) fn count_error_lines(lines: &[String]) -> usize {
    lines
        .iter()
        .filter(|line| line.starts_with("error") || line.contains(": error["))
        .count()
}

pub(super) fn count_warning_lines(lines: &[String]) -> usize {
    lines
        .iter()
        .filter(|line| line.starts_with("warning") || line.contains(": warning["))
        .count()
}
