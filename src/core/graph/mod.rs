//! Code graph module.
//!
//! Sub-modules:
//! - `db`      — [`GraphDb`] connection owner, schema, path helpers, project record management, FTS
//! - `index`   — full project walk and file record insertion
//! - `sync`    — incremental sync (stat/hash diffing, per-file reparse)
//! - `symbols` — tree-sitter symbol extraction and reference resolution
//! - `watch`   — filesystem watcher loop
//! - `util`    — shared utilities (content hash, skip predicate)
//!
//! Graph databases are stored centrally at:
//!   `~/.local/share/so-context/graphs/<hash>.db`
//! where `<hash>` is a 64-bit hash of the canonical project root path.

mod db;
mod index;
mod sync;
mod symbols;
mod util;
pub mod watch;

pub use db::{GraphDb, FileOutline, validate_project_root};
pub use watch::watch_project;

// ---------------------------------------------------------------------------
// Constants (shared across sub-modules via `super::`)
// ---------------------------------------------------------------------------

pub(self) const REINDEX_DEBOUNCE_MS: u64 = 700;
pub(self) const WATCH_POLL_SECS: u64 = 1;

// ---------------------------------------------------------------------------
// Public free functions (one-shot CLI use — open, run, drop)
// ---------------------------------------------------------------------------

/// Performs a full project graph index into SQLite under `.so-context/graph.db`.
/// Wipes all existing data for the project and rebuilds from scratch.
pub fn index_project(project_path: &str) -> Result<String, String> {
    let project_root = validate_project_root(project_path)?;
    GraphDb::open(project_root)?.index()
}

/// Incrementally syncs the project graph: re-parses only added/modified files,
/// removes stale records for deleted files. Falls back to a full index when no
/// DB exists yet.
#[allow(dead_code)]
pub fn sync_project(project_path: &str) -> Result<String, String> {
    let project_root = validate_project_root(project_path)?;
    if !GraphDb::exists(&project_root) {
        return index_project(project_path);
    }
    GraphDb::open(project_root)?.sync()
}

/// Returns a structured file outline from the graph DB for the given file path.
/// The project root is inferred by walking up from the file path to find the DB.
/// Returns `None` when the file is not indexed.
pub fn outline_file(project_path: &str, file_path: &str) -> Result<Option<FileOutline>, String> {
    let project_root = validate_project_root(project_path)?;
    if !GraphDb::exists(&project_root) {
        return Ok(None);
    }
    GraphDb::open(project_root)?.query_file_outline(file_path)
}
/// Returns formatted results and total token count of matched files.
pub fn search_project_with_stats(project_path: &str, query: &str, limit: usize) -> Result<(String, i64), String> {
    let project_root = validate_project_root(project_path)?;
    if !GraphDb::exists(&project_root) {
        let db_path = db::graph_db_path(&project_root);
        return Err(format!(
            "graph db not found at {} (run index first)",
            db_path.display()
        ));
    }
    GraphDb::open(project_root)?.search_with_stats(query, limit)
}
