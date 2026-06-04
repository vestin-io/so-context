use std::collections::BTreeSet;

use super::super::text::{append_omitted_line, sample_lines, truncate_text};
use super::analyze::{SummaryDetails, clean_search_snippet};

const SEARCH_PASSTHROUGH_THRESHOLD: usize = 60;

pub(super) fn summarize_search_hits(hits: &[String]) -> SummaryDetails {
    let total_files = count_files(hits);
    let shown = hits.len().min(SEARCH_PASSTHROUGH_THRESHOLD);
    let mut details = sample_lines(
        hits.iter()
            .map(|hit| render_search_hit(&parse_search_hit(hit))),
        SEARCH_PASSTHROUGH_THRESHOLD,
    );
    append_omitted_line(&mut details, hits.len(), shown, "matches");

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

fn count_files(hits: &[String]) -> usize {
    let mut files = BTreeSet::new();
    for hit in hits {
        files.insert(parse_search_hit(hit).file);
    }
    files.len()
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

fn render_search_hit(hit: &SearchHit) -> String {
    let location = hit
        .line
        .as_ref()
        .map(|line| format!("{}:{line}", hit.file))
        .unwrap_or_else(|| hit.file.clone());
    let snippet = truncate_text(&clean_search_snippet(&hit.snippet), 100);
    if hit.snippet.is_empty() {
        location
    } else {
        format!("{location} — {snippet}")
    }
}
