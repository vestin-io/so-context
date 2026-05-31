use std::collections::BTreeSet;

use super::super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::super::text::preview;

pub(super) fn summarize(result: &ShellResult) -> CompressionSummary {
    let mut branch = "unknown".to_string();
    let mut staged = 0usize;
    let mut unstaged = 0usize;
    let mut untracked = 0usize;
    let mut changed_files = BTreeSet::new();
    let mut staged_files = Vec::new();
    let mut unstaged_files = Vec::new();
    let mut untracked_files = Vec::new();
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
                if !path.is_empty() {
                    untracked_files.push(format!("untracked: {path}"));
                }
            } else {
                if x != ' ' {
                    staged += 1;
                    if !path.is_empty() {
                        staged_files.push(format!("staged: {path}"));
                    }
                }
                if y != ' ' {
                    unstaged += 1;
                    if !path.is_empty() {
                        unstaged_files.push(format!("unstaged: {path}"));
                    }
                }
            }
            continue;
        }

        if let Some(path) = trimmed.strip_prefix("modified:") {
            let path = path.trim().to_string();
            changed_files.insert(path.clone());
            match section {
                Some("staged") => {
                    staged += 1;
                    staged_files.push(format!("staged: {path}"));
                }
                Some("unstaged") => {
                    unstaged += 1;
                    unstaged_files.push(format!("unstaged: {path}"));
                }
                _ => {
                    unstaged += 1;
                    unstaged_files.push(format!("unstaged: {path}"));
                }
            }
        } else if let Some(path) = trimmed.strip_prefix("new file:") {
            let path = path.trim().to_string();
            changed_files.insert(path.clone());
            match section {
                Some("untracked") => {
                    untracked += 1;
                    untracked_files.push(format!("untracked: {path}"));
                }
                _ => {
                    staged += 1;
                    staged_files.push(format!("staged: {path}"));
                }
            }
        } else if let Some(path) = trimmed.strip_prefix("deleted:") {
            let path = path.trim().to_string();
            changed_files.insert(path.clone());
            match section {
                Some("staged") => {
                    staged += 1;
                    staged_files.push(format!("staged: {path}"));
                }
                Some("unstaged") => {
                    unstaged += 1;
                    unstaged_files.push(format!("unstaged: {path}"));
                }
                _ => {
                    unstaged += 1;
                    unstaged_files.push(format!("unstaged: {path}"));
                }
            }
        } else if let Some(path) = trimmed.strip_prefix("renamed:") {
            let path = path.trim().to_string();
            changed_files.insert(path.clone());
            staged += 1;
            staged_files.push(format!("staged: {path}"));
        } else if let Some(path) = line.strip_prefix('\t') {
            let path = path.trim().to_string();
            changed_files.insert(path.clone());
            match section {
                Some("staged") => {
                    staged += 1;
                    staged_files.push(format!("staged: {path}"));
                }
                Some("unstaged") => {
                    unstaged += 1;
                    unstaged_files.push(format!("unstaged: {path}"));
                }
                Some("untracked") => {
                    untracked += 1;
                    untracked_files.push(format!("untracked: {path}"));
                }
                _ => {}
            }
        }
    }

    let mut details = Vec::new();
    details.extend(staged_files);
    details.extend(unstaged_files);
    details.extend(untracked_files);

    CompressionSummary::new(
        ShellPattern::GitStatus,
        format!(
            "{branch} | staged={staged}; unstaged={unstaged}; untracked={untracked}; changed_files={}",
            changed_files.len()
        ),
        details,
        preview(&result.stderr),
        result,
    )
}

fn is_porcelain_status_line(line: &str) -> bool {
    if line.len() < 3 || !line.is_ascii() {
        return false;
    }

    let bytes = line.as_bytes();
    let valid = b" MADRCU?!";
    valid.contains(&bytes[0]) && valid.contains(&bytes[1]) && bytes[2] == b' '
}
