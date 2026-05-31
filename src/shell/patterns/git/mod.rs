mod diff;
mod family;
pub(super) mod ops;
pub(super) mod refs;
mod status;
pub(super) mod transport;

use super::super::types::{CompressionSummary, ShellPattern, ShellResult};

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    family::classify(program, args)
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    family::summarize_pattern(result, pattern)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
