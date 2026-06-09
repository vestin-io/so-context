use std::collections::HashMap;

use super::super::text::append_omitted_line;
use super::analyze::SummaryDetails;

const SEARCH_MAX_RESULTS: usize = 200;
const SEARCH_MAX_PER_FILE: usize = 25;
const SEARCH_MAX_CONTENT_CHARS: usize = 160;

pub(super) fn summarize_search_hits(hits: &[String], query: Option<&str>) -> SummaryDetails {
    if hits.is_empty() {
        return SummaryDetails {
            summary: "0 matches".to_string(),
            details: Vec::new(),
        };
    }

    let mut shown_per_file: HashMap<&str, usize> = HashMap::new();
    let mut rendered_hits = Vec::new();

    for hit in hits {
        if rendered_hits.len() >= SEARCH_MAX_RESULTS {
            break;
        }

        let file_key = search_hit_file_key(hit);
        let shown_for_file = shown_per_file.entry(file_key).or_default();
        if *shown_for_file >= SEARCH_MAX_PER_FILE {
            continue;
        }

        rendered_hits.push(truncate_search_hit(hit, query));
        *shown_for_file += 1;
    }

    let shown = rendered_hits.len();
    append_omitted_line(&mut rendered_hits, hits.len(), shown, "matches");

    SummaryDetails {
        summary: String::new(),
        details: rendered_hits,
    }
}

fn search_hit_file_key(hit: &str) -> &str {
    match hit.split_once(':') {
        Some((file, _)) => file,
        None => hit,
    }
}

fn truncate_search_hit(hit: &str, query: Option<&str>) -> String {
    let Some((prefix, content)) = split_search_hit(hit) else {
        return truncate_around_match(hit, query, SEARCH_MAX_CONTENT_CHARS);
    };

    let truncated = truncate_around_match(content, query, SEARCH_MAX_CONTENT_CHARS);
    format!("{prefix}{truncated}")
}

fn split_search_hit(hit: &str) -> Option<(&str, &str)> {
    let (file, rest) = hit.split_once(':')?;
    let (line, content) = rest.split_once(':')?;
    Some((&hit[..file.len() + 1 + line.len() + 1], content))
}

fn truncate_around_match(text: &str, query: Option<&str>, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_chars {
        return text.to_string();
    }

    if let Some(query) = query.filter(|value| !value.is_empty()) {
        let lower_text = text.to_lowercase();
        let lower_query = query.to_lowercase();
        if let Some(byte_pos) = lower_text.find(&lower_query) {
            let char_pos = lower_text[..byte_pos].chars().count();
            let start = char_pos.saturating_sub(max_chars / 3);
            let mut end = (start + max_chars).min(chars.len());
            let start = if end == chars.len() {
                end = chars.len();
                end.saturating_sub(max_chars)
            } else {
                start
            };

            let slice: String = chars[start..end].iter().collect();
            return match (start > 0, end < chars.len()) {
                (true, true) => format!("...{slice}..."),
                (true, false) => format!("...{slice}"),
                (false, true) => format!("{slice}..."),
                (false, false) => slice,
            };
        }
    }

    let slice: String = chars[..max_chars.saturating_sub(3)].iter().collect();
    format!("{slice}...")
}
