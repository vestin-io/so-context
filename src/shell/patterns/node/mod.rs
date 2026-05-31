use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::text::{compact_whitespace, sample_lines, truncate_text};

fn summarize(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let lines = meaningful_lines(&result.stdout);
    let summary = match lines.first() {
        Some(line) => truncate_text(line, 120),
        None => "ok".to_string(),
    };
    let details = if lines.len() <= 1 {
        Vec::new()
    } else {
        sample_lines(lines.into_iter().skip(1), 6)
    };

    CompressionSummary::new(
        pattern,
        summary,
        details,
        filtered_stderr_preview(&result.stderr),
        result,
    )
}

pub(super) fn classify(program: &str, _args: &[String]) -> Option<ShellPattern> {
    match program {
        "npm" => Some(ShellPattern::NodeNpm),
        "pnpm" => Some(ShellPattern::NodePnpm),
        "yarn" => Some(ShellPattern::NodeYarn),
        "bun" => Some(ShellPattern::NodeBun),
        "bunx" | "npx" => Some(ShellPattern::NodeNpx),
        _ => None,
    }
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    matches!(
        pattern,
        ShellPattern::NodeNpm
            | ShellPattern::NodePnpm
            | ShellPattern::NodeYarn
            | ShellPattern::NodeBun
            | ShellPattern::NodeNpx
    )
    .then(|| summarize(result, pattern))
}

fn meaningful_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with('>'))
        .filter(|line| !line.starts_with("npm WARN"))
        .filter(|line| !line.starts_with("npm notice"))
        .filter(|line| !line.starts_with(" WARN "))
        .filter(|line| !line.starts_with("WARN "))
        .filter(|line| !line.starts_with("Done in 0.0"))
        .map(compact_whitespace)
        .filter(|line| !looks_like_progress(line))
        .collect()
}

fn filtered_stderr_preview(stderr: &str) -> Vec<String> {
    let filtered: Vec<String> = stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with("$ "))
        .map(compact_whitespace)
        .collect();
    sample_lines(filtered, 5)
}

fn looks_like_progress(line: &str) -> bool {
    line.starts_with("Packages: ")
        || line.starts_with("Progress: ")
        || line.starts_with("Resolving: ")
        || line.starts_with("Downloading ")
        || line.starts_with("[1/")
        || line.starts_with("[2/")
        || line.starts_with("[3/")
        || line.starts_with("[4/")
        || line.starts_with("Already up to date")
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
