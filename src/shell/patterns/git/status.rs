use std::collections::BTreeSet;

use super::super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::super::text::preview;

pub(super) fn summarize(result: &ShellResult) -> CompressionSummary {
    let mut branch = None;
    let mut changed_files = BTreeSet::new();
    let mut rendered_status_lines = Vec::new();
    let mut section: Option<&str> = None;

    for line in result.stdout.lines() {
        let trimmed = line.trim();

        if let Some(rest) = line.strip_prefix("## ") {
            branch = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("On branch ") {
            branch = Some(rest.trim().to_string());
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
            let path = line[3..].trim();
            if !path.is_empty() {
                changed_files.insert(path.to_string());
            }
            rendered_status_lines.push(line.to_string());
            continue;
        }

        if let Some(path) = trimmed.strip_prefix("modified:") {
            let path = path.trim().to_string();
            changed_files.insert(path.clone());
            match section {
                Some("staged") => rendered_status_lines.push(format!("M  {path}")),
                Some("unstaged") => rendered_status_lines.push(format!(" M {path}")),
                _ => rendered_status_lines.push(format!(" M {path}")),
            }
        } else if let Some(path) = trimmed.strip_prefix("new file:") {
            let path = path.trim().to_string();
            changed_files.insert(path.clone());
            match section {
                Some("untracked") => rendered_status_lines.push(format!("?? {path}")),
                _ => rendered_status_lines.push(format!("A  {path}")),
            }
        } else if let Some(path) = trimmed.strip_prefix("deleted:") {
            let path = path.trim().to_string();
            changed_files.insert(path.clone());
            match section {
                Some("staged") => rendered_status_lines.push(format!("D  {path}")),
                Some("unstaged") => rendered_status_lines.push(format!(" D {path}")),
                _ => rendered_status_lines.push(format!(" D {path}")),
            }
        } else if let Some(path) = trimmed.strip_prefix("renamed:") {
            let path = path.trim().to_string();
            changed_files.insert(path.clone());
            rendered_status_lines.push(format!("R  {path}"));
        } else if let Some(path) = line.strip_prefix('\t') {
            let path = path.trim().to_string();
            changed_files.insert(path.clone());
            match section {
                Some("staged") => rendered_status_lines.push(format!("M  {path}")),
                Some("unstaged") => rendered_status_lines.push(format!(" M {path}")),
                Some("untracked") => rendered_status_lines.push(format!("?? {path}")),
                _ => {}
            }
        }
    }

    let has_branch = branch.is_some();
    let summary = if let Some(branch) = branch {
        format!("* {branch}")
    } else if rendered_status_lines.is_empty() {
        "Clean working tree".to_string()
    } else {
        rendered_status_lines.remove(0)
    };

    if has_branch && changed_files.is_empty() {
        rendered_status_lines.push("clean — nothing to commit".to_string());
    }

    CompressionSummary::plain(
        ShellPattern::GitStatus,
        summary,
        rendered_status_lines,
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
