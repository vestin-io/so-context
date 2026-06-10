use crate::shell::types::{CompressionSummary, ShellPattern, ShellResult};

use super::super::text::{non_empty_lines, preferred_output, preview, sample_lines};

pub(super) fn is_text_excerpt_command(program: &str, args: &[String]) -> bool {
    matches!(program, "sed" | "sh" | "bash" | "zsh")
        && excerpt_line_count_hint(program, args).is_some()
}

pub(super) fn excerpt_line_count_hint(program: &str, args: &[String]) -> Option<usize> {
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

pub(super) fn infer_wget_filename(stderr: &str, args: &[String]) -> Option<String> {
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

pub(super) fn infer_wget_size(stderr: &str) -> Option<String> {
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

pub(super) fn summarize_file_excerpt(
    result: &ShellResult,
    pattern: ShellPattern,
    label: &str,
) -> CompressionSummary {
    let output = authoritative_text_output(result);
    let lines = non_empty_lines(&output);

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

pub(super) fn authoritative_text_output(result: &ShellResult) -> String {
    if !result.stdout.is_empty() {
        result.stdout.clone()
    } else {
        preferred_output(result)
    }
}
