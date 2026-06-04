use std::path::PathBuf;

/// Graph-DB-backed outline for a specific file path inside a project.
///
/// Queries the graph DB for imports + typed symbol nodes (kind, name, signature,
/// visibility, line).  Falls back to a regex scan when the file is not indexed
/// or the DB doesn't exist yet.
pub fn build_outline_for_path(file_path: &str, content: &str) -> String {
    if let Some(root) = infer_project_root(file_path) {
        match crate::core_graph::outline_file(&root, file_path) {
            Ok(Some(outline)) => return format_db_outline(&outline, file_path),
            Ok(None) => {} // not indexed yet — fall through
            Err(_) => {}   // DB error — fall through
        }
    }
    build_outline_regex(content)
}

/// Formats a `FileOutline` from the graph DB into a compact, agent-friendly string.
fn format_db_outline(outline: &crate::core_graph::FileOutline, file_path: &str) -> String {
    use std::path::Path;

    let short = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(file_path);

    let mut out = format!("{short} [{}]", outline.language);

    if !outline.imports.is_empty() {
        out.push_str("\n  imports: ");
        out.push_str(&outline.imports.join(", "));
    }

    if !outline.symbols.is_empty() {
        out.push_str("\n  symbols:");
        for sym in &outline.symbols {
            let vis = if sym.visibility == "public" || sym.visibility == "pub" {
                "pub "
            } else {
                ""
            };
            let async_prefix = if sym.is_async { "async " } else { "" };
            let static_prefix = if sym.is_static { "static " } else { "" };
            let sig = if sym.signature.is_empty() {
                sym.name.clone()
            } else {
                sym.signature.clone()
            };
            out.push_str(&format!(
                "\n    {}: {vis}{async_prefix}{static_prefix}[{}] {sig}",
                sym.line, sym.kind,
            ));
        }
    } else if outline.imports.is_empty() {
        out.push_str("\n  (no indexed symbols)");
    }

    out
}

/// Walk up from file_path to find the nearest project root.
///
/// Recognises: `.git`, `Cargo.toml`, `package.json`, `go.mod`, `pyproject.toml`.
fn infer_project_root(file_path: &str) -> Option<String> {
    const MARKERS: &[&str] = &[
        ".git",
        "Cargo.toml",
        "package.json",
        "go.mod",
        "pyproject.toml",
    ];
    let p = PathBuf::from(file_path);
    let mut dir = if p.is_absolute() {
        p.parent()?.to_path_buf()
    } else {
        std::env::current_dir()
            .ok()?
            .join(p)
            .parent()?
            .to_path_buf()
    };

    loop {
        for marker in MARKERS {
            if dir.join(marker).exists() {
                return Some(dir.to_string_lossy().to_string());
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Regex-style scan outline — fallback when the graph DB is unavailable.
fn build_outline_regex(content: &str) -> String {
    let mut out = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with('#')
            || t.starts_with("fn ")
            || t.starts_with("pub fn ")
            || t.starts_with("async fn ")
            || t.starts_with("pub async fn ")
            || t.starts_with("struct ")
            || t.starts_with("pub struct ")
            || t.starts_with("enum ")
            || t.starts_with("pub enum ")
            || t.starts_with("trait ")
            || t.starts_with("pub trait ")
            || t.starts_with("impl ")
            || t.starts_with("class ")
            || t.starts_with("interface ")
            || t.starts_with("def ")
            || t.starts_with("export fn ")
            || t.starts_with("export function ")
            || t.starts_with("export class ")
            || t.starts_with("export default ")
        {
            out.push(format!("{}: {}", idx + 1, t));
        }
    }

    if out.is_empty() {
        "No outline items found.".to_string()
    } else {
        out.join("\n")
    }
}
