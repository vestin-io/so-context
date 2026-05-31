mod classify;
mod summarize;

use super::super::types::{CompressionSummary, ShellPattern, ShellResult};

#[cfg_attr(not(test), allow(dead_code))]
fn summarize(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    summarize::summarize(result, pattern)
}

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    classify::classify(program, args)
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    summarize::summarize_pattern(result, pattern)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
