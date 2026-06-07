use std::collections::HashMap;

use super::super::text::append_omitted_line;
use super::analyze::SummaryDetails;

const SEARCH_MAX_RESULTS: usize = 200;
const SEARCH_MAX_PER_FILE: usize = 25;

pub(super) fn summarize_search_hits(hits: &[String]) -> SummaryDetails {
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

        rendered_hits.push(hit.clone());
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
