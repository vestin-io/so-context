//! Database helpers: schema init, path utilities, project record management, FTS refresh.
//!
//! The main entry point is [`GraphDb`], which owns a single SQLite connection
//! and runs schema init once on open. Use it for long-lived contexts (watch mode).
//! One-shot CLI calls can use the free functions in `mod.rs` which open a
//! temporary `GraphDb` and drop it immediately.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction, params};

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
    pub fn search_with_stats(&self, query: &str, limit: usize) -> Result<(String, i64, i64), String> {
        let fts_query = build_fts_query(query);
        if fts_query.is_empty() {
            return Ok(("No results.".to_string(), 0, 0));
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

        let (matched_files_tokens, matched_files_size) = if matched_paths.is_empty() {
            (0i64, 0i64)
        } else {
            let placeholders = std::iter::repeat_n("?", matched_paths.len()).collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT COALESCE(SUM(COALESCE(token_count, 0)), 0),
                        COALESCE(SUM(COALESCE(size_bytes, 0)), 0)
                 FROM files
                 WHERE path IN ({placeholders})"
            );
            let params: Vec<&dyn rusqlite::ToSql> = matched_paths
                .iter()
                .map(|p| p as &dyn rusqlite::ToSql)
                .collect();
            self.conn
                .query_row(&sql, params.as_slice(), |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(|e| format!("failed to sum matched file stats: {e}"))?
        };

        if out.is_empty() {
            Ok(("No results.".to_string(), 0, 0))
        } else {
            Ok((out.join("\n"), matched_files_tokens, matched_files_size))
        }
    }

    // -----------------------------------------------------------------------
    // References
    // -----------------------------------------------------------------------

    /// Returns all references to (or from) a named symbol, filtered by `kind`.
    ///
    /// `kind` values:
    ///   - `"all"`     — every edge involving the symbol (default)
    ///   - `"callers"` — symbols that call this symbol
    ///   - `"callees"` — symbols this symbol calls
    ///   - `"imports"` — import edges involving this symbol
    pub fn query_references(
        &self,
        symbol: &str,
        kind: &str,
        include_declaration: bool,
    ) -> Result<Vec<ReferenceEntry>, String> {
        // Resolve the target symbol node(s) by name.
        let mut symbol_stmt = self
            .conn
            .prepare(
                "SELECT n.id, f.path, n.kind, n.start_line
                 FROM nodes n
                 JOIN files f ON f.id = n.file_id
                 WHERE n.name = ?1 AND n.kind != 'file'",
            )
            .map_err(|e| format!("failed to prepare symbol lookup: {e}"))?;

        let target_nodes: Vec<(i64, String, String, i64)> = symbol_stmt
            .query_map(params![symbol], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(|e| format!("failed to query symbol nodes: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        if target_nodes.is_empty() {
            return Ok(vec![]);
        }

        let mut results: Vec<ReferenceEntry> = Vec::new();

        // Optionally include the declaration site(s).
        if include_declaration {
            for (_, path, sym_kind, line) in &target_nodes {
                results.push(ReferenceEntry {
                    file:     path.clone(),
                    line:     *line,
                    ref_kind: "declaration".to_string(),
                    symbol:   symbol.to_string(),
                    sym_kind: sym_kind.clone(),
                    snippet:  None,
                });
            }
        }

        for (node_id, _, _, _) in &target_nodes {
            match kind {
                "callers" => {
                    // Who calls this symbol?
                    let mut stmt = self.conn.prepare(
                        "SELECT f.path, caller.name, caller.kind, e.line
                         FROM edges e
                         JOIN nodes caller ON caller.id = e.from_node_id
                         JOIN files  f     ON f.id = caller.file_id
                         WHERE e.to_node_id = ?1 AND e.kind = 'calls'",
                    ).map_err(|e| format!("failed to prepare callers query: {e}"))?;
                    let rows = stmt.query_map(params![node_id], |row| {
                        Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?,
                            row.get::<_,String>(2)?, row.get::<_,i64>(3)?))
                    }).map_err(|e| format!("failed to query callers: {e}"))?;
                    for row in rows.filter_map(|r| r.ok()) {
                        results.push(ReferenceEntry {
                            file: row.0, line: row.3,
                            ref_kind: "caller".to_string(),
                            symbol: row.1, sym_kind: row.2, snippet: None,
                        });
                    }
                }
                "callees" => {
                    // What does this symbol call?
                    let mut stmt = self.conn.prepare(
                        "SELECT f.path, callee.name, callee.kind, e.line
                         FROM edges e
                         JOIN nodes callee ON callee.id = e.to_node_id
                         JOIN files  f     ON f.id = callee.file_id
                         WHERE e.from_node_id = ?1 AND e.kind = 'calls'",
                    ).map_err(|e| format!("failed to prepare callees query: {e}"))?;
                    let rows = stmt.query_map(params![node_id], |row| {
                        Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?,
                            row.get::<_,String>(2)?, row.get::<_,i64>(3)?))
                    }).map_err(|e| format!("failed to query callees: {e}"))?;
                    for row in rows.filter_map(|r| r.ok()) {
                        results.push(ReferenceEntry {
                            file: row.0, line: row.3,
                            ref_kind: "callee".to_string(),
                            symbol: row.1, sym_kind: row.2, snippet: None,
                        });
                    }
                }
                "imports" => {
                    // Import edges where symbol is the target.
                    let mut stmt = self.conn.prepare(
                        "SELECT f.path, importer.name, importer.kind, e.line
                         FROM edges e
                         JOIN nodes importer ON importer.id = e.from_node_id
                         JOIN files  f       ON f.id = importer.file_id
                         WHERE e.to_node_id = ?1 AND e.kind = 'imports'",
                    ).map_err(|e| format!("failed to prepare imports query: {e}"))?;
                    let rows = stmt.query_map(params![node_id], |row| {
                        Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?,
                            row.get::<_,String>(2)?, row.get::<_,i64>(3)?))
                    }).map_err(|e| format!("failed to query imports: {e}"))?;
                    for row in rows.filter_map(|r| r.ok()) {
                        results.push(ReferenceEntry {
                            file: row.0, line: row.3,
                            ref_kind: "import".to_string(),
                            symbol: row.1, sym_kind: row.2, snippet: None,
                        });
                    }
                }
                _ => {
                    // "all" — callers + callees + imports
                    let mut stmt = self.conn.prepare(
                        "SELECT f.path, other.name, other.kind, e.line, e.kind,
                                CASE WHEN e.to_node_id = ?1 THEN 'inbound' ELSE 'outbound' END
                         FROM edges e
                         JOIN nodes other ON other.id = CASE
                             WHEN e.to_node_id = ?1 THEN e.from_node_id
                             ELSE e.to_node_id END
                         JOIN files f ON f.id = other.file_id
                         WHERE (e.from_node_id = ?1 OR e.to_node_id = ?1)
                           AND e.kind IN ('calls', 'imports')",
                    ).map_err(|e| format!("failed to prepare all-refs query: {e}"))?;
                    let rows = stmt.query_map(params![node_id], |row| {
                        Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?,
                            row.get::<_,String>(2)?, row.get::<_,i64>(3)?,
                            row.get::<_,String>(4)?, row.get::<_,String>(5)?))
                    }).map_err(|e| format!("failed to query all refs: {e}"))?;
                    for row in rows.filter_map(|r| r.ok()) {
                        let ref_kind = format!("{}:{}", row.4, row.5); // e.g. "calls:inbound"
                        results.push(ReferenceEntry {
                            file: row.0, line: row.3,
                            ref_kind,
                            symbol: row.1, sym_kind: row.2, snippet: None,
                        });
                    }
                }
            }
        }

        // Deduplicate by (file, line, ref_kind, symbol).
        results.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
        results.dedup_by(|a, b| {
            a.file == b.file && a.line == b.line && a.ref_kind == b.ref_kind && a.symbol == b.symbol
        });

        Ok(results)
    }

    // -----------------------------------------------------------------------
    // Outline
    // -----------------------------------------------------------------------

    /// Returns a structured outline for a single file given its project-relative
    /// or absolute path. Queries the graph DB for:
    ///   - imports (unresolved_refs with ref_kind = 'import')
    ///   - symbol nodes (functions, structs, classes, …) with visibility + line
    ///
    /// Returns `None` when the file is not indexed or the DB doesn't exist yet.
    pub fn query_file_outline(&self, file_path: &str) -> Result<Option<FileOutline>, String> {
        // Normalise to a project-relative path for the DB lookup.
        let rel_path = {
            let root = self.project_root.to_string_lossy();
            let abs = if std::path::Path::new(file_path).is_absolute() {
                file_path.to_string()
            } else {
                // Already relative — use as-is.
                file_path.to_string()
            };
            if abs.starts_with(root.as_ref()) {
                abs[root.len()..].trim_start_matches('/').to_string()
            } else {
                abs
            }
        };

        // Look up the file record.
        let file_row: Option<(i64, i64, String)> = self
            .conn
            .query_row(
                "SELECT f.id, f.project_id, f.language
                 FROM files f
                 JOIN projects p ON p.id = f.project_id
                 WHERE f.path = ?1
                 LIMIT 1",
                params![rel_path],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|e| format!("failed to look up file: {e}"))?;

        let (file_id, project_id, language) = match file_row {
            Some(r) => r,
            None => return Ok(None),
        };

        // Query imports.
        let mut stmt = self
            .conn
            .prepare(
                "SELECT ref_text, line FROM unresolved_refs
                 WHERE file_id = ?1 AND project_id = ?2 AND ref_kind = 'import'
                 ORDER BY line ASC",
            )
            .map_err(|e| format!("failed to prepare import query: {e}"))?;

        let imports: Vec<String> = stmt
            .query_map(params![file_id, project_id], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|e| format!("failed to query imports: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        // Query symbol nodes (skip the file-level node itself).
        let mut stmt = self
            .conn
            .prepare(
                "SELECT kind, name, COALESCE(signature, ''), COALESCE(visibility, ''),
                        start_line, COALESCE(is_async, 0), COALESCE(is_static, 0)
                 FROM nodes
                 WHERE file_id = ?1 AND project_id = ?2 AND kind != 'file'
                 ORDER BY start_line ASC",
            )
            .map_err(|e| format!("failed to prepare nodes query: {e}"))?;

        let symbols: Vec<SymbolEntry> = stmt
            .query_map(params![file_id, project_id], |row| {
                Ok(SymbolEntry {
                    kind:       row.get(0)?,
                    name:       row.get(1)?,
                    signature:  row.get(2)?,
                    visibility: row.get(3)?,
                    line:       row.get(4)?,
                    is_async:   row.get::<_, i64>(5)? != 0,
                    is_static:  row.get::<_, i64>(6)? != 0,
                })
            })
            .map_err(|e| format!("failed to query symbols: {e}"))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(Some(FileOutline { language, imports, symbols }))
    }
}

// ---------------------------------------------------------------------------
// Outline data types
// ---------------------------------------------------------------------------

/// Structured outline of a single file as stored in the graph DB.
#[derive(Debug)]
pub struct FileOutline {
    pub language: String,
    pub imports:  Vec<String>,
    pub symbols:  Vec<SymbolEntry>,
}

/// A single symbol (function, struct, class, …) extracted from a file.
#[derive(Debug)]
pub struct SymbolEntry {
    pub kind:       String,
    pub name:       String,
    pub signature:  String,
    pub visibility: String,
    pub line:       i64,
    pub is_async:   bool,
    pub is_static:  bool,
}
/// A single reference entry returned by [`GraphDb::query_references`].
#[derive(Debug)]
pub struct ReferenceEntry {
    /// Project-relative file path where the reference appears.
    pub file:     String,
    /// Line number of the reference.
    pub line:     i64,
    /// Kind of reference: `caller`, `callee`, `import`, `declaration`,
    /// or `calls:inbound` / `calls:outbound` in `all` mode.
    pub ref_kind: String,
    /// Name of the referencing (or referenced) symbol.
    pub symbol:   String,
    /// Tree-sitter kind of the referencing symbol.
    pub sym_kind: String,
    /// Optional surrounding code snippet (not populated by default; reserved for future use).
    #[allow(dead_code)]
    pub snippet:  Option<String>,
}

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
