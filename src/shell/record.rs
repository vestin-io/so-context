use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use super::types::{CompressionSummary, ShellResult};

const SHELL_DB_DIR: &str = ".so-context";
const SHELL_DB_NAME: &str = "shell.db";

pub fn persist(
    result: &ShellResult,
    compressed: &CompressionSummary,
    compressed_rendered: &str,
    record_raw: bool,
) -> Result<()> {
    let cwd = std::env::current_dir().context("failed to resolve current working directory")?;
    let db_path = shell_db_path(&cwd);
    persist_to_path(
        &db_path,
        result,
        compressed,
        compressed_rendered,
        record_raw,
    )
}

fn persist_to_path(
    db_path: &Path,
    result: &ShellResult,
    compressed: &CompressionSummary,
    compressed_rendered: &str,
    record_raw: bool,
) -> Result<()> {
    ensure_parent_dir(db_path)?;

    let conn = Connection::open(db_path)
        .with_context(|| format!("failed to open shell metrics db: {}", db_path.display()))?;
    init_schema(&conn)?;

    let argv_json =
        serde_json::to_string(&result.invocation.argv).context("failed to encode argv json")?;
    let raw_stdout = record_raw.then_some(result.stdout.as_str());
    let raw_stderr = record_raw.then_some(result.stderr.as_str());

    conn.execute(
        "INSERT INTO shell_runs(
            family,
            command_line,
            argv_json,
            exit_code,
            raw_stdout_bytes,
            raw_stderr_bytes,
            compressed_bytes,
            raw_capture,
            raw_stdout,
            raw_stderr
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            compressed.pattern.label(),
            result.invocation.command_line(),
            argv_json,
            result.exit_code,
            result.stdout.len() as i64,
            result.stderr.len() as i64,
            compressed_rendered.len() as i64,
            record_raw,
            raw_stdout,
            raw_stderr,
        ],
    )
    .context("failed to insert shell metrics row")?;

    Ok(())
}

fn shell_db_path(project_root: &Path) -> PathBuf {
    project_root.join(SHELL_DB_DIR).join(SHELL_DB_NAME)
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create shell db dir: {}", parent.display()))?;
    }
    Ok(())
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS shell_runs (
            id INTEGER PRIMARY KEY,
            captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            family TEXT NOT NULL,
            command_line TEXT NOT NULL,
            argv_json TEXT NOT NULL,
            exit_code INTEGER NOT NULL,
            raw_stdout_bytes INTEGER NOT NULL,
            raw_stderr_bytes INTEGER NOT NULL,
            compressed_bytes INTEGER NOT NULL,
            raw_capture INTEGER NOT NULL DEFAULT 0,
            raw_stdout TEXT,
            raw_stderr TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_shell_runs_captured_at ON shell_runs(captured_at);
        CREATE INDEX IF NOT EXISTS idx_shell_runs_family ON shell_runs(family);",
    )
    .context("failed to initialize shell metrics schema")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::types::{CompressionSummary, ShellInvocation, ShellPattern};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn persists_metrics_without_raw_payloads_by_default() {
        let temp_root = temp_test_dir("shell-record-no-raw");
        let db_path = shell_db_path(&temp_root);
        let result = sample_result();
        let summary = sample_summary();

        persist_to_path(&db_path, &result, &summary, "compressed output", false).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let row = conn
            .query_row(
                "SELECT family, raw_stdout_bytes, compressed_bytes, raw_capture, raw_stdout, raw_stderr
                 FROM shell_runs LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, bool>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(row.0, "git.diff");
        assert_eq!(row.1, result.stdout.len() as i64);
        assert_eq!(row.2, "compressed output".len() as i64);
        assert!(!row.3);
        assert!(row.4.is_none());
        assert!(row.5.is_none());

        let _ = fs::remove_dir_all(temp_root);
    }

    #[test]
    fn persists_raw_payloads_when_enabled() {
        let temp_root = temp_test_dir("shell-record-with-raw");
        let db_path = shell_db_path(&temp_root);
        let result = sample_result();
        let summary = sample_summary();

        persist_to_path(&db_path, &result, &summary, "compressed output", true).unwrap();

        let conn = Connection::open(&db_path).unwrap();
        let row = conn
            .query_row(
                "SELECT raw_capture, raw_stdout, raw_stderr FROM shell_runs LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, bool>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .unwrap();

        assert!(row.0);
        assert_eq!(row.1.as_deref(), Some(result.stdout.as_str()));
        assert_eq!(row.2.as_deref(), Some(result.stderr.as_str()));

        let _ = fs::remove_dir_all(temp_root);
    }

    fn sample_result() -> ShellResult {
        ShellResult {
            invocation: ShellInvocation::new(vec!["git".into(), "diff".into()]),
            stdout: "diff output\n".into(),
            stderr: "warning\n".into(),
            exit_code: 0,
        }
    }

    fn sample_summary() -> CompressionSummary {
        CompressionSummary {
            pattern: ShellPattern::GitDiff,
            summary: "files=1; hunks=1; additions=1; deletions=1".into(),
            details: vec!["src/main.rs (+1/-1, 1 hunk)".into()],
            stderr_preview: vec!["warning".into()],
            exit_code: 0,
            command_line: "git diff".into(),
        }
    }

    fn temp_test_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
