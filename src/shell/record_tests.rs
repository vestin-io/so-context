use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::shell::types::{ShellInvocation, ShellOutputMode, ShellPattern};

#[test]
fn persists_metrics_without_payload_columns() {
    let db_path = temp_db_path("shell-record-no-payloads");
    let result = sample_result();

    persist_to_path(&db_path, |conn| {
        insert_shell_run(
            conn,
            "run-1",
            &result,
            ShellPattern::GitDiff,
            "compressed output",
            ShellOutputMode::Compressed,
        )
    })
    .unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let row = conn
        .query_row(
            "SELECT run_id, family, raw_stdout_bytes, compressed_bytes, render_mode
             FROM shell_runs LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .unwrap();

    assert_eq!(row.0, "run-1");
    assert_eq!(row.1, "git.diff");
    assert_eq!(row.2, result.stdout.len() as i64);
    assert_eq!(row.3, "compressed output".len() as i64);
    assert_eq!(row.4, "compressed");
    assert!(
        !table_columns(&conn, "shell_runs")
            .unwrap()
            .contains(&"raw_stdout".to_string())
    );
    assert!(
        !table_columns(&conn, "shell_runs")
            .unwrap()
            .contains(&"raw_stderr".to_string())
    );

    let _ = fs::remove_file(db_path);
}

#[test]
fn reinitializes_schema_for_each_db_path() {
    let first = temp_db_path("shell-record-first");
    let second = temp_db_path("shell-record-second");
    let result = sample_result();

    persist_to_path(&first, |conn| {
        insert_shell_run(
            conn,
            "run-first",
            &result,
            ShellPattern::GitDiff,
            "compressed output",
            ShellOutputMode::Compressed,
        )
    })
    .unwrap();
    persist_to_path(&second, |conn| {
        insert_shell_run(
            conn,
            "run-second",
            &result,
            ShellPattern::GitDiff,
            "compressed output",
            ShellOutputMode::Compressed,
        )
    })
    .unwrap();

    let first_conn = Connection::open(&first).unwrap();
    let second_conn = Connection::open(&second).unwrap();
    let first_count: i64 = first_conn
        .query_row("SELECT COUNT(*) FROM shell_runs", [], |row| row.get(0))
        .unwrap();
    let second_count: i64 = second_conn
        .query_row("SELECT COUNT(*) FROM shell_runs", [], |row| row.get(0))
        .unwrap();

    assert_eq!(first_count, 1);
    assert_eq!(second_count, 1);

    let _ = fs::remove_file(first);
    let _ = fs::remove_file(second);
}

#[test]
fn migrates_legacy_raw_capture_schema() {
    let db_path = temp_db_path("shell-record-legacy-schema");
    let conn = Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE shell_runs (
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
        );",
    )
    .unwrap();
    drop(conn);

    let result = sample_result();
    persist_to_path(&db_path, |conn| {
        insert_shell_run(
            conn,
            "run-legacy",
            &result,
            ShellPattern::GitDiff,
            "compressed output",
            ShellOutputMode::Compressed,
        )
    })
    .unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let row = conn
        .query_row(
            "SELECT run_id, command_line, render_mode FROM shell_runs LIMIT 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .unwrap();

    assert_eq!(row.0, "run-legacy");
    assert_eq!(row.1, "git diff");
    assert_eq!(row.2, "compressed");
    let columns = table_columns(&conn, "shell_runs").unwrap();
    assert!(!columns.contains(&"raw_capture".to_string()));
    assert!(!columns.contains(&"raw_stdout".to_string()));
    assert!(!columns.contains(&"raw_stderr".to_string()));
    assert!(!columns.contains(&"capture_requested".to_string()));

    let _ = fs::remove_file(db_path);
}

#[test]
fn redacts_sensitive_command_args_before_persisting() {
    let db_path = temp_db_path("shell-record-redacted-argv");
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "curl".into(),
            "-H".into(),
            "Authorization: Bearer super-secret".into(),
            "https://api.example.test/data?access_token=abc123&foo=bar".into(),
            "two words".into(),
        ]),
        stdout: String::new(),
        stderr: String::new(),
        exit_code: 0,
    };

    persist_to_path(&db_path, |conn| {
        insert_shell_run(
            conn,
            "run-redacted",
            &result,
            ShellPattern::GitDiff,
            "compressed output",
            ShellOutputMode::Compressed,
        )
    })
    .unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let row = conn
        .query_row(
            "SELECT command_line, argv_json FROM shell_runs LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap();

    assert!(row.0.contains("[REDACTED]"));
    assert!(!row.0.contains("super-secret"));
    assert!(!row.1.contains("abc123"));
    assert!(row.1.contains("[REDACTED]"));
    assert!(row.0.contains("'two words'"));

    let _ = fs::remove_file(db_path);
}

#[test]
fn records_render_mode_for_full_and_raw_fallback() {
    let db_path = temp_db_path("shell-record-render-modes");
    let result = sample_result();

    persist_to_path(&db_path, |conn| {
        insert_shell_run(
            conn,
            "run-full",
            &result,
            ShellPattern::GitDiff,
            "full output",
            ShellOutputMode::Full,
        )
    })
    .unwrap();
    persist_to_path(&db_path, |conn| {
        insert_shell_run(
            conn,
            "run-fallback",
            &result,
            ShellPattern::GitDiff,
            &result.stdout,
            ShellOutputMode::RawFallback,
        )
    })
    .unwrap();

    let conn = Connection::open(&db_path).unwrap();
    let rows = conn
        .prepare("SELECT run_id, render_mode, compressed_bytes FROM shell_runs ORDER BY id ASC")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    assert_eq!(rows[0].0, "run-full");
    assert_eq!(rows[0].1, "full");
    assert_eq!(rows[0].2, "full output".len() as i64);
    assert_eq!(rows[1].0, "run-fallback");
    assert_eq!(rows[1].1, "raw_fallback");
    assert_eq!(rows[1].2, result.stdout.len() as i64);

    let _ = fs::remove_file(db_path);
}

fn sample_result() -> ShellResult {
    ShellResult {
        invocation: ShellInvocation::new(vec!["git".into(), "diff".into()]),
        stdout: "diff output\n".into(),
        stderr: "warning\n".into(),
        exit_code: 0,
    }
}

fn temp_db_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}.db"))
}
