mod aggregate;

use std::collections::BTreeSet;

use aggregate::{
    count_by_key, fallback_lint_summary, group_rules_by_file, normalized_lines, noun,
    parse_biome_issue, parse_eslint_issue, parse_golangci_issue, parse_golangci_json_output,
    render_golangci_details, render_top_rules_and_files,
};

use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::argv::{first_positional, positional_args};
use super::text::{
    append_omitted_line, compact_whitespace, preferred_output, preview, sample_lines,
};

const DETAIL_LIMIT: usize = 8;

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    match program {
        "eslint" => Some(ShellPattern::LintEslint),
        "biome" => Some(ShellPattern::LintBiome),
        "prettier" => Some(ShellPattern::LintPrettier),
        "golangci-lint" => Some(ShellPattern::LintGolangci),
        "go" => classify_go(args),
        "npx" | "bunx" => classify_wrapped_tool(first_positional(args, &[])),
        "bun" => classify_bun(args),
        "npm" => classify_npm(args),
        "pnpm" => classify_pnpm(args),
        "yarn" => classify_yarn(args),
        _ => None,
    }
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    matches!(
        pattern,
        ShellPattern::LintEslint
            | ShellPattern::LintBiome
            | ShellPattern::LintPrettier
            | ShellPattern::LintGolangci
    )
    .then(|| summarize(result, pattern))
}

fn summarize(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    match pattern {
        ShellPattern::LintEslint => summarize_eslint(result),
        ShellPattern::LintBiome => summarize_biome(result),
        ShellPattern::LintPrettier => summarize_prettier(result),
        ShellPattern::LintGolangci => summarize_golangci(result),
        _ => CompressionSummary::new(
            pattern,
            compact_whitespace(&preferred_output(result)),
            Vec::new(),
            preview(&result.stderr),
            result,
        ),
    }
}

