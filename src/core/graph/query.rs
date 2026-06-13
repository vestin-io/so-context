use rusqlite::{OptionalExtension, params};
use std::collections::HashSet;

use super::db::GraphDb;

impl GraphDb {
    /// FTS search over indexed symbols. Returns formatted results and total
    /// token count of matched files (used for token saving estimates).
    pub fn search_with_stats(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<(String, i64, i64), String> {
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
        let mut matched_paths = HashSet::new();
        for row in rows {
            let (path, kind, name, line, score) =
                row.map_err(|e| format!("failed to read row: {e}"))?;
            out.push(format!(
                "{path}:{line} [{kind}] {name} (score={:.4})",
                score.abs()
            ));
            matched_paths.insert(path);
        }

        let (matched_files_tokens, matched_files_size) = if matched_paths.is_empty() {
            (0i64, 0i64)
        } else {
            let placeholders = std::iter::repeat_n("?", matched_paths.len())
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT COALESCE(SUM(COALESCE(token_count, 0)), 0),
                        COALESCE(SUM(COALESCE(size_bytes, 0)), 0)
                 FROM files
                 WHERE path IN ({placeholders})"
            );
            let params: Vec<&dyn rusqlite::ToSql> = matched_paths
                .iter()
                .map(|path| path as &dyn rusqlite::ToSql)
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

    /// Returns all references to (or from) a named symbol, filtered by `kind`.
    pub fn query_references(
        &self,
        symbol: &str,
        kind: &str,
        include_declaration: bool,
    ) -> Result<Vec<ReferenceEntry>, String> {
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
            .filter_map(|row| row.ok())
            .collect();

        if target_nodes.is_empty() {
            return Ok(vec![]);
        }

        let mut results = Vec::new();

        if include_declaration {
            for (_, path, sym_kind, line) in &target_nodes {
                results.push(ReferenceEntry {
                    file: path.clone(),
                    line: *line,
                    ref_kind: "declaration".to_string(),
                    symbol: symbol.to_string(),
                    sym_kind: sym_kind.clone(),
                    snippet: None,
                });
            }
        }

        for (node_id, _, _, _) in &target_nodes {
            match kind {
                "callers" => {
                    let mut stmt = self
                        .conn
                        .prepare(
                            "SELECT f.path, caller.name, caller.kind, e.line
                         FROM edges e
                         JOIN nodes caller ON caller.id = e.from_node_id
                         JOIN files  f     ON f.id = caller.file_id
                         WHERE e.to_node_id = ?1 AND e.kind = 'calls'",
                        )
                        .map_err(|e| format!("failed to prepare callers query: {e}"))?;
                    let rows = stmt
                        .query_map(params![node_id], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, i64>(3)?,
                            ))
                        })
                        .map_err(|e| format!("failed to query callers: {e}"))?;
                    for row in rows.filter_map(|entry| entry.ok()) {
                        results.push(ReferenceEntry {
                            file: row.0,
                            line: row.3,
                            ref_kind: "caller".to_string(),
                            symbol: row.1,
                            sym_kind: row.2,
                            snippet: None,
                        });
                    }
                }
                "callees" => {
                    let mut stmt = self
                        .conn
                        .prepare(
                            "SELECT f.path, callee.name, callee.kind, e.line
                         FROM edges e
                         JOIN nodes callee ON callee.id = e.to_node_id
                         JOIN files  f     ON f.id = callee.file_id
                         WHERE e.from_node_id = ?1 AND e.kind = 'calls'",
                        )
                        .map_err(|e| format!("failed to prepare callees query: {e}"))?;
                    let rows = stmt
                        .query_map(params![node_id], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, i64>(3)?,
                            ))
                        })
                        .map_err(|e| format!("failed to query callees: {e}"))?;
                    for row in rows.filter_map(|entry| entry.ok()) {
                        results.push(ReferenceEntry {
                            file: row.0,
                            line: row.3,
                            ref_kind: "callee".to_string(),
                            symbol: row.1,
                            sym_kind: row.2,
                            snippet: None,
                        });
                    }
                }
                "imports" => {
                    let mut stmt = self
                        .conn
                        .prepare(
                            "SELECT f.path, importer.name, importer.kind, e.line
                         FROM edges e
                         JOIN nodes importer ON importer.id = e.from_node_id
                         JOIN files  f       ON f.id = importer.file_id
                         WHERE e.to_node_id = ?1 AND e.kind = 'imports'",
                        )
                        .map_err(|e| format!("failed to prepare imports query: {e}"))?;
                    let rows = stmt
                        .query_map(params![node_id], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, i64>(3)?,
                            ))
                        })
                        .map_err(|e| format!("failed to query imports: {e}"))?;
                    for row in rows.filter_map(|entry| entry.ok()) {
                        results.push(ReferenceEntry {
                            file: row.0,
                            line: row.3,
                            ref_kind: "import".to_string(),
                            symbol: row.1,
                            sym_kind: row.2,
                            snippet: None,
                        });
                    }
                }
                _ => {
                    let mut stmt = self
                        .conn
                        .prepare(
                            "SELECT f.path, other.name, other.kind, e.line, e.kind,
                                CASE WHEN e.to_node_id = ?1 THEN 'inbound' ELSE 'outbound' END
                         FROM edges e
                         JOIN nodes other ON other.id = CASE
                             WHEN e.to_node_id = ?1 THEN e.from_node_id
                             ELSE e.to_node_id END
                         JOIN files f ON f.id = other.file_id
                         WHERE (e.from_node_id = ?1 OR e.to_node_id = ?1)
                           AND e.kind IN ('calls', 'imports')",
                        )
                        .map_err(|e| format!("failed to prepare all-refs query: {e}"))?;
                    let rows = stmt
                        .query_map(params![node_id], |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, String>(2)?,
                                row.get::<_, i64>(3)?,
                                row.get::<_, String>(4)?,
                                row.get::<_, String>(5)?,
                            ))
                        })
                        .map_err(|e| format!("failed to query all refs: {e}"))?;
                    for row in rows.filter_map(|entry| entry.ok()) {
                        let ref_kind = format!("{}:{}", row.4, row.5);
                        results.push(ReferenceEntry {
                            file: row.0,
                            line: row.3,
                            ref_kind,
                            symbol: row.1,
                            sym_kind: row.2,
                            snippet: None,
                        });
                    }
                }
            }
        }

        results.sort_by(|left, right| left.file.cmp(&right.file).then(left.line.cmp(&right.line)));
        results.dedup_by(|left, right| {
            left.file == right.file
                && left.line == right.line
                && left.ref_kind == right.ref_kind
                && left.symbol == right.symbol
        });

        Ok(results)
    }

    /// Returns a structured outline for a single file given its project-relative
    /// or absolute path. Returns `None` when the file is not indexed.
    pub fn query_file_outline(&self, file_path: &str) -> Result<Option<FileOutline>, String> {
        let rel_path = {
            let root = self.project_root.to_string_lossy();
            let abs = file_path.to_string();
            if abs.starts_with(root.as_ref()) {
                abs[root.len()..].trim_start_matches('/').to_string()
            } else {
                abs
            }
        };

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
            Some(row) => row,
            None => return Ok(None),
        };

        let mut stmt = self
            .conn
            .prepare(
                "SELECT ref_text, line FROM unresolved_refs
                 WHERE file_id = ?1 AND project_id = ?2 AND ref_kind = 'import'
                 ORDER BY line ASC",
            )
            .map_err(|e| format!("failed to prepare import query: {e}"))?;

        let imports: Vec<String> = stmt
            .query_map(params![file_id, project_id], |row| row.get::<_, String>(0))
            .map_err(|e| format!("failed to query imports: {e}"))?
            .filter_map(|row| row.ok())
            .collect();

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
                    kind: row.get(0)?,
                    name: row.get(1)?,
                    signature: row.get(2)?,
                    visibility: row.get(3)?,
                    line: row.get(4)?,
                    is_async: row.get::<_, i64>(5)? != 0,
                    is_static: row.get::<_, i64>(6)? != 0,
                })
            })
            .map_err(|e| format!("failed to query symbols: {e}"))?
            .filter_map(|row| row.ok())
            .collect();

        Ok(Some(FileOutline {
            language,
            imports,
            symbols,
        }))
    }
}

#[derive(Debug)]
pub struct FileOutline {
    pub language: String,
    pub imports: Vec<String>,
    pub symbols: Vec<SymbolEntry>,
}

#[derive(Debug)]
pub struct SymbolEntry {
    pub kind: String,
    pub name: String,
    pub signature: String,
    pub visibility: String,
    pub line: i64,
    pub is_async: bool,
    pub is_static: bool,
}

#[derive(Debug)]
pub struct ReferenceEntry {
    pub file: String,
    pub line: i64,
    pub ref_kind: String,
    pub symbol: String,
    pub sym_kind: String,
    #[allow(dead_code)]
    pub snippet: Option<String>,
}

fn build_fts_query(input: &str) -> String {
    let cleaned = input
        .replace("::", " ")
        .chars()
        .filter(|ch| !matches!(ch, '\'' | '"' | '*' | '(' | ')' | ':' | '^'))
        .collect::<String>();

    cleaned
        .split_whitespace()
        .filter(|term| {
            let upper = term.to_ascii_uppercase();
            !matches!(upper.as_str(), "AND" | "OR" | "NOT" | "NEAR")
        })
        .map(|term| format!("\"{term}\"*"))
        .collect::<Vec<_>>()
        .join(" OR ")
}
