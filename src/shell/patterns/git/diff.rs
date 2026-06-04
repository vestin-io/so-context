use std::cmp::Reverse;

use super::super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::super::text::{compact_whitespace, preview, truncate_text};

pub(super) fn summarize(result: &ShellResult) -> CompressionSummary {
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
                files[index].record_hunk_header(line);
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
            if let Some(index) = current_file {
                files[index].additions += 1;
                files[index].record_change(line);
            }
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
            if let Some(index) = current_file {
                files[index].deletions += 1;
                files[index].record_change(line);
            }
        }
    }

    let file_count = files.len();
    let mut ranked_indexes: Vec<usize> = (0..file_count).collect();
    ranked_indexes.sort_by_key(|index| Reverse(files[*index].weight()));
    let details = ranked_indexes
        .into_iter()
        .enumerate()
        .map(|(position, index)| {
            let include_evidence = file_count <= 8 || position < 4;
            files[index].render(include_evidence)
        })
        .collect();

    CompressionSummary::new(
        ShellPattern::GitDiff,
        format!(
            "{} files changed, {hunks} hunks, +{additions}/-{deletions}",
            files.len()
        ),
        details,
        preview(&result.stderr),
        result,
    )
}

#[derive(Debug, Clone)]
struct DiffFileSummary {
    path: String,
    rename_from: Option<String>,
    additions: usize,
    deletions: usize,
    hunks: usize,
    status: Option<&'static str>,
    first_hunk_header: Option<String>,
    snippets: Vec<String>,
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
            first_hunk_header: None,
            snippets: Vec::new(),
        }
    }

    fn weight(&self) -> usize {
        self.additions + self.deletions + (self.hunks * 4)
    }

    fn record_hunk_header(&mut self, line: &str) {
        if self.first_hunk_header.is_none() {
            self.first_hunk_header = Some(truncate_text(line.trim(), 80));
        }
    }

    fn record_change(&mut self, line: &str) {
        if self.snippets.len() >= 3 {
            return;
        }

        let normalized = compact_whitespace(line.trim());
        if normalized.len() <= 1 {
            return;
        }
        self.snippets.push(truncate_text(&normalized, 90));
    }

    fn render(&self, include_evidence: bool) -> String {
        let label = self
            .rename_from
            .as_ref()
            .map(|source| format!("{source} -> {}", self.path))
            .unwrap_or_else(|| self.path.clone());
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

        let base = if parts.is_empty() {
            label
        } else {
            format!("{label} ({})", parts.join(", "))
        };

        if !include_evidence {
            return base;
        }

        let mut evidence = Vec::new();
        if let Some(header) = &self.first_hunk_header {
            evidence.push(header.clone());
        }
        evidence.extend(self.snippets.iter().cloned());

        if evidence.is_empty() {
            base
        } else {
            format!("{base} — {}", evidence.join(" | "))
        }
    }
}