fn summarize_eslint(result: &ShellResult) -> CompressionSummary {
    let lines = normalized_lines(result);
    let issues = lines
        .iter()
        .filter_map(|line| parse_eslint_issue(line))
        .collect::<Vec<_>>();

    if issues.is_empty() {
        return fallback_lint_summary(
            result,
            ShellPattern::LintEslint,
            "ESLint: No issues found",
            "ESLint: failed",
        );
    }

    let error_count = issues
        .iter()
        .filter(|issue| issue.severity.eq_ignore_ascii_case("error"))
        .count();
    let warning_count = issues.len().saturating_sub(error_count);
    let file_count = issues
        .iter()
        .map(|issue| issue.file.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let summary = format!(
        "ESLint: {} {}, {} {} in {} {}",
        error_count,
        noun(error_count, "error", "errors"),
        warning_count,
        noun(warning_count, "warning", "warnings"),
        file_count,
        noun(file_count, "file", "files"),
    );
    let rule_counts = count_by_key(issues.iter().map(|issue| issue.rule.clone()));
    let file_rules = group_rules_by_file(issues.iter().map(|issue| (&issue.file, &issue.rule)));

    CompressionSummary::plain(
        ShellPattern::LintEslint,
        summary,
        render_top_rules_and_files(&rule_counts, &file_rules),
        preview(&result.stderr),
        result,
    )
}

fn summarize_biome(result: &ShellResult) -> CompressionSummary {
    let lines = normalized_lines(result);
    let issues = lines
        .iter()
        .filter_map(|line| parse_biome_issue(line))
        .collect::<Vec<_>>();

    if issues.is_empty() {
        return fallback_lint_summary(
            result,
            ShellPattern::LintBiome,
            "Biome: No issues found",
            "Biome: failed",
        );
    }

    let file_count = issues
        .iter()
        .map(|issue| issue.file.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let summary = format!("Biome: {} issues in {} files", issues.len(), file_count);
    let rule_counts = count_by_key(issues.iter().map(|issue| issue.rule.clone()));
    let file_rules = group_rules_by_file(issues.iter().map(|issue| (&issue.file, &issue.rule)));

    CompressionSummary::plain(
        ShellPattern::LintBiome,
        summary,
        render_top_rules_and_files(&rule_counts, &file_rules),
        preview(&result.stderr),
        result,
    )
}

fn summarize_prettier(result: &ShellResult) -> CompressionSummary {
    let lines = normalized_lines(result);
    let check_mode = is_prettier_check_mode(result);
    let files = lines
        .iter()
        .filter_map(|line| prettier_file_line(line))
        .collect::<Vec<_>>();
    let summary = if preferred_output(result).trim().is_empty() {
        "Prettier: no output".to_string()
    } else if files.is_empty() && result.exit_code == 0 {
        "Prettier: All files formatted correctly".to_string()
    } else if !files.is_empty() && check_mode {
        format!(
            "Prettier: {} {} need{} formatting",
            files.len(),
            noun(files.len(), "file", "files"),
            if files.len() == 1 { "s" } else { "" }
        )
    } else if !files.is_empty() {
        format!(
            "Prettier: {} {} formatted",
            files.len(),
            noun(files.len(), "file", "files")
        )
    } else {
        lines
            .first()
            .map(|line| sanitize_prettier_message(line))
            .unwrap_or_else(|| "Prettier: failed".to_string())
    };
    let total_files = files.len();
    let shown = total_files.min(DETAIL_LIMIT);
    let mut details = sample_lines(files, DETAIL_LIMIT);
    append_omitted_line(&mut details, total_files, shown, "files");

    CompressionSummary::new(
        ShellPattern::LintPrettier,
        summary,
        details,
        preview(&result.stderr),
        result,
    )
}

fn is_prettier_check_mode(result: &ShellResult) -> bool {
    result
        .invocation
        .args()
        .iter()
        .any(|arg| matches!(arg.as_str(), "--check" | "-c" | "--list-different"))
}

fn prettier_file_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("Checking formatting")
        || trimmed.starts_with("All matched")
        || trimmed.starts_with("Code style issues found")
        || trimmed.starts_with("Ignored unknown option")
        || trimmed.starts_with("[error]")
    {
        return None;
    }

    let trimmed = trimmed
        .strip_prefix("[warn] ")
        .or_else(|| trimmed.strip_prefix("[write] "))
        .unwrap_or(trimmed);
    looks_like_file_path(trimmed).then(|| trimmed.to_string())
}

fn sanitize_prettier_message(line: &str) -> String {
    line.trim()
        .strip_prefix("[warn] ")
        .or_else(|| line.trim().strip_prefix("[error] "))
        .unwrap_or_else(|| line.trim())
        .to_string()
}

fn looks_like_file_path(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    [
        ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".json", ".md", ".css", ".scss", ".html",
        ".yml", ".yaml",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

fn summarize_golangci(result: &ShellResult) -> CompressionSummary {
    let output = preferred_output(result);
    let issues = parse_golangci_json_output(&output).unwrap_or_else(|| {
        normalized_lines(result)
            .iter()
            .filter_map(|line| parse_golangci_issue(line))
            .collect::<Vec<_>>()
    });

    if issues.is_empty() {
        return fallback_lint_summary(
            result,
            ShellPattern::LintGolangci,
            "golangci-lint: No issues found",
            "golangci-lint: failed",
        );
    }

    let file_count = issues
        .iter()
        .map(|issue| issue.file.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    let summary = format!(
        "golangci-lint: {} issues in {} files",
        issues.len(),
        file_count
    );

    CompressionSummary::plain(
        ShellPattern::LintGolangci,
        summary,
        render_golangci_details(&issues),
        preview(&result.stderr),
        result,
    )
}

fn classify_wrapped_tool(tool: Option<&str>) -> Option<ShellPattern> {
    match tool {
        Some("eslint") => Some(ShellPattern::LintEslint),
        Some("biome") => Some(ShellPattern::LintBiome),
        Some("prettier") => Some(ShellPattern::LintPrettier),
        Some("golangci-lint") => Some(ShellPattern::LintGolangci),
        _ => None,
    }
}

fn classify_go(args: &[String]) -> Option<ShellPattern> {
    match go_tool_name(args) {
        Some("golangci-lint") => Some(ShellPattern::LintGolangci),
        _ => None,
    }
}

fn classify_bun(args: &[String]) -> Option<ShellPattern> {
    let positionals = positional_args(args, &["--cwd"]);
    match positionals.first().copied() {
        Some("x") => classify_wrapped_tool(positionals.get(1).copied()),
        _ => None,
    }
}

fn classify_npm(args: &[String]) -> Option<ShellPattern> {
    let positionals = positional_args(args, &["--prefix", "--cache", "-w", "--workspace"]);
    match positionals.first().copied() {
        Some("exec") | Some("x") | Some("run") | Some("run-script") => {
            classify_wrapped_tool(positionals.get(1).copied())
        }
        other => classify_wrapped_tool(other),
    }
}

fn classify_pnpm(args: &[String]) -> Option<ShellPattern> {
    let positionals = positional_args(args, &["--filter", "-C", "--dir"]);
    match positionals.first().copied() {
        Some("exec") | Some("dlx") | Some("run") | Some("run-script") => {
            classify_wrapped_tool(positionals.get(1).copied())
        }
        other => classify_wrapped_tool(other),
    }
}

fn classify_yarn(args: &[String]) -> Option<ShellPattern> {
    let positionals = positional_args(args, &["--cwd"]);
    match positionals.first().copied() {
        Some("dlx") | Some("run") => classify_wrapped_tool(positionals.get(1).copied()),
        other => classify_wrapped_tool(other),
    }
}

fn go_tool_name(args: &[String]) -> Option<&str> {
    let mut index = 0usize;

    while index < args.len() {
        match args[index].as_str() {
            "tool" => {
                index += 1;
                while index < args.len() && args[index].starts_with('-') {
                    if go_tool_flag_takes_value(args[index].as_str()) {
                        index += 1;
                    }
                    index += 1;
                }
                return args.get(index).map(String::as_str);
            }
            arg if arg.starts_with('-') => {
                if go_global_flag_takes_value(arg) {
                    index += 1;
                }
            }
            _ => return None,
        }
        index += 1;
    }

    None
}

fn go_global_flag_takes_value(flag: &str) -> bool {
    matches!(flag, "-C" | "-overlay" | "-modfile" | "-mod")
}

fn go_tool_flag_takes_value(flag: &str) -> bool {
    matches!(flag, "-modfile")
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
