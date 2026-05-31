use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use rusqlite::Connection;

use crate::core_events::events_db_path;

static SHELL_METRICS_DB: OnceLock<Mutex<Connection>> = OnceLock::new();

pub(super) fn open_db() -> Result<&'static Mutex<Connection>> {
    if let Some(db) = SHELL_METRICS_DB.get() {
        return Ok(db);
    }

    let db_path = events_db_path();
    ensure_parent_dir(&db_path)?;
    let conn = Connection::open(&db_path)
        .with_context(|| format!("failed to open shell metrics db: {}", db_path.display()))?;
    init_schema(&conn)?;
    let _ = SHELL_METRICS_DB.set(Mutex::new(conn));

    Ok(SHELL_METRICS_DB
        .get()
        .expect("shell metrics db should be initialized"))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn persist_to_path(
    db_path: &Path,
    write_row: impl FnOnce(&Connection) -> Result<()>,
) -> Result<()> {
    ensure_parent_dir(db_path)?;

    let conn = Connection::open(db_path)
        .with_context(|| format!("failed to open shell metrics db: {}", db_path.display()))?;
    init_schema(&conn)?;
    write_row(&conn)
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create shell metrics dir: {}", parent.display()))?;
    }
    Ok(())
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS shell_runs (
            id INTEGER PRIMARY KEY,
            captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            run_id TEXT NOT NULL,
            family TEXT NOT NULL,
            command_line TEXT NOT NULL,
            argv_json TEXT NOT NULL,
            exit_code INTEGER NOT NULL,
            raw_stdout_bytes INTEGER NOT NULL,
            raw_stderr_bytes INTEGER NOT NULL,
            compressed_bytes INTEGER NOT NULL,
            render_mode TEXT NOT NULL DEFAULT 'compressed'
        );
        CREATE INDEX IF NOT EXISTS idx_shell_runs_captured_at ON shell_runs(captured_at);
        CREATE INDEX IF NOT EXISTS idx_shell_runs_family ON shell_runs(family);",
    )
    .context("failed to initialize shell metrics schema")?;

    let existing = table_columns(conn, "shell_runs")?;
    if needs_schema_rebuild(&existing) {
        rebuild_shell_runs_table(conn, &existing)?;
    }

    let existing = table_columns(conn, "shell_runs")?;
    if !existing.iter().any(|column| column == "run_id") {
        conn.execute(
            "ALTER TABLE shell_runs ADD COLUMN run_id TEXT NOT NULL DEFAULT ''",
            [],
        )
        .context("failed to add run_id column to shell_runs")?;
        conn.execute(
            "UPDATE shell_runs SET run_id = printf('legacy-%d', id) WHERE run_id = ''",
            [],
        )
        .context("failed to backfill legacy run_id values")?;
    }
    if !existing.iter().any(|column| column == "render_mode") {
        conn.execute(
            "ALTER TABLE shell_runs ADD COLUMN render_mode TEXT NOT NULL DEFAULT 'compressed'",
            [],
        )
        .context("failed to add render_mode column to shell_runs")?;
    }
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_shell_runs_run_id ON shell_runs(run_id)",
        [],
    )
    .context("failed to ensure run_id index on shell_runs")?;

    Ok(())
}

fn needs_schema_rebuild(existing: &[String]) -> bool {
    existing.iter().any(|column| {
        matches!(
            column.as_str(),
            "raw_capture" | "raw_stdout" | "raw_stderr" | "capture_requested"
        )
    })
}

fn rebuild_shell_runs_table(conn: &Connection, existing: &[String]) -> Result<()> {
    let run_id_expr = if existing.iter().any(|column| column == "run_id") {
        "NULLIF(run_id, '')"
    } else {
        "NULL"
    };
    let render_mode_expr = if existing.iter().any(|column| column == "render_mode") {
        "render_mode"
    } else {
        "'compressed'"
    };

    conn.execute_batch("ALTER TABLE shell_runs RENAME TO shell_runs_legacy;")
        .context("failed to rename legacy shell_runs table")?;
    conn.execute_batch(
        "CREATE TABLE shell_runs (
            id INTEGER PRIMARY KEY,
            captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            run_id TEXT NOT NULL,
            family TEXT NOT NULL,
            command_line TEXT NOT NULL,
            argv_json TEXT NOT NULL,
            exit_code INTEGER NOT NULL,
            raw_stdout_bytes INTEGER NOT NULL,
            raw_stderr_bytes INTEGER NOT NULL,
            compressed_bytes INTEGER NOT NULL,
            render_mode TEXT NOT NULL DEFAULT 'compressed'
        );",
    )
    .context("failed to create rebuilt shell_runs table")?;

    let copy_sql = format!(
        "INSERT INTO shell_runs(
            id,
            captured_at,
            run_id,
            family,
            command_line,
            argv_json,
            exit_code,
            raw_stdout_bytes,
            raw_stderr_bytes,
            compressed_bytes,
            render_mode
        )
        SELECT
            id,
            captured_at,
            COALESCE({run_id_expr}, printf('legacy-%d', id)),
            family,
            command_line,
            argv_json,
            exit_code,
            raw_stdout_bytes,
            raw_stderr_bytes,
            compressed_bytes,
            {render_mode_expr}
        FROM shell_runs_legacy;"
    );
    conn.execute_batch(&copy_sql)
        .context("failed to copy data into rebuilt shell_runs table")?;
    conn.execute_batch("DROP TABLE shell_runs_legacy;")
        .context("failed to drop legacy shell_runs table")?;

    Ok(())
}

pub(crate) fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .with_context(|| format!("failed to inspect schema for table: {table}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .context("failed to read schema columns")?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("failed to collect schema columns")
}
