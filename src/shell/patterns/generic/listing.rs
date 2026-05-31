use std::path::{Path, PathBuf};

use super::analyze::SummaryDetails;

const LS_PASSTHROUGH_THRESHOLD: usize = 20;
const LS_MAX_DETAILS: usize = 20;
const LS_MAX_OMITTED_NAMES: usize = 15;

pub(super) fn summarize_ls_entries(entries: &[String], args: &[String]) -> SummaryDetails {
    if uses_long_listing(args)
        || entries
            .iter()
            .any(|entry| looks_like_long_listing_row(entry))
    {
        return summarize_long_listing(entries);
    }

    summarize_simple_listing(entries, args)
}

fn summarize_simple_listing(entries: &[String], args: &[String]) -> SummaryDetails {
    let base_dir = resolve_ls_base_dir(args);
    let mut dirs = 0usize;
    let mut files = 0usize;
    let mut hidden = 0usize;
    let mut dir_entries = Vec::new();
    let mut file_entries = Vec::new();
    let mut other_entries = Vec::new();

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

        match classify_ls_entry(base_dir.as_deref(), entry) {
            Some(LsEntryKind::Directory) => dir_entries.push(display),
            Some(LsEntryKind::File) => file_entries.push(display),
            None => other_entries.push(display),
        }
    }

    dir_entries.sort();
    file_entries.sort();
    other_entries.sort();

    let mut ordered = Vec::with_capacity(entries.len());
    ordered.append(&mut dir_entries);
    ordered.append(&mut file_entries);
    ordered.append(&mut other_entries);

    let mut summary = format!("entries={}", entries.len());
    if dirs > 0 || files > 0 {
        summary.push_str(&format!("; dirs={dirs}; files={files}"));
    }
    if hidden > 0 {
        summary.push_str(&format!("; hidden={hidden}"));
    }

    SummaryDetails {
        summary,
        details: select_ls_details(ordered),
    }
}

fn summarize_long_listing(entries: &[String]) -> SummaryDetails {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut others = Vec::new();
    let mut hidden = 0usize;

    for entry in entries {
        let trimmed = entry.trim();
        if trimmed.is_empty() || trimmed.starts_with("total ") {
            continue;
        }
        if ignored_long_listing_row(trimmed) {
            continue;
        }

        match parse_long_listing_row(trimmed) {
            Some(parsed) => {
                if parsed.hidden {
                    hidden += 1;
                }
                match parsed.kind {
                    InventoryKind::Directory => dirs.push(format!("{}/", parsed.name)),
                    InventoryKind::File => files.push(format!("{}  {}", parsed.name, parsed.size)),
                    InventoryKind::Other => others.push(parsed.name),
                }
            }
            None => others.push(trimmed.to_string()),
        }
    }

    dirs.sort();
    files.sort();
    others.sort();

    let entry_count = dirs.len() + files.len() + others.len();
    let mut ordered = Vec::with_capacity(entry_count);
    ordered.extend(dirs.iter().cloned());
    ordered.extend(files.iter().cloned());
    ordered.extend(others.iter().cloned());

    let mut summary = format!("entries={entry_count}");
    if !dirs.is_empty() || !files.is_empty() {
        summary.push_str(&format!("; dirs={}; files={}", dirs.len(), files.len()));
    }
    if hidden > 0 {
        summary.push_str(&format!("; hidden={hidden}"));
    }

    SummaryDetails {
        summary,
        details: select_ls_details(ordered),
    }
}

fn select_ls_details(ordered: Vec<String>) -> Vec<String> {
    if ordered.len() <= LS_PASSTHROUGH_THRESHOLD {
        return ordered;
    }

    let mut details: Vec<String> = ordered.iter().take(LS_MAX_DETAILS).cloned().collect();
    let omitted: Vec<String> = ordered.into_iter().skip(LS_MAX_DETAILS).collect();
    let named = omitted.len().min(LS_MAX_OMITTED_NAMES);
    details.extend(omitted.iter().take(named).cloned());

    let remaining = omitted.len() - named;
    if remaining > 0 {
        details.push(format!("+ {remaining} more entries"));
    }

    details
}

fn uses_long_listing(args: &[String]) -> bool {
    args.iter().any(|arg| {
        arg == "--format=long"
            || arg == "-l"
            || arg == "-g"
            || arg == "-o"
            || (arg.starts_with('-')
                && !arg.starts_with("--")
                && arg
                    .chars()
                    .skip(1)
                    .any(|flag| matches!(flag, 'l' | 'g' | 'o')))
    })
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

fn looks_like_long_listing_row(entry: &str) -> bool {
    parse_long_listing_row(entry).is_some()
}

fn parse_long_listing_row(entry: &str) -> Option<LongInventoryEntry> {
    let columns: Vec<&str> = entry.split_whitespace().collect();
    if columns.len() < 9 || !looks_like_permissions(columns[0]) {
        return None;
    }

    let name = columns[8..].join(" ");
    Some(LongInventoryEntry {
        name,
        size: format_size(columns[4]),
        hidden: columns[8].starts_with('.'),
        kind: classify_inventory_kind(columns[0]),
    })
}

fn ignored_long_listing_row(entry: &str) -> bool {
    let columns: Vec<&str> = entry.split_whitespace().collect();
    columns.len() >= 9 && looks_like_permissions(columns[0]) && matches!(columns[8], "." | "..")
}

fn looks_like_permissions(mode: &str) -> bool {
    let mut chars = mode.chars();
    let Some(kind) = chars.next() else {
        return false;
    };
    if !matches!(kind, '-' | 'd' | 'l' | 'b' | 'c' | 'p' | 's') {
        return false;
    }

    mode.len() >= 10
}

fn classify_inventory_kind(mode: &str) -> InventoryKind {
    match mode.chars().next() {
        Some('d') => InventoryKind::Directory,
        Some('-') | Some('l') => InventoryKind::File,
        _ => InventoryKind::Other,
    }
}

fn format_size(raw: &str) -> String {
    if raw.chars().any(|ch| !ch.is_ascii_digit()) {
        return raw.to_string();
    }

    let Ok(bytes) = raw.parse::<u64>() else {
        return raw.to_string();
    };

    if bytes < 1024 {
        return format!("{bytes}B");
    }

    let kib = bytes as f64 / 1024.0;
    if kib < 1024.0 {
        return format!("{kib:.1}K");
    }

    let mib = kib / 1024.0;
    if mib < 1024.0 {
        return format!("{mib:.1}M");
    }

    format!("{:.1}G", mib / 1024.0)
}

#[derive(Debug, Clone)]
struct LongInventoryEntry {
    name: String,
    size: String,
    hidden: bool,
    kind: InventoryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InventoryKind {
    Directory,
    File,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LsEntryKind {
    Directory,
    File,
}

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
