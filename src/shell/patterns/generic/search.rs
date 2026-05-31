use std::collections::BTreeMap;

use super::super::text::{append_omitted_line, sample_lines, truncate_text};
use super::analyze::{SummaryDetails, clean_search_snippet};

pub(super) fn summarize_search_hits(hits: &[String]) -> SummaryDetails {
    let grouped = group_search_hits(hits);
    let total_files = grouped.len();
    let shown = total_files.min(6);
    let mut details = sample_lines(grouped, 6);
    append_omitted_line(&mut details, total_files, shown, "files");

    SummaryDetails {
        summary: format!(
            "{} matches in {} {}",
            hits.len(),
            total_files,
            if total_files == 1 { "file" } else { "files" }
        ),
        details,
    }
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
        (None, None) | (None, Some(_)) => SearchHit {
            file,
            line: None,
            snippet: String::new(),
        },
    }
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
        let match_label = if self.count == 1 { "match" } else { "matches" };
        let snippet = truncate_text(&clean_search_snippet(&self.first_snippet), 100);
        if self.first_snippet.is_empty() {
            format!("{location} ({} {match_label})", self.count)
        } else {
            format!("{location} ({} {match_label}) — {snippet}", self.count)
        }
    }
}
