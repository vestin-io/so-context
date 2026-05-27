use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher,
};
use rusqlite::{Connection, params};
use tree_sitter::{Parser, TreeCursor};
use tree_sitter_language_pack::{detect_language_from_path, get_language};
use walkdir::WalkDir;

pub fn index_project(project_path: &str) -> Result<String, String> {
    let project_root = PathBuf::from(project_path);
    if !project_root.is_dir() {
        return Err(format!("path is not a directory: {}", project_root.display()));
    }

    let db_path = project_root.join(".so-context").join("graph.db");
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("failed to create db dir: {e}"))?;
    }

    let mut conn = Connection::open(&db_path).map_err(|e| format!("failed to open db: {e}"))?;
    init_schema(&conn)?;
    let root_str = project_root.to_string_lossy().to_string();

    let tx = conn
        .transaction()
        .map_err(|e| format!("failed to start tx: {e}"))?;
    tx.execute(
        "INSERT INTO projects(root_path) VALUES (?1)
         ON CONFLICT(root_path) DO UPDATE SET updated_at = CURRENT_TIMESTAMP",
        params![root_str],
    )
    .map_err(|e| format!("failed to upsert project row: {e}"))?;
    let project_id: i64 = tx
        .query_row(
            "SELECT id FROM projects WHERE root_path = ?1",
            params![root_str],
            |row| row.get(0),
        )
        .map_err(|e| format!("failed to load project id: {e}"))?;
    tx.execute("DELETE FROM nodes WHERE project_id = ?1", params![project_id])
        .map_err(|e| format!("failed to clear nodes: {e}"))?;
    tx.execute("DELETE FROM files WHERE project_id = ?1", params![project_id])
        .map_err(|e| format!("failed to clear files: {e}"))?;

    let mut parser = Parser::new();
    let mut file_count = 0_i64;
    let mut symbol_count = 0_i64;

    for entry in WalkDir::new(&project_root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if !entry.file_type().is_file() || should_skip(path) {
            continue;
        }

        let path_str = path.to_string_lossy();
        let Some(lang_name) = detect_language_from_path(&path_str) else {
            continue;
        };
        let language = get_language(&lang_name)
            .map_err(|e| format!("failed to load tree-sitter language '{lang_name}': {e}"))?;
        parser
            .set_language(&language)
            .map_err(|e| format!("failed to set parser language: {e}"))?;

        let content = fs::read_to_string(path).map_err(|e| format!("failed to read file: {e}"))?;
        let Some(tree) = parser.parse(&content, None) else {
            continue;
        };

        let rel_path = path
            .strip_prefix(&project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        tx.execute(
            "INSERT INTO files(project_id, path, language) VALUES (?1, ?2, ?3)",
            params![project_id, rel_path, lang_name],
        )
        .map_err(|e| format!("failed to insert file row: {e}"))?;
        let file_id = tx.last_insert_rowid();
        file_count += 1;

        let mut cursor = tree.walk();
        symbol_count += collect_symbols(&tx, &content, project_id, file_id, &mut cursor)?;
    }

    // Keep FTS content in sync with the current project snapshot.
    tx.execute(
        "DELETE FROM nodes_fts WHERE rowid IN (
            SELECT n.id FROM nodes n WHERE n.project_id = ?1
        )",
        params![project_id],
    )
    .map_err(|e| format!("failed to clear project fts rows: {e}"))?;
    tx.execute(
        "INSERT INTO nodes_fts(rowid, name, fq_name, signature, doc, path)
         SELECT n.id, n.name, COALESCE(n.fq_name, ''), COALESCE(n.signature, ''), COALESCE(n.doc, ''), f.path
         FROM nodes n
         JOIN files f ON f.id = n.file_id
         WHERE n.project_id = ?1",
        params![project_id],
    )
    .map_err(|e| format!("failed to populate fts rows: {e}"))?;

    tx.commit().map_err(|e| format!("failed to commit tx: {e}"))?;
    Ok(format!(
        "Graph indexed: files={} symbols={} db={}",
        file_count,
        symbol_count,
        db_path.display()
    ))
}

