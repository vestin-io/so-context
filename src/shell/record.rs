use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use super::types::{ShellInvocation, ShellOutputMode, ShellPattern, ShellResult};
#[path = "record_redaction.rs"]
mod redaction;
#[path = "record_schema.rs"]
mod schema;

use redaction::redact_argv;
use schema::open_db;
#[cfg(test)]
pub(super) use schema::{persist_to_path, table_columns};

pub(super) fn persist(
    run_id: &str,
    result: &ShellResult,
    pattern: ShellPattern,
    rendered_output: &str,
    output_mode: ShellOutputMode,
) -> Result<()> {
    let db = open_db()?;
    let conn = db
        .lock()
        .map_err(|e| anyhow::anyhow!("shell metrics db lock poisoned: {e}"))?;
    insert_shell_run(&conn, run_id, result, pattern, rendered_output, output_mode)
}
fn insert_shell_run(
    conn: &Connection,
    run_id: &str,
    result: &ShellResult,
    pattern: ShellPattern,
    rendered_output: &str,
    output_mode: ShellOutputMode,
) -> Result<()> {
    let redacted_argv = redact_argv(&result.invocation.argv);
    let argv_json = serde_json::to_string(&redacted_argv).context("failed to encode argv json")?;
    let redacted_command_line = ShellInvocation::render_argv(&redacted_argv);

    conn.execute(
        "INSERT INTO shell_runs(
            run_id,
            family,
            command_line,
            argv_json,
            exit_code,
            raw_stdout_bytes,
            raw_stderr_bytes,
            compressed_bytes,
            render_mode
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            run_id,
            pattern.label(),
            redacted_command_line,
            argv_json,
            result.exit_code,
            result.stdout.len() as i64,
            result.stderr.len() as i64,
            rendered_output.len() as i64,
            output_mode.label(),
        ],
    )
    .context("failed to insert shell metrics row")?;

    Ok(())
}
#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
