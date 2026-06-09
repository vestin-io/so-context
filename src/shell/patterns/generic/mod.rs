mod analyze;
mod listing;
mod search;

use self::analyze::{SummaryDetails, stream_source, summarize_env_vars, summarize_find_paths};
use self::listing::summarize_ls_entries;
use self::search::summarize_search_hits;
use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::text::{
    append_omitted_line, non_empty_lines, preferred_output, preview, sample_lines, truncate_text,
};

const FALLBACK_PASSTHROUGH_THRESHOLD: usize = 40;
const SCRIPT_PASSTHROUGH_THRESHOLD: usize = 80;
const TEXT_TRANSFORM_PASSTHROUGH_THRESHOLD: usize = 120;
const RG_FILES_PASSTHROUGH_THRESHOLD: usize = 200;

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

fn summarize_rg_files(result: &ShellResult) -> CompressionSummary {
    let files = exact_output_lines(&result.stdout);
    if files.len() <= RG_FILES_PASSTHROUGH_THRESHOLD {
        return CompressionSummary::plain(
            ShellPattern::RgFiles,
            String::new(),
            files,
            preview(&result.stderr),
            result,
        );
    }

    let shown = RG_FILES_PASSTHROUGH_THRESHOLD;
    let mut details = sample_lines(files, shown);
    append_omitted_line(&mut details, result.stdout.lines().count(), shown, "files");

    CompressionSummary::plain(
        ShellPattern::RgFiles,
        String::new(),
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_grep(result: &ShellResult) -> CompressionSummary {
    summarize_search_output(result, ShellPattern::Grep)
}

fn summarize_search_output(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let hits = result
        .stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect::<Vec<_>>();
    let query = extract_search_query(result.invocation.args());
    let SummaryDetails { summary, details } = summarize_search_hits(&hits, query.as_deref());

    CompressionSummary::plain(pattern, summary, details, preview(&result.stderr), result)
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

fn summarize_text_excerpt(result: &ShellResult) -> CompressionSummary {
    CompressionSummary::plain(
        ShellPattern::TextExcerpt,
        String::new(),
        exact_output_lines(&preferred_output(result)),
        preview(&result.stderr),
        result,
    )
}

pub(super) fn summarize_unknown(result: &ShellResult) -> CompressionSummary {
    summarize_fallback(result, ShellPattern::Unknown)
}

pub(super) fn summarize_fallback(
    result: &ShellResult,
    pattern: ShellPattern,
) -> CompressionSummary {
    let program = result.invocation.program();
    let output = preferred_output(result);
    let details = non_empty_lines(&output);
    let threshold = fallback_passthrough_threshold(program);

    if should_plain_passthrough(&output, details.len(), threshold) {
        return CompressionSummary::plain(
            pattern,
            String::new(),
            exact_output_lines(&output),
            Vec::new(),
            result,
        );
    }

    let shown = details.len().min(threshold);
    let mut rendered_details = sample_lines(details.clone(), threshold);
    append_omitted_line(&mut rendered_details, details.len(), shown, "lines");

    CompressionSummary::new(
        pattern,
        fallback_summary(result, program, &details),
        rendered_details,
        preview(&result.stderr),
        result,
    )
}

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    if is_text_excerpt_command(program, args) {
        return Some(ShellPattern::TextExcerpt);
    }

    match program {
        "ls" => Some(ShellPattern::Ls),
        "find" => Some(ShellPattern::Find),
        "rg" => {
            if args.iter().any(|arg| arg == "--files") {
                Some(ShellPattern::RgFiles)
            } else {
                Some(ShellPattern::Rg)
            }
        }
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
        ShellPattern::RgFiles => summarize_rg_files(result),
        ShellPattern::Grep => summarize_grep(result),
        ShellPattern::Curl => summarize_curl(result),
        ShellPattern::Wget => summarize_wget(result),
        ShellPattern::Env => summarize_env(result),
        ShellPattern::Cat => summarize_cat(result),
        ShellPattern::Head => summarize_head(result),
        ShellPattern::Tail => summarize_tail(result),
        ShellPattern::TextExcerpt => summarize_text_excerpt(result),
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

fn extract_search_query(args: &[String]) -> Option<String> {
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }

        match arg.as_str() {
            "-e" | "--regexp" | "-f" | "--file" | "-g" | "--glob" | "-t" | "--type" | "-A"
            | "-B" | "-C" | "-m" | "--max-count" => {
                skip_next = true;
                continue;
            }
            _ => {}
        }

        if arg.starts_with('-') {
            continue;
        }

        return Some(arg.trim().to_string());
    }

    None
}

fn looks_like_json(text: &str) -> bool {
    (text.starts_with('{') && text.ends_with('}'))
        || (text.starts_with('[') && text.ends_with(']'))
        || (text.starts_with('"') && text.ends_with('"') && text.len() >= 2)
}

fn fallback_passthrough_threshold(program: &str) -> usize {
    match program {
        "jq" | "sed" => TEXT_TRANSFORM_PASSTHROUGH_THRESHOLD,
        "sh" | "bash" | "zsh" => SCRIPT_PASSTHROUGH_THRESHOLD,
        _ => FALLBACK_PASSTHROUGH_THRESHOLD,
    }
}

fn fallback_summary(result: &ShellResult, program: &str, details: &[String]) -> String {
    let bytes = result.render_full().len();
    let stdout_lines = non_empty_lines(&result.stdout).len();
    let stderr_lines = non_empty_lines(&result.stderr).len();
    let line_count = details.len();

    if line_count == 0 {
        return "no output".to_string();
    }

    if program == "jq" && looks_like_json(result.stdout.trim()) {
        return if line_count == 1 {
            format!("JSON output | {bytes}B")
        } else {
            format!("JSON output | {line_count} lines | {bytes}B")
        };
    }

    if stderr_lines > 0 && stdout_lines == 0 {
        return format!("{stderr_lines} stderr lines | {bytes}B");
    }

    if stdout_lines > 0 && stderr_lines == 0 {
        return if line_count == 1 {
            format!("1 line | {bytes}B")
        } else {
            format!("{line_count} lines | {bytes}B")
        };
    }

    let lead = truncate_text(details.first().map(String::as_str).unwrap_or(""), 80);
    format!("{line_count} lines | {stderr_lines} stderr lines | {lead}")
}

fn should_plain_passthrough(output: &str, line_count: usize, threshold: usize) -> bool {
    !output.trim().is_empty() && line_count > 0 && line_count <= threshold
}

fn exact_output_lines(output: &str) -> Vec<String> {
    output.lines().map(|line| line.to_string()).collect()
}

fn is_text_excerpt_command(program: &str, args: &[String]) -> bool {
    matches!(program, "sed" | "sh" | "bash" | "zsh")
        && excerpt_line_count_hint(program, args).is_some()
}

fn excerpt_line_count_hint(program: &str, args: &[String]) -> Option<usize> {
    if program == "sed" {
        return parse_sed_excerpt_args(args);
    }

    shell_c_command(args).and_then(parse_shell_excerpt_command)
}

fn parse_sed_excerpt_args(args: &[String]) -> Option<usize> {
    let has_print_range = args.iter().any(|arg| parse_sed_range(arg).is_some());
    let has_file = args.iter().any(|arg| !arg.starts_with('-'));
    has_print_range.then_some(())?;
    has_file.then_some(())?;
    args.iter().find_map(|arg| parse_sed_range(arg))
}

fn shell_c_command(args: &[String]) -> Option<&str> {
    let command_index = args
        .iter()
        .position(|arg| arg.starts_with('-') && arg.contains('c'))?;
    args.get(command_index + 1).map(String::as_str)
}

fn parse_shell_excerpt_command(command: &str) -> Option<usize> {
    let normalized = command.trim();
    let has_line_number_prefix =
        normalized.starts_with("nl -ba ") || normalized.contains(" nl -ba ");
    let has_sed_range = normalized.contains("sed -n");
    if !(has_line_number_prefix && has_sed_range) {
        return None;
    }

    let sed_segment = normalized
        .split('|')
        .map(str::trim)
        .find(|part| part.starts_with("sed -n"))?;
    parse_sed_command_range(sed_segment)
}

fn parse_sed_command_range(command: &str) -> Option<usize> {
    let mut tokens = command
        .split_whitespace()
        .map(|token| token.trim_matches(|ch| ch == '\'' || ch == '"'));
    tokens.find_map(parse_sed_range)
}

fn parse_sed_range(token: &str) -> Option<usize> {
    let body = token.strip_suffix('p')?;
    let (start, end) = body.split_once(',')?;
    let start: usize = start.parse().ok()?;
    let end: usize = end.parse().ok()?;
    (end >= start).then_some(end - start + 1)
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
