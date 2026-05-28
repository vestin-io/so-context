//! Database helpers: schema init, path utilities, project record management, FTS refresh.
//!
//! The main entry point is [`GraphDb`], which owns a single SQLite connection
//! and runs schema init once on open. Use it for long-lived contexts (watch mode).
//! One-shot CLI calls can use the free functions in `mod.rs` which open a
//! temporary `GraphDb` and drop it immediately.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, Transaction, params};

use super::index::index_files;
use super::sync::{load_tracked_files, sync_files};
use super::symbols::resolve_reference_edges;

// ---------------------------------------------------------------------------
// GraphDb — long-lived connection owner
// ---------------------------------------------------------------------------

/// Owns a SQLite connection to the project graph database.
/// Schema is initialised once in [`GraphDb::open`]; subsequent calls to
/// `index`, `sync`, and `search` reuse the same connection without repeating
/// the schema setup cost.
pub struct GraphDb {
    conn: Connection,
    project_root: PathBuf,
    db_path: PathBuf,
}

impl GraphDb {
    /// Opens (or creates) the graph database for `project_root`, running
    /// schema init exactly once.
    pub fn open(project_root: PathBuf) -> Result<Self, String> {
        let db_path = graph_db_path(&project_root);
        ensure_parent_dir(&db_path)?;
        let conn =
            Connection::open(&db_path).map_err(|e| format!("failed to open db: {e}"))?;
        init_schema(&conn)?;
        Ok(Self { conn, project_root, db_path })
    }

    /// Returns the path to the database file.
    #[allow(dead_code)]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Returns whether the database file already existed before this open.
    /// Useful for deciding whether to run a full index or an incremental sync.
    pub fn exists(project_root: &Path) -> bool {
        graph_db_path(project_root).exists()
    }

    // -----------------------------------------------------------------------
    // Index (full wipe + rebuild)
    // -----------------------------------------------------------------------

    /// Wipes all existing project data and rebuilds the graph from scratch.
    /// Returns a human-readable summary string.
    pub fn index(&mut self) -> Result<String, String> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| format!("failed to start tx: {e}"))?;

        let project_id = upsert_project(&tx, &self.project_root)?;
        clear_project_data(&tx, project_id)?;

        let (file_count, symbol_count) = index_files(&tx, &self.project_root, project_id)?;

        resolve_reference_edges(&tx, project_id)?;
        refresh_fts(&tx, project_id)?;

        tx.commit()
            .map_err(|e| format!("failed to commit tx: {e}"))?;
        Ok(format!(
            "Graph indexed: files={} symbols={} db={}",
            file_count,
            symbol_count,
            self.db_path.display()
        ))
    }

    // -----------------------------------------------------------------------
    // Sync (incremental)
    // -----------------------------------------------------------------------

    /// Incrementally syncs the graph: re-parses only added/modified files,
    /// removes stale records for deleted files.
    /// Returns a human-readable summary string.
    pub fn sync(&mut self) -> Result<String, String> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| format!("failed to start tx: {e}"))?;

        let project_id = upsert_project(&tx, &self.project_root)?;
        let tracked = load_tracked_files(&tx, project_id)?;
        let counts = sync_files(&tx, &self.project_root, project_id, &tracked)?;

        if counts.added > 0 || counts.modified > 0 || counts.removed > 0 {
            resolve_reference_edges(&tx, project_id)?;
            refresh_fts(&tx, project_id)?;
        }

        tx.commit()
            .map_err(|e| format!("failed to commit tx: {e}"))?;
        Ok(format!(
            "Graph synced: added={} modified={} removed={} unchanged={}",
            counts.added, counts.modified, counts.removed, counts.unchanged
        ))
    }

    // -----------------------------------------------------------------------
    // Search
    // -----------------------------------------------------------------------

    /// FTS search over indexed symbols. Returns formatted results and total
    /// token count of matched files (used for token saving estimates).
    pub fn search_with_stats(&self, query: &str, limit: usize) -> Result<(String, i64), String> {
        let fts_query = build_fts_query(query);
        if fts_query.is_empty() {
            return Ok(("No results.".to_string(), 0));
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT f.path, n.kind, n.name, n.start_line, bm25(nodes_fts, 0, 20, 5, 1, 2) as score
                 FROM nodes_fts
                 JOIN nodes n ON n.id = nodes_fts.rowid
                 JOIN files f ON f.id = n.file_id
                 WHERE nodes_fts MATCH ?1
                 ORDER BY score ASC
                 LIMIT ?2",
            )
            .map_err(|e| format!("failed to prepare search query: {e}"))?;

        let rows = stmt
            .query_map(params![fts_query, limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, f64>(4)?,
                ))
            })
            .map_err(|e| format!("failed to execute search query: {e}"))?;

        let mut out = Vec::new();
        let mut matched_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
        for row in rows {
            let (path, kind, name, line, score) =
                row.map_err(|e| format!("failed to read row: {e}"))?;
            out.push(format!("{path}:{line} [{kind}] {name} (score={:.4})", score.abs()));
            matched_paths.insert(path);
        }

        let matched_files_tokens = if matched_paths.is_empty() {
            0
        } else {
            let placeholders = std::iter::repeat_n("?", matched_paths.len()).collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT COALESCE(SUM(COALESCE(token_count, 0)), 0)
                 FROM files
                 WHERE path IN ({placeholders})"
            );
            let params: Vec<&dyn rusqlite::ToSql> = matched_paths
                .iter()
                .map(|p| p as &dyn rusqlite::ToSql)
                .collect();
            self.conn
                .query_row(&sql, params.as_slice(), |row| row.get::<_, i64>(0))
                .map_err(|e| format!("failed to sum matched file tokens: {e}"))?
        };

        if out.is_empty() {
            Ok(("No results.".to_string(), 0))
        } else {
            Ok((out.join("\n"), matched_files_tokens))
        }
    }
}

