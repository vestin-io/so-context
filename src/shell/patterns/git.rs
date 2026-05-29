use std::cmp::Reverse;
use std::collections::BTreeSet;

use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::helpers::{non_empty_lines, preferred_output, preview, sample_lines};

pub fn summarize_status(result: &ShellResult) -> CompressionSummary {
    let mut branch = "unknown".to_string();
    let mut staged = 0usize;
    let mut unstaged = 0usize;
    let mut untracked = 0usize;
    let mut changed_files = BTreeSet::new();
    let mut section: Option<&str> = None;

    for line in result.stdout.lines() {
        let trimmed = line.trim();

        if let Some(rest) = line.strip_prefix("## ") {
            branch = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("On branch ") {
            branch = rest.trim().to_string();
            continue;
        }

        match trimmed {
            "Changes to be committed:" => {
                section = Some("staged");
                continue;
            }
            "Changes not staged for commit:" => {
                section = Some("unstaged");
                continue;
            }
            "Untracked files:" => {
                section = Some("untracked");
                continue;
            }
            ""
            | "(use \"git restore --staged <file>...\" to unstage)"
            | "(use \"git add <file>...\" to update what will be committed)"
            | "(use \"git restore <file>...\" to discard changes in working directory)"
            | "(use \"git add <file>...\" to include in what will be committed)" => continue,
            _ => {}
        }

        if is_porcelain_status_line(line) {
            let bytes = line.as_bytes();
            let x = bytes[0] as char;
            let y = bytes[1] as char;
            let path = line[3..].trim();
            if !path.is_empty() {
                changed_files.insert(path.to_string());
            }

            if x == '?' && y == '?' {
                untracked += 1;
            } else {
                if x != ' ' {
                    staged += 1;
                }
                if y != ' ' {
                    unstaged += 1;
                }
            }
            continue;
        }

        if let Some(path) = trimmed.strip_prefix("modified:") {
            changed_files.insert(path.trim().to_string());
            match section {
                Some("staged") => staged += 1,
                Some("unstaged") => unstaged += 1,
                _ => unstaged += 1,
            }
        } else if let Some(path) = trimmed.strip_prefix("new file:") {
            changed_files.insert(path.trim().to_string());
            match section {
                Some("untracked") => untracked += 1,
                _ => staged += 1,
            }
        } else if let Some(path) = trimmed.strip_prefix("deleted:") {
            changed_files.insert(path.trim().to_string());
            match section {
                Some("staged") => staged += 1,
                Some("unstaged") => unstaged += 1,
                _ => unstaged += 1,
            }
        } else if let Some(path) = trimmed.strip_prefix("renamed:") {
            changed_files.insert(path.trim().to_string());
            staged += 1;
        } else if let Some(path) = line.strip_prefix('\t') {
            changed_files.insert(path.trim().to_string());
            match section {
                Some("staged") => staged += 1,
                Some("unstaged") => unstaged += 1,
                Some("untracked") => untracked += 1,
                _ => {}
            }
        }
    }

    CompressionSummary {
        pattern: ShellPattern::GitStatus,
        summary: format!(
            "branch={branch}; staged={staged}; unstaged={unstaged}; untracked={untracked}; files={}",
            changed_files.len()
        ),
        details: sample_lines(changed_files.into_iter().collect(), 6),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_diff(result: &ShellResult) -> CompressionSummary {
    let mut files = Vec::new();
    let mut additions = 0usize;
    let mut deletions = 0usize;
    let mut hunks = 0usize;
    let mut current_file: Option<usize> = None;

    for line in result.stdout.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            if let Some((path, _)) = rest.split_once(" b/") {
                files.push(DiffFileSummary::new(path.trim().to_string()));
                current_file = Some(files.len() - 1);
            }
        } else if line.starts_with("new file mode") {
            if let Some(index) = current_file {
                files[index].status = Some("new");
            }
        } else if line.starts_with("deleted file mode") {
            if let Some(index) = current_file {
                files[index].status = Some("deleted");
            }
        } else if let Some(source) = line.strip_prefix("rename from ") {
            if let Some(index) = current_file {
                files[index].rename_from = Some(source.trim().to_string());
                files[index].status = Some("renamed");
            }
        } else if let Some(target) = line.strip_prefix("rename to ") {
            if let Some(index) = current_file {
                files[index].path = target.trim().to_string();
                files[index].status = Some("renamed");
            }
        } else if line.starts_with("Binary files ") {
            if let Some(index) = current_file {
                files[index].status = Some("binary");
            }
        } else if line.starts_with("@@") {
            hunks += 1;
            if let Some(index) = current_file {
                files[index].hunks += 1;
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
            if let Some(index) = current_file {
                files[index].additions += 1;
            }
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
            if let Some(index) = current_file {
                files[index].deletions += 1;
            }
        }
    }

    let mut ranked = files.clone();
    ranked.sort_by_key(|file| Reverse(file.weight()));
    let details: Vec<String> = ranked.into_iter().map(DiffFileSummary::render).collect();

    CompressionSummary {
        pattern: ShellPattern::GitDiff,
        summary: format!(
            "files={}; hunks={hunks}; additions={additions}; deletions={deletions}",
            files.len()
        ),
        details,
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_log(result: &ShellResult) -> CompressionSummary {
    let entries = non_empty_lines(&result.stdout);

    CompressionSummary {
        pattern: ShellPattern::GitLog,
        summary: format!("commits={}", entries.len()),
        details: sample_lines(entries, 6),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_branch(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let current = lines
        .iter()
        .find_map(|line| line.strip_prefix("* ").map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string());
    let branches: Vec<String> = lines
        .iter()
        .map(|line| line.trim_start_matches("* ").trim().to_string())
        .collect();

    CompressionSummary {
        pattern: ShellPattern::GitBranch,
        summary: format!("current={current}; branches={}", branches.len()),
        details: sample_lines(branches, 6),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_remote(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let remotes: BTreeSet<String> = lines
        .iter()
        .filter_map(|line| line.split_whitespace().next().map(str::to_string))
        .collect();

    CompressionSummary {
        pattern: ShellPattern::GitRemote,
        summary: format!("remotes={}", remotes.len()),
        details: sample_lines(lines, 6),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_show(result: &ShellResult) -> CompressionSummary {
    let output = preferred_output(result);
    let lines = non_empty_lines(&output);
    let first = lines
        .first()
        .cloned()
        .unwrap_or_else(|| "no output".to_string());
    let diff_like = summarize_diff(result);

    CompressionSummary {
        pattern: ShellPattern::GitShow,
        summary: format!("{}; preview={first}", diff_like.summary),
        details: sample_lines(lines, 6),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_fetch(result: &ShellResult) -> CompressionSummary {
    summarize_transport(result, ShellPattern::GitFetch, "fetch")
}

pub fn summarize_pull(result: &ShellResult) -> CompressionSummary {
    summarize_transport(result, ShellPattern::GitPull, "pull")
}

pub fn summarize_push(result: &ShellResult) -> CompressionSummary {
    summarize_transport(result, ShellPattern::GitPush, "push")
}

pub fn summarize_checkout(result: &ShellResult) -> CompressionSummary {
    summarize_transition(result, ShellPattern::GitCheckout)
}

pub fn summarize_switch(result: &ShellResult) -> CompressionSummary {
    summarize_transition(result, ShellPattern::GitSwitch)
}

pub fn summarize_commit(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let headline = lines
        .first()
        .cloned()
        .unwrap_or_else(|| "no output".to_string());

    CompressionSummary {
        pattern: ShellPattern::GitCommit,
        summary: format!("commit_result={headline}"),
        details: sample_lines(lines, 6),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

fn summarize_transport(
    result: &ShellResult,
    pattern: ShellPattern,
    label: &str,
) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let updated_refs = lines
        .iter()
        .filter(|line| line.contains("->") || line.contains("Fast-forward"))
        .count();

    CompressionSummary {
        pattern,
        summary: format!("{label}_lines={}; updated_refs={updated_refs}", lines.len()),
        details: sample_lines(lines, 6),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

fn summarize_transition(result: &ShellResult, pattern: ShellPattern) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let target = lines
        .first()
        .cloned()
        .unwrap_or_else(|| "no output".to_string());

    CompressionSummary {
        pattern,
        summary: format!("transition={target}"),
        details: sample_lines(lines, 6),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

fn is_porcelain_status_line(line: &str) -> bool {
    if line.len() < 3 || !line.is_ascii() {
        return false;
    }

    let bytes = line.as_bytes();
    let valid = b" MADRCU?!";
    valid.contains(&bytes[0]) && valid.contains(&bytes[1]) && bytes[2] == b' '
}

#[derive(Debug, Clone)]
struct DiffFileSummary {
    path: String,
    rename_from: Option<String>,
    additions: usize,
    deletions: usize,
    hunks: usize,
    status: Option<&'static str>,
}

impl DiffFileSummary {
    fn new(path: String) -> Self {
        Self {
            path,
            rename_from: None,
            additions: 0,
            deletions: 0,
            hunks: 0,
            status: None,
        }
    }

    fn weight(&self) -> usize {
        self.additions + self.deletions + (self.hunks * 4)
    }

    fn render(self) -> String {
        let label = self
            .rename_from
            .map(|source| format!("{source} -> {}", self.path))
            .unwrap_or(self.path);
        let mut parts = Vec::new();
        if self.additions > 0 || self.deletions > 0 {
            parts.push(format!("+{}/-{}", self.additions, self.deletions));
        }
        if self.hunks > 0 {
            let label = if self.hunks == 1 { "hunk" } else { "hunks" };
            parts.push(format!("{} {label}", self.hunks));
        }
        if let Some(status) = self.status {
            parts.push(status.to_string());
        }

        if parts.is_empty() {
            label
        } else {
            format!("{label} ({})", parts.join(", "))
        }
    }
}

#[cfg(test)]
#[path = "git_tests.rs"]
mod tests;
