//! Full project indexing: walks every file, parses, and writes all records from scratch.

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use rusqlite::{Transaction, params};
use tree_sitter::Parser;
use tree_sitter_language_pack::{detect_language_from_path, get_language};
use walkdir::WalkDir;

use super::symbols::collect_symbols;
use super::util::{content_hash, should_skip};
use crate::core_tokens::count_tokens;

// ---------------------------------------------------------------------------
// Full index walk
// ---------------------------------------------------------------------------

/// Walks the project tree, parses each supported file, and inserts all records.
/// Returns `(file_count, symbol_count)`.
pub(super) fn index_files(
    tx: &Transaction<'_>,
    project_root: &Path,
    project_id: i64,
) -> Result<(i64, i64), String> {
    let mut parser = Parser::new();
    let mut file_count = 0_i64;
    let mut symbol_count = 0_i64;

    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() || should_skip(path) {
            continue;
        }

        let path_str = path.to_string_lossy();
        let Some(lang_name) = detect_language_from_path(&path_str) else {
            continue;
        };

        let language = get_language(lang_name)
            .map_err(|e| format!("failed to load tree-sitter language '{lang_name}': {e}"))?;
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to set parser language: {e}"))?;

        let content = fs::read_to_string(path).map_err(|e| format!("failed to read file: {e}"))?;
        let Some(tree) = parser.parse(&content, None) else {
            continue;
        };

        let (size, mtime) = file_stat(path);
        let hash = content_hash(&content);
        let token_count = count_tokens(&content);

        let (file_id, file_node_id) = insert_file_record_full(
            tx,
            project_root,
            path,
            project_id,
            lang_name,
            size,
            mtime,
            &hash,
            token_count,
        )?;
        file_count += 1;

        let mut cursor = tree.walk();
        symbol_count +=
            collect_symbols(tx, &content, project_id, file_id, file_node_id, &mut cursor)?;
    }

    Ok((file_count, symbol_count))
}

// ---------------------------------------------------------------------------
// Per-file record helpers (also used by sync)
// ---------------------------------------------------------------------------

/// Inserts a new `files` row and a corresponding `'file'`-kind node.
/// Returns `(file_id, file_node_id)`.
#[allow(clippy::too_many_arguments)]
pub(super) fn insert_file_record_full(
    tx: &Transaction<'_>,
    project_root: &Path,
    path: &Path,
    project_id: i64,
    lang_name: &str,
    size: i64,
    mtime: i64,
    hash: &str,
    token_count: i64,
) -> Result<(i64, i64), String> {
    let rel_path = path
        .strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    tx.execute(
        "INSERT INTO files(project_id, path, language, content_hash, size_bytes, mtime_unix, token_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![project_id, rel_path, lang_name, hash, size, mtime, token_count],
    )
    .map_err(|e| format!("failed to insert file row: {e}"))?;
    let file_id = tx.last_insert_rowid();

    let abs_path = path.to_string_lossy().to_string();
    tx.execute(
        "INSERT INTO nodes(
            project_id, file_id, kind, name, start_line, start_col, end_line, end_col
        ) VALUES (?1, ?2, 'file', ?3, 1, 1, 1, 1)",
        params![project_id, file_id, abs_path],
    )
    .map_err(|e| format!("failed to insert file node: {e}"))?;
    let file_node_id = tx.last_insert_rowid();

    Ok((file_id, file_node_id))
}

/// Deletes all nodes, edges, and unresolved refs for a single file.
/// The `files` row itself is left intact (callers remove it as needed).
pub(super) fn purge_file_data(tx: &Transaction<'_>, file_id: i64) -> Result<(), String> {
    tx.execute(
        "DELETE FROM unresolved_refs WHERE file_id=?1",
        params![file_id],
    )
    .map_err(|e| format!("failed to purge unresolved refs: {e}"))?;

    // Nodes cascade-delete their edges via FK.
    tx.execute("DELETE FROM nodes WHERE file_id=?1", params![file_id])
        .map_err(|e| format!("failed to purge nodes: {e}"))?;

    Ok(())
}

/// Re-parses a modified file and updates its DB record in place.
#[allow(clippy::too_many_arguments)]
pub(super) fn reindex_file(
    tx: &Transaction<'_>,
    parser: &mut Parser,
    path: &Path,
    project_id: i64,
    file_id: i64,
    lang_name: &str,
    content: &str,
    size: i64,
    mtime: i64,
    hash: &str,
    token_count: i64,
) -> Result<(), String> {
    tx.execute(
        "UPDATE files SET language=?1, content_hash=?2, size_bytes=?3,
                          mtime_unix=?4, token_count=?5, indexed_at=CURRENT_TIMESTAMP
         WHERE id=?6",
        params![lang_name, hash, size, mtime, token_count, file_id],
    )
    .map_err(|e| format!("failed to update file record: {e}"))?;

    let abs_path = path.to_string_lossy().to_string();
    tx.execute(
        "INSERT INTO nodes(
            project_id, file_id, kind, name, start_line, start_col, end_line, end_col
         ) VALUES (?1, ?2, 'file', ?3, 1, 1, 1, 1)",
        params![project_id, file_id, abs_path],
    )
    .map_err(|e| format!("failed to insert file node: {e}"))?;
    let file_node_id = tx.last_insert_rowid();

    let language = get_language(lang_name)
        .map_err(|e| format!("failed to load tree-sitter language '{lang_name}': {e}"))?;
    parser
        .set_language(&language)
        .map_err(|e| format!("failed to set parser language: {e}"))?;

    if let Some(tree) = parser.parse(content, None) {
        let mut cursor = tree.walk();
        collect_symbols(tx, content, project_id, file_id, file_node_id, &mut cursor)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

pub(super) fn file_stat(path: &Path) -> (i64, i64) {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return (0, 0),
    };
    let size = meta.len() as i64;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    (size, mtime)
}

/// Resolve relative path from project root, returning a string.
#[allow(dead_code)]
pub(super) fn rel_path_str(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}
