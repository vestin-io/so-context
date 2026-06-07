use anyhow::Result;

use crate::core_events::open_db;

use super::MetricsRequest;

#[derive(Debug)]
pub(super) struct EventRow {
    pub tool: String,
    pub result_ok: bool,
    pub duration_ms: i64,
    pub estimated_origin_tokens: i64,
    pub actual_tokens: i64,
    pub estimated_origin_size: i64,
    pub actual_size: i64,
    pub read_mode: Option<String>,
    pub shell_params: Option<String>,
}

pub(super) fn load_event_rows(request: &MetricsRequest) -> Result<Vec<EventRow>> {
    let conn = open_db().map_err(anyhow::Error::msg)?;
    let query = build_event_query(request);
    let mut stmt = conn.prepare(&query)?;

    let mapped = if let Some(project) = request.project.as_deref() {
        stmt.query_map([project], map_event_row)?
    } else {
        stmt.query_map([], map_event_row)?
    };

    let mut rows = Vec::new();
    for row in mapped {
        rows.push(row?);
    }
    Ok(rows)
}

fn build_event_query(request: &MetricsRequest) -> String {
    let mut query = String::from(
        "SELECT tool, result_ok, COALESCE(duration_ms, 0), \
                COALESCE(estimated_origin_tokens, 0), COALESCE(actual_tokens, 0), \
                COALESCE(estimated_origin_size, 0), COALESCE(actual_size, 0), \
                CASE WHEN tool = 'so_read' THEN json_extract(params, '$.mode') ELSE NULL END, \
                CASE WHEN tool = 'so_shell' THEN params ELSE NULL END \
         FROM events",
    );

    let mut filters = Vec::new();
    if let Some(modifier) = request.window.sql_time_modifier() {
        filters.push(format!(
            "ts >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '{}')",
            modifier
        ));
    }
    if request.project.is_some() {
        filters.push("project = ?1".to_string());
    }
    if !filters.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&filters.join(" AND "));
    }

    query
}

fn map_event_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRow> {
    Ok(EventRow {
        tool: row.get(0)?,
        result_ok: row.get::<_, i64>(1)? != 0,
        duration_ms: row.get(2)?,
        estimated_origin_tokens: row.get(3)?,
        actual_tokens: row.get(4)?,
        estimated_origin_size: row.get(5)?,
        actual_size: row.get(6)?,
        read_mode: row.get(7)?,
        shell_params: row.get(8)?,
    })
}

pub(super) fn is_savings_eligible(row: &EventRow) -> bool {
    match row.tool.as_str() {
        "so_shell" | "so_search" => true,
        "so_read" => row.read_mode.as_deref() != Some("full"),
        _ => false,
    }
}

pub(super) fn saved_tokens(row: &EventRow) -> i64 {
    (row.estimated_origin_tokens - row.actual_tokens).max(0)
}

pub(super) fn saved_bytes(row: &EventRow) -> i64 {
    (row.estimated_origin_size - row.actual_size).max(0)
}

pub(super) fn classify_shell_command(params: Option<&str>) -> String {
    let Some(params) = params else {
        return "unknown".to_string();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(params) else {
        return "unknown".to_string();
    };
    let argv = value
        .get("argv")
        .and_then(|v| v.as_array())
        .map(|items| {
            let mut argv = Vec::new();
            for item in items {
                if let Some(arg) = item.as_str() {
                    argv.push(arg.to_string());
                }
            }
            argv
        })
        .unwrap_or_default();

    if argv.is_empty() {
        return value
            .get("family")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
    }

    classify_shell_argv(&argv, &value)
}

fn classify_shell_argv(argv: &[String], value: &serde_json::Value) -> String {
    let command = argv[0].as_str();
    match command {
        "rg" | "grep" => {
            if argv.iter().any(|arg| arg == "--files") {
                format!("{command} --files")
            } else if argv.iter().any(|arg| arg == "-n") {
                format!("{command} -n")
            } else {
                command.to_string()
            }
        }
        "git" => match argv.get(1).map(String::as_str) {
            Some("status") if argv.iter().any(|arg| arg == "--short") => {
                "git status --short".to_string()
            }
            Some(sub) => format!("git {sub}"),
            None => "git".to_string(),
        },
        "cargo" => match argv.get(1).map(String::as_str) {
            Some(sub) => format!("cargo {sub}"),
            None => "cargo".to_string(),
        },
        "sed" => {
            if argv.iter().any(|arg| arg == "-n") {
                "sed -n".to_string()
            } else {
                "sed".to_string()
            }
        }
        "find" | "cat" | "head" | "tail" => command.to_string(),
        "sh" | "bash" | "zsh" => value
            .get("family")
            .and_then(|v| v.as_str())
            .unwrap_or(command)
            .to_string(),
        _ => {
            let mut prefix = Vec::new();
            for item in argv.iter().take(2) {
                prefix.push(item.as_str());
            }
            prefix.join(" ")
        }
    }
}
