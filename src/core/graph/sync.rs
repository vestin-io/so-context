//! Incremental project sync: re-parses only added/modified files, removes deleted ones.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use rusqlite::{Transaction, params};
use tree_sitter::Parser;
use tree_sitter_language_pack::{detect_language_from_path, get_language};
use walkdir::WalkDir;

use super::index::{insert_file_record_full, purge_file_data, reindex_file};
use super::symbols::collect_symbols;
use super::util::{content_hash, should_skip};
use crate::core_tokens::count_tokens;

// ---------------------------------------------------------------------------
// Tracked file snapshot
// ---------------------------------------------------------------------------

/// Lightweight snapshot of a `files` row used for change detection.
pub(super) struct TrackedFile {
    pub id: i64,
    pub size: i64,
    pub mtime: i64,
    pub hash: String,
}

/// Loads all file records for a project into a map keyed by relative path.
pub(super) fn load_tracked_files(
    tx: &Transaction<'_>,
    project_id: i64,
) -> Result<HashMap<String, TrackedFile>, String> {
    let mut stmt = tx
        .prepare(
            "SELECT id, path, COALESCE(size_bytes,0), COALESCE(mtime_unix,0),
                    COALESCE(content_hash,'')
             FROM files WHERE project_id=?1",
        )
        .map_err(|e| format!("failed to prepare tracked files query: {e}"))?;

    let rows = stmt
        .query_map(params![project_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("failed to query tracked files: {e}"))?;

    let mut map = HashMap::new();
    for row in rows {
        let (id, path, size, mtime, hash) =
            row.map_err(|e| format!("failed to read tracked file row: {e}"))?;
        map.insert(
            path,
            TrackedFile {
                id,
                size,
                mtime,
                hash,
            },
        );
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// Sync counters
// ---------------------------------------------------------------------------

#[derive(Default)]
pub(super) struct SyncCounts {
    pub added: i64,
    pub modified: i64,
    pub removed: i64,
    pub unchanged: i64,
}

// ---------------------------------------------------------------------------
// Core sync walk
// ---------------------------------------------------------------------------

/// Walks current files on disk, classifies each against the DB snapshot,
/// and applies additions, modifications, and deletions.
/// Returns counts for each category.
pub(super) fn sync_files(
    tx: &Transaction<'_>,
    project_root: &Path,
    project_id: i64,
    tracked: &HashMap<String, TrackedFile>,
) -> Result<SyncCounts, String> {
    let mut counts = SyncCounts::default();
    let mut seen_paths: HashMap<String, ()> = HashMap::new();
    let mut parser = Parser::new();

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

        let rel_path = path
            .strip_prefix(project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        seen_paths.insert(rel_path.clone(), ());

        // Cheap stat pre-filter: skip if size + mtime both match DB.
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let size = meta.len() as i64;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if let Some(rec) = tracked.get(&rel_path) {
            if rec.size == size && rec.mtime == mtime {
                counts.unchanged += 1;
                continue;
            }
        }

        // Read + hash to confirm a real content change.
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let hash = content_hash(&content);
        let token_count = count_tokens(&content);

        if let Some(rec) = tracked.get(&rel_path) {
            if rec.hash == hash {
                // Stat noise only (e.g. touch) — update stat columns, skip reparse.
                tx.execute(
                    "UPDATE files SET size_bytes=?1, mtime_unix=?2 WHERE id=?3",
                    params![size, mtime, rec.id],
                )
                .map_err(|e| format!("failed to update file stat: {e}"))?;
                counts.unchanged += 1;
            } else {
                // Genuinely modified: purge old data and reparse.
                purge_file_data(tx, rec.id)?;
                reindex_file(
                    tx,
                    &mut parser,
                    path,
                    project_id,
                    rec.id,
                    &lang_name,
                    &content,
                    size,
                    mtime,
                    &hash,
                    token_count,
                )?;
                counts.modified += 1;
            }
        } else {
            // New file.
            let (file_id, file_node_id) = insert_file_record_full(
                tx,
                project_root,
                path,
                project_id,
                &lang_name,
                size,
                mtime,
                &hash,
                token_count,
            )?;
            let language = get_language(&lang_name)
                .map_err(|e| format!("failed to load tree-sitter language '{lang_name}': {e}"))?;
            parser
                .set_language(&language)
                .map_err(|e| format!("failed to set parser language: {e}"))?;
            if let Some(tree) = parser.parse(&content, None) {
                let mut cursor = tree.walk();
                collect_symbols(tx, &content, project_id, file_id, file_node_id, &mut cursor)?;
            }
            counts.added += 1;
        }
    }

    // Remove records for files that no longer exist on disk.
    for (rel_path, rec) in tracked {
        if !seen_paths.contains_key(rel_path.as_str()) {
            purge_file_data(tx, rec.id)?;
            tx.execute("DELETE FROM files WHERE id=?1", params![rec.id])
                .map_err(|e| format!("failed to delete file row: {e}"))?;
            counts.removed += 1;
        }
    }

    Ok(counts)
}
