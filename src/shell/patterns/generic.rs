use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::helpers::{
    append_omitted_line, non_empty_lines, preferred_output, preview, sample_lines,
};

pub fn summarize_ls(result: &ShellResult) -> CompressionSummary {
    let entries = non_empty_lines(&result.stdout);
    let (summary, details) = summarize_ls_entries(&entries, result.invocation.args());

    CompressionSummary {
        pattern: ShellPattern::Ls,
        summary,
        details,
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_find(result: &ShellResult) -> CompressionSummary {
    let paths = non_empty_lines(&result.stdout);

    CompressionSummary {
        pattern: ShellPattern::Find,
        summary: format!("paths={}", paths.len()),
        details: sample_lines(paths, 8),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_rg(result: &ShellResult) -> CompressionSummary {
    summarize_search_output(result, ShellPattern::Rg)
}

pub fn summarize_grep(result: &ShellResult) -> CompressionSummary {
    summarize_search_output(result, ShellPattern::Grep)
}

fn summarize_search_output(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let hits = non_empty_lines(&result.stdout);
    let grouped = group_search_hits(&hits);
    let total_files = grouped.len();
    let shown = total_files.min(6);
    let mut details = sample_lines(grouped, 6);
    append_omitted_line(&mut details, total_files, shown, "files");

    CompressionSummary {
        pattern,
        summary: format!("hits={}; files={total_files}", hits.len()),
        details,
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_curl(result: &ShellResult) -> CompressionSummary {
    summarize_stream_like(result, ShellPattern::Curl, "curl")
}

pub fn summarize_wget(result: &ShellResult) -> CompressionSummary {
    summarize_stream_like(result, ShellPattern::Wget, "wget")
}

pub fn summarize_env(result: &ShellResult) -> CompressionSummary {
    let vars = non_empty_lines(&result.stdout);
    let keys: Vec<String> = vars
        .iter()
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.to_string()))
        .collect();

    CompressionSummary {
        pattern: ShellPattern::Env,
        summary: format!("vars={}", vars.len()),
        details: sample_lines(keys, 8),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_cat(result: &ShellResult) -> CompressionSummary {
    summarize_file_excerpt(result, ShellPattern::Cat, "cat")
}

pub fn summarize_head(result: &ShellResult) -> CompressionSummary {
    summarize_file_excerpt(result, ShellPattern::Head, "head")
}

pub fn summarize_tail(result: &ShellResult) -> CompressionSummary {
    summarize_file_excerpt(result, ShellPattern::Tail, "tail")
}

pub fn summarize_unknown(result: &ShellResult) -> CompressionSummary {
    let details = non_empty_lines(&result.stdout);

    CompressionSummary {
        pattern: ShellPattern::Unknown,
        summary: format!(
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
        details: sample_lines(details, 8),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

fn summarize_stream_like(
    result: &ShellResult,
    pattern: ShellPattern,
    label: &str,
) -> CompressionSummary {
    let body = non_empty_lines(&result.stdout);
    let stderr_lines = non_empty_lines(&result.stderr);

    CompressionSummary {
        pattern,
        summary: format!(
            "{label}_body_lines={}; body_bytes={}; stderr_lines={}",
            body.len(),
            result.stdout.len(),
            stderr_lines.len()
        ),
        details: sample_lines(body, 8),
        stderr_preview: sample_lines(stderr_lines, 5),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

fn summarize_file_excerpt(
    result: &ShellResult,
    pattern: ShellPattern,
    label: &str,
) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));

    CompressionSummary {
        pattern,
        summary: format!(
            "{label}_lines={}; bytes={}",
            lines.len(),
            result.stdout.len()
        ),
        details: sample_lines(lines, 8),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

fn summarize_ls_entries(entries: &[String], args: &[String]) -> (String, Vec<String>) {
    let mut details = Vec::new();
    let mut dirs = 0usize;
    let mut files = 0usize;
    let mut hidden = 0usize;
    let base_dir = resolve_ls_base_dir(args);

    for entry in entries {
        if entry.starts_with('.') {
            hidden += 1;
        }

        let display = match classify_ls_entry(base_dir.as_deref(), entry) {
            Some(LsEntryKind::Directory) => {
                dirs += 1;
                format!("{entry}/")
            }
            Some(LsEntryKind::File) => {
                files += 1;
                entry.clone()
            }
            None => entry.clone(),
        };

        details.push(display);
    }

    let shown = details.len().min(8);
    let mut details = sample_lines(details, 8);
    append_omitted_line(&mut details, entries.len(), shown, "entries");

    let mut summary = format!("entries={}", entries.len());
    if dirs > 0 || files > 0 {
        summary.push_str(&format!("; dirs={dirs}; files={files}"));
    }
    if hidden > 0 {
        summary.push_str(&format!("; hidden={hidden}"));
    }

    (summary, details)
}

fn group_search_hits(hits: &[String]) -> Vec<String> {
    let mut grouped: BTreeMap<String, SearchGroup> = BTreeMap::new();

    for hit in hits {
        let parsed = parse_search_hit(hit);
        let entry = grouped
            .entry(parsed.file.clone())
            .or_insert_with(|| SearchGroup::new(parsed.line.clone(), parsed.snippet.clone()));
        entry.count += 1;
    }

    grouped
        .into_iter()
        .map(|(file, group)| group.render(file))
        .collect()
}

fn resolve_ls_base_dir(args: &[String]) -> Option<PathBuf> {
    let targets: Vec<&String> = args.iter().filter(|arg| !arg.starts_with('-')).collect();
    match targets.as_slice() {
        [] => Some(PathBuf::from(".")),
        [single] => Some(PathBuf::from(single)),
        _ => None,
    }
}

fn classify_ls_entry(base_dir: Option<&Path>, entry: &str) -> Option<LsEntryKind> {
    let base_dir = base_dir?;
    let candidate = if Path::new(entry).is_absolute() {
        PathBuf::from(entry)
    } else {
        base_dir.join(entry)
    };

    if candidate.is_dir() {
        Some(LsEntryKind::Directory)
    } else if candidate.is_file() {
        Some(LsEntryKind::File)
    } else {
        None
    }
}

fn parse_search_hit(hit: &str) -> SearchHit {
    let mut parts = hit.splitn(3, ':');
    let file = parts.next().unwrap_or(hit).to_string();
    match (parts.next(), parts.next()) {
        (Some(line), Some(snippet)) if line.chars().all(|ch| ch.is_ascii_digit()) => SearchHit {
            file,
            line: Some(line.to_string()),
            snippet: snippet.trim().to_string(),
        },
        (Some(rest), Some(snippet)) => SearchHit {
            file,
            line: None,
            snippet: format!("{rest}:{}", snippet.trim()),
        },
        (Some(rest), None) => SearchHit {
            file,
            line: None,
            snippet: rest.trim().to_string(),
        },
        (None, None) => SearchHit {
            file,
            line: None,
            snippet: String::new(),
        },
        (None, Some(_)) => SearchHit {
            file,
            line: None,
            snippet: String::new(),
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LsEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone)]
struct SearchHit {
    file: String,
    line: Option<String>,
    snippet: String,
}

#[derive(Debug, Clone)]
struct SearchGroup {
    count: usize,
    first_line: Option<String>,
    first_snippet: String,
}

impl SearchGroup {
    fn new(first_line: Option<String>, first_snippet: String) -> Self {
        Self {
            count: 0,
            first_line,
            first_snippet,
        }
    }

    fn render(self, file: String) -> String {
        let location = self
            .first_line
            .as_ref()
            .map(|line| format!("{file}:{line}"))
            .unwrap_or(file);
        if self.first_snippet.is_empty() {
            format!("{location} ({} hits)", self.count)
        } else {
            format!("{location} ({} hits): {}", self.count, self.first_snippet)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::types::ShellInvocation;

    #[test]
    fn summarizes_rg_hits() {
        let result = ShellResult {
            invocation: ShellInvocation::new(vec!["rg".into(), "todo".into()]),
            stdout: "src/main.rs:10: TODO one\nsrc/lib.rs:7: TODO two\n".into(),
            stderr: String::new(),
            exit_code: 0,
        };

        let summary = summarize_rg(&result);
        assert!(summary.summary.contains("hits=2"));
        assert!(summary.summary.contains("files=2"));
        assert_eq!(summary.details[0], "src/lib.rs:7 (1 hits): TODO two");
    }

    #[test]
    fn summarizes_ls_entries() {
        let result = ShellResult {
            invocation: ShellInvocation::new(vec!["ls".into()]),
            stdout: "Cargo.toml\nREADME.md\nsrc\n".into(),
            stderr: String::new(),
            exit_code: 0,
        };

        let summary = summarize_ls(&result);
        assert!(summary.summary.contains("entries=3"));
        assert_eq!(summary.details[0], "Cargo.toml");
    }

    #[test]
    fn summarizes_find_paths() {
        let result = ShellResult {
            invocation: ShellInvocation::new(vec!["find".into(), "src".into()]),
            stdout: "src/main.rs\nsrc/shell/mod.rs\n".into(),
            stderr: String::new(),
            exit_code: 0,
        };

        let summary = summarize_find(&result);
        assert!(summary.summary.contains("paths=2"));
        assert_eq!(summary.details[1], "src/shell/mod.rs");
    }

    #[test]
    fn summarizes_grep_hits() {
        let result = ShellResult {
            invocation: ShellInvocation::new(vec![
                "grep".into(),
                "-R".into(),
                "todo".into(),
                ".".into(),
            ]),
            stdout: "./src/main.rs:10: TODO one\n./src/lib.rs:7: TODO two\n".into(),
            stderr: String::new(),
            exit_code: 0,
        };

        let summary = summarize_grep(&result);
        assert!(summary.summary.contains("hits=2"));
        assert!(summary.summary.contains("files=2"));
        assert_eq!(summary.details[0], "./src/lib.rs:7 (1 hits): TODO two");
    }

    #[test]
    fn summarizes_head_excerpt() {
        let result = ShellResult {
            invocation: ShellInvocation::new(vec![
                "head".into(),
                "-n".into(),
                "2".into(),
                "README.md".into(),
            ]),
            stdout: "# so-context\nintro line\n".into(),
            stderr: String::new(),
            exit_code: 0,
        };

        let summary = summarize_head(&result);
        assert!(summary.summary.contains("head_lines=2"));
        assert_eq!(summary.details[0], "# so-context");
    }

    #[test]
    fn summarizes_env_keys() {
        let result = ShellResult {
            invocation: ShellInvocation::new(vec!["env".into()]),
            stdout: "FOO=one\nBAR=two\n".into(),
            stderr: String::new(),
            exit_code: 0,
        };

        let summary = summarize_env(&result);
        assert!(summary.summary.contains("vars=2"));
        assert_eq!(summary.details[0], "FOO");
    }
}