pub fn watch_project(project_path: &str) -> Result<(), String> {
    let project_root = PathBuf::from(project_path);
    if !project_root.is_dir() {
        return Err(format!("path is not a directory: {}", project_root.display()));
    }

    let first = index_project(project_path)?;
    println!("{first}");
    println!("Watching for file changes: {}", project_root.display());

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher: RecommendedWatcher = recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| format!("failed to create watcher: {e}"))?;

    watcher
        .watch(&project_root, RecursiveMode::Recursive)
        .map_err(|e| format!("failed to watch path: {e}"))?;

    let mut last_reindex = Instant::now() - Duration::from_secs(10);
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(event)) => {
                if !is_meaningful_change(&event) {
                    continue;
                }
                if last_reindex.elapsed() < Duration::from_millis(700) {
                    continue;
                }
                last_reindex = Instant::now();
                match index_project(project_path) {
                    Ok(summary) => println!("{summary}"),
                    Err(err) => eprintln!("reindex failed: {err}"),
                }
            }
            Ok(Err(err)) => {
                eprintln!("watch error: {err}");
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                return Err("watch channel disconnected".to_string());
            }
        }
    }
}

pub fn search_project(project_path: &str, query: &str, limit: usize) -> Result<String, String> {
    let project_root = PathBuf::from(project_path);
    if !project_root.is_dir() {
        return Err(format!("path is not a directory: {}", project_root.display()));
    }
    let db_path = project_root.join(".so-context").join("graph.db");
    if !db_path.exists() {
        return Err(format!(
            "graph db not found at {} (run index first)",
            db_path.display()
        ));
    }
    let conn = Connection::open(&db_path).map_err(|e| format!("failed to open db: {e}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT f.path, n.kind, n.name, n.start_line
             FROM nodes_fts fts
             JOIN nodes n ON n.id = fts.rowid
             JOIN files f ON f.id = n.file_id
             WHERE fts MATCH ?1
             LIMIT ?2",
        )
        .map_err(|e| format!("failed to prepare search query: {e}"))?;

    let rows = stmt
        .query_map(params![query, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| format!("failed to execute search query: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        let (path, kind, name, line) = row.map_err(|e| format!("failed to read row: {e}"))?;
        out.push(format!("{path}:{line} [{kind}] {name}"));
    }

    if out.is_empty() {
        Ok("No results.".to_string())
    } else {
        Ok(out.join("\n"))
    }
}

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(include_str!("graph_schema.sql"))
    .map_err(|e| format!("failed to init schema: {e}"))
}

fn should_skip(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/.git/")
        || s.contains("/node_modules/")
        || s.contains("/target/")
        || s.contains("/.so-context/")
}

fn is_meaningful_change(event: &Event) -> bool {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return false,
    }
    event.paths.iter().any(|p| {
        let s = p.to_string_lossy();
        !s.contains("/.git/") && !s.contains("/node_modules/") && !s.contains("/target/")
    })
}

fn collect_symbols(
    tx: &rusqlite::Transaction<'_>,
    content: &str,
    project_id: i64,
    file_id: i64,
    cursor: &mut TreeCursor<'_>,
) -> Result<i64, String> {
    let mut count = 0_i64;
    let mut stack = vec![cursor.node()];
    while let Some(node) = stack.pop() {
        let kind = node.kind();
        if is_symbol_kind(kind) {
            if let Some(name_node) = node.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(content.as_bytes()) {
                    let p = node.start_position();
                    let end = node.end_position();
                    tx.execute(
                        "INSERT INTO nodes(
                            project_id, file_id, kind, name, start_line, start_col, end_line, end_col
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            project_id,
                            file_id,
                            kind,
                            name,
                            p.row as i64 + 1,
                            p.column as i64 + 1,
                            end.row as i64 + 1,
                            end.column as i64 + 1
                        ],
                    )
                    .map_err(|e| format!("failed to insert symbol row: {e}"))?;
                    count += 1;
                }
            }
        }

        let mut walk = node.walk();
        if walk.goto_first_child() {
            loop {
                stack.push(walk.node());
                if !walk.goto_next_sibling() {
                    break;
                }
            }
        }
    }
    Ok(count)
}

fn is_symbol_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "function_declaration"
            | "function_definition"
            | "method_definition"
            | "method_declaration"
            | "class_declaration"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "impl_item"
    )
}
