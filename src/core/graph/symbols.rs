//! Tree-sitter symbol extraction and reference edge resolution.

use rusqlite::{Transaction, params};
use tree_sitter::TreeCursor;

// ---------------------------------------------------------------------------
// Symbol collection
// ---------------------------------------------------------------------------

/// Walks a syntax tree and inserts supported symbol nodes + `contains` edges into SQLite.
/// Returns the number of symbols inserted.
pub(super) fn collect_symbols(
    tx: &Transaction<'_>,
    content: &str,
    project_id: i64,
    file_id: i64,
    file_node_id: i64,
    cursor: &mut TreeCursor<'_>,
) -> Result<i64, String> {
    let mut count = 0_i64;
    let mut stack = vec![cursor.node()];

    while let Some(node) = stack.pop() {
        let kind = node.kind();

        if is_symbol_kind(kind)
            && let Some(name_node) = node.child_by_field_name("name")
            && let Ok(name) = name_node.utf8_text(content.as_bytes())
        {
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
            let symbol_id = tx.last_insert_rowid();

            tx.execute(
                "INSERT INTO edges(project_id, from_node_id, to_node_id, kind, line, col)
                 VALUES (?1, ?2, ?3, 'contains', ?4, ?5)",
                params![
                    project_id,
                    file_node_id,
                    symbol_id,
                    p.row as i64 + 1,
                    p.column as i64 + 1
                ],
            )
            .map_err(|e| format!("failed to insert contains edge: {e}"))?;
            count += 1;
        }

        if let Some(ref_kind) = relation_kind(kind) {
            let p = node.start_position();
            if let Ok(raw) = node.utf8_text(content.as_bytes()) {
                let ref_text = normalize_ref(raw);
                if !ref_text.is_empty() {
                    tx.execute(
                        "INSERT INTO unresolved_refs(
                            project_id, file_id, ref_kind, ref_text, line, col
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            project_id,
                            file_id,
                            ref_kind,
                            ref_text,
                            p.row as i64 + 1,
                            p.column as i64 + 1
                        ],
                    )
                    .map_err(|e| format!("failed to insert unresolved ref: {e}"))?;
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

// ---------------------------------------------------------------------------
// Reference edge resolution
// ---------------------------------------------------------------------------

/// Resolves unresolved call/import refs into concrete graph edges by symbol name.
pub(super) fn resolve_reference_edges(tx: &Transaction<'_>, project_id: i64) -> Result<(), String> {
    // Clear existing resolved edges before re-resolving to avoid duplicates.
    tx.execute(
        "DELETE FROM edges WHERE project_id=?1 AND kind IN ('calls','imports')",
        params![project_id],
    )
    .map_err(|e| format!("failed to clear resolved edges: {e}"))?;

    tx.execute(
        "INSERT INTO edges(project_id, from_node_id, to_node_id, kind, line, col)
         SELECT ur.project_id, fn.id, n.id,
                CASE ur.ref_kind WHEN 'call' THEN 'calls' ELSE 'imports' END,
                ur.line, ur.col
         FROM unresolved_refs ur
         JOIN nodes fn
           ON fn.project_id = ur.project_id
          AND fn.file_id = ur.file_id
          AND fn.kind = 'file'
         JOIN nodes n
           ON n.project_id = ur.project_id
          AND n.name = ur.ref_text
         WHERE ur.project_id = ?1
           AND ur.ref_kind IN ('call', 'import')
           AND n.kind != 'file'",
        params![project_id],
    )
    .map_err(|e| format!("failed to resolve reference edges: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Classification helpers
// ---------------------------------------------------------------------------

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

fn relation_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "call_expression" | "call" => Some("call"),
        "import_statement" | "import_from_statement" | "import_declaration" | "use_declaration" => {
            Some("import")
        }
        _ => None,
    }
}

fn normalize_ref(raw: &str) -> String {
    let mut token = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == ':' || ch == '.' {
            token.push(ch);
        } else {
            token.push(' ');
        }
    }
    let last = token
        .split_whitespace()
        .next_back()
        .unwrap_or_default()
        .rsplit([':', '.'])
        .next()
        .unwrap_or_default()
        .trim();
    last.to_string()
}
