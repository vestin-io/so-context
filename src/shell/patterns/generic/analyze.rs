use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use super::super::text::{compact_whitespace, truncate_text};

pub(super) struct SummaryDetails {
    pub summary: String,
    pub details: Vec<String>,
}

pub(super) fn summarize_find_paths(paths: &[String]) -> SummaryDetails {
    if paths.is_empty() {
        return SummaryDetails {
            summary: "0F 0D".to_string(),
            details: Vec::new(),
        };
    }

    let mut by_dir: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut by_ext: HashMap<String, usize> = HashMap::new();

    for path in paths {
        let path_obj = Path::new(path);
        let dir = path_obj
            .parent()
            .map(|parent| parent.display().to_string())
            .filter(|parent| !parent.is_empty())
            .unwrap_or_else(|| ".".to_string());
        let name = path_obj
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());

        let ext = path_obj
            .extension()
            .map(|ext| format!(".{}", ext.to_string_lossy()))
            .unwrap_or_else(|| "no-ext".to_string());
        *by_ext.entry(ext).or_default() += 1;
        by_dir.entry(dir).or_default().push(name);
    }

    let mut details = Vec::new();
    for (dir, names) in &by_dir {
        let mut names = names.clone();
        names.sort();
        details.push(format!("{dir}/ {}", names.join(" ")));
    }

    let mut ext_counts: Vec<(String, usize)> = by_ext.into_iter().collect();
    ext_counts.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    if !ext_counts.is_empty() {
        let ext_summary = ext_counts
            .into_iter()
            .take(5)
            .map(|(ext, count)| format!("{ext}({count})"))
            .collect::<Vec<_>>()
            .join(" ");
        details.push(format!("ext: {ext_summary}"));
    }

    SummaryDetails {
        summary: format!("{}F {}D", paths.len(), by_dir.len()),
        details,
    }
}

pub(super) fn summarize_env_vars(vars: &[String]) -> Vec<String> {
    let mut path_vars = Vec::new();
    let mut lang_vars = Vec::new();
    let mut cloud_vars = Vec::new();
    let mut tool_vars = Vec::new();
    let mut other_vars = Vec::new();

    for line in vars {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let rendered = format!("{key}={}", summarize_env_value(key, value));
        if key.contains("PATH") {
            path_vars.push(rendered);
        } else if is_language_env_key(key) {
            lang_vars.push(rendered);
        } else if is_cloud_env_key(key) {
            cloud_vars.push(rendered);
        } else if is_tool_env_key(key) {
            tool_vars.push(rendered);
        } else if is_interesting_env_key(key) {
            other_vars.push(rendered);
        }
    }

    let mut details = Vec::new();
    details.extend(path_vars.into_iter().take(2));
    details.extend(lang_vars.into_iter().take(3));
    details.extend(cloud_vars.into_iter().take(3));
    details.extend(tool_vars.into_iter().take(3));
    details.extend(other_vars.into_iter().take(3));
    details
}

pub(super) fn clean_search_snippet(snippet: &str) -> String {
    let trimmed = compact_whitespace(snippet);
    for marker in ["stdout:\"", "stderr:\""] {
        if let Some((_, inner)) = trimmed.split_once(marker) {
            let line = inner.split("\\n").next().unwrap_or(inner);
            let line = line.trim_end_matches('"').trim_end_matches(".into(),");
            let mut parts = line.splitn(3, ':');
            let _ = parts.next();
            let maybe_line = parts.next();
            let maybe_content = parts.next();
            if maybe_line.is_some() && maybe_content.is_some() {
                return maybe_content.unwrap_or(line).trim().to_string();
            }
            return line.trim().to_string();
        }
    }
    trimmed
}

pub(super) fn stream_source(args: &[String]) -> Option<String> {
    args.iter()
        .rev()
        .find(|arg| !arg.starts_with('-'))
        .map(|arg| truncate_text(&redact_stream_source(arg), 50))
}

fn redact_stream_source(source: &str) -> String {
    if source.contains("Authorization:") || source.contains("authorization:") {
        return "[REDACTED]".to_string();
    }

    if let Some((scheme, rest)) = source.split_once("://") {
        let (before_query, query_suffix) = match rest.split_once('?') {
            Some((prefix, _)) => (prefix, "?…"),
            None => (rest, ""),
        };

        let redacted_authority =
            if let Some((userinfo, host_and_path)) = before_query.split_once('@') {
                if !userinfo.is_empty() {
                    format!("[REDACTED]@{host_and_path}")
                } else {
                    before_query.to_string()
                }
            } else {
                before_query.to_string()
            };

        return format!("{scheme}://{redacted_authority}{query_suffix}");
    }

    source.to_string()
}

fn summarize_env_value(key: &str, value: &str) -> String {
    if is_sensitive_env_key(key) {
        return "***".to_string();
    }
    if key == "PATH" {
        let segments = value.split(':').count();
        return format!("{segments} entries");
    }
    truncate_text(value, 60)
}

fn is_sensitive_env_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    ["TOKEN", "SECRET", "PASSWORD", "KEY", "CREDENTIAL"]
        .iter()
        .any(|pattern| upper.contains(pattern))
}

fn is_language_env_key(key: &str) -> bool {
    matches!(
        key,
        "RUSTUP_HOME"
            | "CARGO_HOME"
            | "RUST_LOG"
            | "GOENV"
            | "GOMOD"
            | "GOPATH"
            | "PYTHONPATH"
            | "VIRTUAL_ENV"
            | "BUNDLE_PATH"
            | "GEM_HOME"
            | "NODE_ENV"
            | "NVM_DIR"
    )
}

fn is_cloud_env_key(key: &str) -> bool {
    key.starts_with("AWS_")
        || key.starts_with("AZURE_")
        || key.starts_with("GCP_")
        || key.starts_with("GOOGLE_")
        || key.starts_with("CLOUD_")
}

fn is_tool_env_key(key: &str) -> bool {
    key.starts_with("DOCKER_")
        || key.starts_with("KUBECONFIG")
        || key.starts_with("GIT_")
        || key.starts_with("CI")
        || key.starts_with("TERM")
}

fn is_interesting_env_key(key: &str) -> bool {
    matches!(
        key,
        "HOME" | "USER" | "SHELL" | "PWD" | "LANG" | "EDITOR" | "TMPDIR"
    )
}
