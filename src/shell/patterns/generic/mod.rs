mod analyze;
mod listing;
mod search;

use self::analyze::{SummaryDetails, stream_source, summarize_env_vars, summarize_find_paths};
use self::listing::summarize_ls_entries;
use self::search::summarize_search_hits;
use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::text::{non_empty_lines, preferred_output, preview, sample_lines};

fn summarize_ls(result: &ShellResult) -> CompressionSummary {
    let entries = non_empty_lines(&result.stdout);
    let SummaryDetails { summary, details } =
        summarize_ls_entries(&entries, result.invocation.args());

    CompressionSummary::new(
        ShellPattern::Ls,
        summary,
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_find(result: &ShellResult) -> CompressionSummary {
    let paths = non_empty_lines(&result.stdout);
    let SummaryDetails { summary, details } = summarize_find_paths(&paths);

    CompressionSummary::new(
        ShellPattern::Find,
        summary,
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_rg(result: &ShellResult) -> CompressionSummary {
    summarize_search_output(result, ShellPattern::Rg)
}

fn summarize_grep(result: &ShellResult) -> CompressionSummary {
    summarize_search_output(result, ShellPattern::Grep)
}

fn summarize_search_output(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let hits = non_empty_lines(&result.stdout);
    let SummaryDetails { summary, details } = summarize_search_hits(&hits);

    CompressionSummary::new(pattern, summary, details, preview(&result.stderr), result)
}

fn summarize_curl(result: &ShellResult) -> CompressionSummary {
    let raw = result.stdout.trim();
    if looks_like_json(raw) {
        return CompressionSummary::new(
            ShellPattern::Curl,
            format!("json response | {}B", raw.len()),
            sample_lines(non_empty_lines(raw), 12),
            preview(&result.stderr),
            result,
        );
    }

    summarize_stream_like(result, ShellPattern::Curl)
}

fn summarize_wget(result: &ShellResult) -> CompressionSummary {
    if !result.stdout.trim().is_empty() {
        return summarize_stream_like(result, ShellPattern::Wget);
    }

    let source = stream_source(result.invocation.args())
        .unwrap_or_else(|| result.invocation.program().to_string());
    let stderr_lines = non_empty_lines(&result.stderr);
    let filename = infer_wget_filename(&result.stderr, result.invocation.args())
        .unwrap_or_else(|| "download".to_string());
    let size = infer_wget_size(&result.stderr).unwrap_or_else(|| "?".to_string());

    CompressionSummary::new(
        ShellPattern::Wget,
        format!("{source} ok | {filename} | {size}"),
        Vec::new(),
        sample_lines(stderr_lines, 5),
        result,
    )
}

fn summarize_env(result: &ShellResult) -> CompressionSummary {
    let vars = non_empty_lines(&result.stdout);
    let details = summarize_env_vars(&vars);
    let shown = details.len();

    CompressionSummary::new(
        ShellPattern::Env,
        format!("vars={}; shown={shown}", vars.len()),
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_cat(result: &ShellResult) -> CompressionSummary {
    summarize_file_excerpt(result, ShellPattern::Cat, "cat")
}

fn summarize_head(result: &ShellResult) -> CompressionSummary {
    summarize_file_excerpt(result, ShellPattern::Head, "head")
}

fn summarize_tail(result: &ShellResult) -> CompressionSummary {
    summarize_file_excerpt(result, ShellPattern::Tail, "tail")
}

pub(super) fn summarize_unknown(result: &ShellResult) -> CompressionSummary {
    summarize_fallback(result, ShellPattern::Unknown)
}

pub(super) fn summarize_fallback(
    result: &ShellResult,
    pattern: ShellPattern,
) -> CompressionSummary {
    let details = non_empty_lines(&result.stdout);

    CompressionSummary::new(
        pattern,
        format!(
            "stdout_lines={}; stderr_lines={}",
            result
                .stdout
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count(),
            result
                .stderr
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count()
        ),
        sample_lines(details, 8),
        preview(&result.stderr),
        result,
    )
}

pub(super) fn classify(program: &str, _args: &[String]) -> Option<ShellPattern> {
    match program {
        "ls" => Some(ShellPattern::Ls),
        "find" => Some(ShellPattern::Find),
        "rg" => Some(ShellPattern::Rg),
        "grep" => Some(ShellPattern::Grep),
        "curl" => Some(ShellPattern::Curl),
        "wget" => Some(ShellPattern::Wget),
        "env" => Some(ShellPattern::Env),
        "cat" => Some(ShellPattern::Cat),
        "head" => Some(ShellPattern::Head),
        "tail" => Some(ShellPattern::Tail),
        _ => None,
    }
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    Some(match pattern {
        ShellPattern::Ls => summarize_ls(result),
        ShellPattern::Find => summarize_find(result),
        ShellPattern::Rg => summarize_rg(result),
        ShellPattern::Grep => summarize_grep(result),
        ShellPattern::Curl => summarize_curl(result),
        ShellPattern::Wget => summarize_wget(result),
        ShellPattern::Env => summarize_env(result),
        ShellPattern::Cat => summarize_cat(result),
        ShellPattern::Head => summarize_head(result),
        ShellPattern::Tail => summarize_tail(result),
        ShellPattern::Unknown => summarize_unknown(result),
        _ => return None,
    })
}

fn summarize_stream_like(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let body = non_empty_lines(&result.stdout);
    let stderr_lines = non_empty_lines(&result.stderr);
    let source = stream_source(result.invocation.args())
        .unwrap_or_else(|| result.invocation.program().to_string());
    let summary = format!(
        "{source} ok | {} lines | {}B",
        body.len(),
        result.stdout.len()
    );
    let details = if body.len() > 20 {
        let body_len = body.len();
        let mut lines = sample_lines(body, 10);
        lines.push(format!("+ {} more lines", body_len - 10));
        lines
    } else {
        sample_lines(body, 10)
    };

    CompressionSummary::new(
        pattern,
        if stderr_lines.is_empty() {
            summary
        } else {
            format!("{summary}; stderr_lines={}", stderr_lines.len())
        },
        details,
        sample_lines(stderr_lines, 5),
        result,
    )
}

fn looks_like_json(text: &str) -> bool {
    (text.starts_with('{') && text.ends_with('}'))
        || (text.starts_with('[') && text.ends_with(']'))
        || (text.starts_with('"') && text.ends_with('"') && text.len() >= 2)
}

fn infer_wget_filename(stderr: &str, args: &[String]) -> Option<String> {
    for (index, arg) in args.iter().enumerate() {
        if (arg == "-O" || arg == "--output-document") && index + 1 < args.len() {
            return Some(args[index + 1].clone());
        }
        if let Some(name) = arg.strip_prefix("-O")
            && !name.is_empty()
        {
            return Some(name.to_string());
        }
    }

    for line in stderr.lines() {
        if line.contains("Saving to") || line.contains("Sauvegarde en") {
            let quoted = line
                .split(['\'', '«'])
                .nth(1)
                .map(|part| part.trim_end_matches(['\'', '»']).trim().to_string());
            if quoted.is_some() {
                return quoted;
            }
        }
    }

    args.iter()
        .rev()
        .find(|arg| !arg.starts_with('-'))
        .and_then(|url| {
            url.rsplit('/').next().map(|name| {
                name.split('?')
                    .next()
                    .filter(|part| !part.is_empty())
                    .unwrap_or("index.html")
                    .to_string()
            })
        })
}

fn infer_wget_size(stderr: &str) -> Option<String> {
    stderr.lines().find_map(|line| {
        let normalized = line.trim();
        if normalized.contains("saved [") {
            normalized
                .split('[')
                .nth(1)
                .and_then(|rest| rest.split(']').next())
                .map(|size| size.trim().to_string())
        } else {
            None
        }
    })
}

fn summarize_file_excerpt(
    result: &ShellResult,
    pattern: ShellPattern,
    label: &str,
) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));

    CompressionSummary::new(
        pattern,
        format!(
            "{label}_lines={}; bytes={}",
            lines.len(),
            result.stdout.len()
        ),
        sample_lines(lines, 8),
        preview(&result.stderr),
        result,
    )
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