/// Normalizes user text into a robust FTS5 query with prefix matching.
///
/// Pipeline:
/// - replace `::` with space
/// - strip FTS special chars: `'"*():^`
/// - split into terms
/// - remove boolean operator tokens (AND/OR/NOT/NEAR)
/// - convert each term to `"term"*` prefix form
/// - join terms with ` OR `
fn build_fts_query(input: &str) -> String {
    let cleaned = input
        .replace("::", " ")
        .chars()
        .filter(|c| !matches!(c, '\'' | '"' | '*' | '(' | ')' | ':' | '^'))
        .collect::<String>();

    cleaned
        .split_whitespace()
        .filter(|term| {
            let u = term.to_ascii_uppercase();
            !matches!(u.as_str(), "AND" | "OR" | "NOT" | "NEAR")
        })
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Returns the centralized graphs directory: `~/.local/share/so-context/graphs/`.
pub fn graphs_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("so-context")
        .join("graphs")
}

/// Returns the path to the graph DB for a given project root.
///
/// Uses a SHA-256 hex digest of the canonical absolute path as the filename,
/// so the DB lives outside the project tree and never needs `.gitignore`.
///
/// Example: `~/.local/share/so-context/graphs/a3f2c1….db`
pub fn graph_db_path(project_root: &Path) -> PathBuf {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Use the canonical path if available, otherwise the raw path string.
    let key = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let key_str = key.to_string_lossy();

    // Simple 64-bit hash is sufficient for a per-user local store.
    let mut hasher = DefaultHasher::new();
    key_str.hash(&mut hasher);
    let hash = hasher.finish();

    graphs_dir().join(format!("{hash:016x}.db"))
}

pub fn validate_project_root(project_path: &str) -> Result<PathBuf, String> {
    let root = PathBuf::from(project_path);
    if root.is_dir() {
        Ok(root)
    } else {
        Err(format!("path is not a directory: {}", root.display()))
    }
}

pub(super) fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create db dir: {e}"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Schema
// ---------------------------------------------------------------------------

/// Creates/updates all required graph schema tables and indexes.
/// Called once per connection open — not on every operation.
pub(super) fn init_schema(conn: &Connection) -> Result<(), String> {
    // Keep existing data intact across opens; only create missing tables/indexes.
    conn.execute_batch(include_str!("graph_schema.sql"))
        .map_err(|e| format!("failed to init schema: {e}"))
}

// ---------------------------------------------------------------------------
// Project record helpers
// ---------------------------------------------------------------------------

pub(super) fn upsert_project(tx: &Transaction<'_>, project_root: &Path) -> Result<i64, String> {
    let root_str = project_root.to_string_lossy().to_string();
    tx.execute(
        "INSERT INTO projects(root_path) VALUES (?1)
         ON CONFLICT(root_path) DO UPDATE SET updated_at = CURRENT_TIMESTAMP",
        params![root_str],
    )
    .map_err(|e| format!("failed to upsert project row: {e}"))?;

    tx.query_row(
        "SELECT id FROM projects WHERE root_path = ?1",
        params![root_str],
        |row| row.get(0),
    )
    .map_err(|e| format!("failed to load project id: {e}"))
}

pub(super) fn clear_project_data(tx: &Transaction<'_>, project_id: i64) -> Result<(), String> {
    for (table, label) in [
        ("nodes", "nodes"),
        ("edges", "edges"),
        ("unresolved_refs", "unresolved refs"),
        ("files", "files"),
    ] {
        tx.execute(
            &format!("DELETE FROM {table} WHERE project_id = ?1"),
            params![project_id],
        )
        .map_err(|e| format!("failed to clear {label}: {e}"))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// FTS
// ---------------------------------------------------------------------------

/// Rebuilds the FTS index for a project.
pub(super) fn refresh_fts(tx: &Transaction<'_>, project_id: i64) -> Result<(), String> {
    tx.execute(
        "DELETE FROM nodes_fts WHERE rowid IN (
            SELECT n.id FROM nodes n WHERE n.project_id = ?1
        )",
        params![project_id],
    )
    .map_err(|e| format!("failed to clear project fts rows: {e}"))?;

    tx.execute(
        "INSERT INTO nodes_fts(rowid, name, fq_name, signature, doc, path)
         SELECT n.id, n.name, COALESCE(n.fq_name, ''), COALESCE(n.signature, ''),
                COALESCE(n.doc, ''), f.path
         FROM nodes n
         JOIN files f ON f.id = n.file_id
         WHERE n.project_id = ?1",
        params![project_id],
    )
    .map_err(|e| format!("failed to populate fts rows: {e}"))?;

    Ok(())
}
