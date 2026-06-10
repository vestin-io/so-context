mod diagnostics;
mod pytest;

use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::argv::positional_args;
use super::text::{
    append_omitted_line, compact_whitespace, non_empty_lines, preferred_output, preview,
    sample_lines, truncate_text,
};

const DETAIL_LIMIT: usize = 8;

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    match program {
        "pip" | "pip3" => Some(ShellPattern::PythonPip),
        "uv" => classify_uv(args),
        "poetry" => classify_poetry(args),
        "pytest" | "py.test" => Some(ShellPattern::PythonPytest),
        "ruff" => Some(ShellPattern::PythonRuff),
        "mypy" => Some(ShellPattern::PythonMypy),
        "python" | "python3" | "py" => classify_python_module(args),
        _ => None,
    }
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    matches!(
        pattern,
        ShellPattern::PythonPip
            | ShellPattern::PythonUv
            | ShellPattern::PythonPoetry
            | ShellPattern::PythonPytest
            | ShellPattern::PythonRuff
            | ShellPattern::PythonMypy
    )
    .then(|| summarize(result, pattern))
}

fn summarize(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    match pattern {
        ShellPattern::PythonPip => summarize_package_tool(result, pattern, "pip"),
        ShellPattern::PythonUv => summarize_package_tool(result, pattern, "uv"),
        ShellPattern::PythonPoetry => summarize_package_tool(result, pattern, "poetry"),
        ShellPattern::PythonPytest => pytest::summarize_pytest(result),
        ShellPattern::PythonRuff => diagnostics::summarize_ruff(result),
        ShellPattern::PythonMypy => diagnostics::summarize_mypy(result),
        _ => CompressionSummary::new(
            pattern,
            truncate_text(&compact_whitespace(&preferred_output(result)), 120),
            Vec::new(),
            preview(&result.stderr),
            result,
        ),
    }
}

fn classify_uv(args: &[String]) -> Option<ShellPattern> {
    let positionals = positional_args(args, &["--project", "--directory", "--python"]);
    match positionals.first().copied() {
        Some("run") => classify_runner_tool(&positionals[1..]).or(Some(ShellPattern::PythonUv)),
        Some(_) | None => Some(ShellPattern::PythonUv),
    }
}

fn classify_poetry(args: &[String]) -> Option<ShellPattern> {
    let positionals = positional_args(args, &["-C", "--directory", "--project"]);
    match positionals.first().copied() {
        Some("run") => classify_runner_tool(&positionals[1..]).or(Some(ShellPattern::PythonPoetry)),
        Some(_) | None => Some(ShellPattern::PythonPoetry),
    }
}

fn classify_python_module(args: &[String]) -> Option<ShellPattern> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "-m" {
            return iter.next().and_then(|module| classify_module_name(module));
        }
        if let Some(module) = arg.strip_prefix("-m") {
            return classify_module_name(module);
        }
    }
    None
}

fn classify_module_name(module: &str) -> Option<ShellPattern> {
    match module {
        "pip" => Some(ShellPattern::PythonPip),
        "pytest" => Some(ShellPattern::PythonPytest),
        "ruff" => Some(ShellPattern::PythonRuff),
        "mypy" => Some(ShellPattern::PythonMypy),
        _ => None,
    }
}

fn classify_runner_tool(positionals: &[&str]) -> Option<ShellPattern> {
    match positionals.first().copied() {
        Some("pip") | Some("pip3") => Some(ShellPattern::PythonPip),
        Some("pytest") | Some("py.test") => Some(ShellPattern::PythonPytest),
        Some("ruff") => Some(ShellPattern::PythonRuff),
        Some("mypy") => Some(ShellPattern::PythonMypy),
        Some("python") | Some("python3") | Some("py") => {
            classify_module_from_positionals(positionals)
        }
        _ => None,
    }
}

fn classify_module_from_positionals(positionals: &[&str]) -> Option<ShellPattern> {
    let mut iter = positionals.iter().copied();
    while let Some(arg) = iter.next() {
        if arg == "-m" {
            return iter.next().and_then(classify_module_name);
        }
        if let Some(module) = arg.strip_prefix("-m") {
            return classify_module_name(module);
        }
    }
    None
}

fn summarize_package_tool(
    result: &ShellResult,
    pattern: ShellPattern,
    tool_name: &str,
) -> CompressionSummary {
    let lines = package_lines(result);
    let summary = lines
        .iter()
        .find(|line| is_package_summary_line(line))
        .cloned()
        .or_else(|| lines.first().cloned())
        .unwrap_or_else(|| format!("{tool_name}: ok"));
    let details = detail_lines(lines, &summary);

    CompressionSummary::new(pattern, summary, details, preview(&result.stderr), result)
}

fn package_lines(result: &ShellResult) -> Vec<String> {
    non_empty_lines(&preferred_output(result))
        .into_iter()
        .map(|line| compact_whitespace(&line))
        .filter(|line| !line.starts_with("Downloading "))
        .filter(|line| !line.starts_with("Prepared "))
        .filter(|line| !line.starts_with("Resolved "))
        .filter(|line| !line.starts_with("Uninstalled "))
        .collect()
}

fn detail_lines(lines: Vec<String>, summary: &str) -> Vec<String> {
    let total = lines.iter().filter(|line| line.as_str() != summary).count();
    let shown = total.min(DETAIL_LIMIT);
    let mut details = sample_lines(
        lines
            .into_iter()
            .filter(|line| line != summary)
            .filter(|line| !line.starts_with("Collecting "))
            .filter(|line| !line.starts_with("Using cached "))
            .filter(|line| !line.starts_with("Installing collected packages")),
        DETAIL_LIMIT,
    );
    append_omitted_line(&mut details, total, shown, "lines");
    details
}

fn is_package_summary_line(line: &str) -> bool {
    line.starts_with("Successfully installed")
        || line.starts_with("Requirement already satisfied")
        || line.starts_with("No dependencies to install or update")
        || line.starts_with("Package operations:")
        || line.starts_with("Installing dependencies from lock file")
        || line.starts_with("Installed ")
        || line.starts_with("Using Python ")
        || line.starts_with("error:")
        || line.starts_with("ERROR:")
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
