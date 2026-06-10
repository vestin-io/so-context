mod classify;
mod go;
mod javascript;
mod models;
mod parsing;
mod presentation;
mod ruby;

use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use presentation::render_test_presentation;

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    classify::classify(program, args)
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    matches!(
        pattern,
        ShellPattern::Jest
            | ShellPattern::Vitest
            | ShellPattern::PlaywrightTest
            | ShellPattern::GoTest
            | ShellPattern::Rspec
            | ShellPattern::Minitest
    )
    .then(|| summarize(result, pattern))
}

fn summarize(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let presentation = match pattern {
        ShellPattern::Jest => javascript::summarize_js_test(result, "Jest"),
        ShellPattern::Vitest => javascript::summarize_js_test(result, "Vitest"),
        ShellPattern::PlaywrightTest => javascript::summarize_playwright_test(result),
        ShellPattern::GoTest => go::summarize_go_test(result),
        ShellPattern::Rspec => ruby::summarize_rspec(result),
        ShellPattern::Minitest => ruby::summarize_minitest(result),
        _ => unreachable!("unsupported test pattern: {:?}", pattern),
    };

    render_test_presentation(pattern, presentation, result)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
