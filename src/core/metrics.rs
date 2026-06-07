use std::collections::{BTreeSet, HashMap};
use std::time::Instant;

use anyhow::Result;
use comfy_table::{
    Attribute, Cell, Color, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::core_events::open_db;
use crate::daemon::WatchManager;
use crate::daemon::watch_manager::WatchState;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricsWindow {
    Last24Hours,
    Last7Days,
    All,
}

impl MetricsWindow {
    pub fn label(self) -> &'static str {
        match self {
            Self::Last24Hours => "24h",
            Self::Last7Days => "7d",
            Self::All => "all",
        }
    }

    fn sql_time_modifier(self) -> Option<&'static str> {
        match self {
            Self::Last24Hours => Some("-1 day"),
            Self::Last7Days => Some("-7 day"),
            Self::All => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricsRequest {
    pub window: MetricsWindow,
    pub project: Option<String>,
    pub top_n: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSummary {
    pub window: MetricsWindow,
    pub project: Option<String>,
    pub summary: SummaryMetrics,
    pub by_tool: Vec<ToolMetrics>,
    pub top_shell_commands: Vec<ShellCommandMetrics>,
    pub runtime: RuntimeMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryMetrics {
    pub total_calls: u64,
    pub success_rate: f64,
    pub avg_latency_ms: f64,
    pub tokens_saved: i64,
    pub bytes_saved: i64,
    pub compression_hit_rate: f64,
    pub compression_hits: u64,
    pub compression_eligible_calls: u64,
    pub savings_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetrics {
    pub tool: String,
    pub calls: u64,
    pub errors: u64,
    pub avg_latency_ms: f64,
    pub tokens_saved: i64,
    pub bytes_saved: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCommandMetrics {
    pub command: String,
    pub calls: u64,
    pub tokens_saved: i64,
    pub saved_ratio: f64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMetrics {
    pub watched_projects: usize,
    pub running: usize,
    pub indexing: usize,
    pub failed: usize,
    pub active_consumers: usize,
    pub daemon_uptime_seconds: u64,
}

#[derive(Debug)]
struct EventRow {
    tool: String,
    result_ok: bool,
    duration_ms: i64,
    estimated_origin_tokens: i64,
    actual_tokens: i64,
    estimated_origin_size: i64,
    actual_size: i64,
    read_mode: Option<String>,
    shell_params: Option<String>,
}

#[derive(Default)]
struct ToolAgg {
    calls: u64,
    errors: u64,
    total_latency_ms: i64,
    tokens_saved: i64,
    bytes_saved: i64,
}

#[derive(Default)]
struct ShellCommandAgg {
    calls: u64,
    total_latency_ms: i64,
    total_saved_tokens: i64,
    total_estimated_tokens: i64,
}

pub fn build_metrics_summary(
    request: &MetricsRequest,
    wm: &WatchManager,
    daemon_started_at: Instant,
) -> Result<MetricsSummary> {
    let conn = open_db().map_err(anyhow::Error::msg)?;
    let rows = load_event_rows(&conn, request)?;

    let total_calls = rows.len() as u64;
    let errors = rows.iter().filter(|row| !row.result_ok).count() as u64;
    let total_latency_ms: i64 = rows.iter().map(|row| row.duration_ms).sum();

    let eligible_rows = rows
        .iter()
        .filter(|row| is_savings_eligible(row))
        .collect::<Vec<_>>();
    let eligible_calls = eligible_rows.len() as u64;
    let tokens_saved: i64 = eligible_rows.iter().map(|row| saved_tokens(row)).sum();
    let bytes_saved: i64 = eligible_rows.iter().map(|row| saved_bytes(row)).sum();
    let compression_hits = eligible_rows
        .iter()
        .filter(|row| saved_tokens(row) > 0)
        .count() as u64;

    let mut tool_aggs: HashMap<String, ToolAgg> = HashMap::new();
    let mut shell_aggs: HashMap<String, ShellCommandAgg> = HashMap::new();

    for row in &rows {
        let tool_entry = tool_aggs.entry(row.tool.clone()).or_default();
        tool_entry.calls += 1;
        tool_entry.total_latency_ms += row.duration_ms;
        if !row.result_ok {
            tool_entry.errors += 1;
        }
        if is_savings_eligible(row) {
            tool_entry.tokens_saved += saved_tokens(row);
            tool_entry.bytes_saved += saved_bytes(row);
        }

        if row.tool == "so_shell" {
            let command = classify_shell_command(row.shell_params.as_deref());
            let shell_entry = shell_aggs.entry(command).or_default();
            shell_entry.calls += 1;
            shell_entry.total_latency_ms += row.duration_ms;
            shell_entry.total_saved_tokens += saved_tokens(row);
            shell_entry.total_estimated_tokens += row.estimated_origin_tokens.max(0);
        }
    }

    let mut by_tool = tool_aggs
        .into_iter()
        .map(|(tool, agg)| ToolMetrics {
            tool,
            calls: agg.calls,
            errors: agg.errors,
            avg_latency_ms: average_ms(agg.total_latency_ms, agg.calls),
            tokens_saved: agg.tokens_saved,
            bytes_saved: agg.bytes_saved,
        })
        .collect::<Vec<_>>();
    by_tool.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.tool.cmp(&b.tool)));

    let mut top_shell_commands = shell_aggs
        .into_iter()
        .map(|(command, agg)| ShellCommandMetrics {
            command,
            calls: agg.calls,
            tokens_saved: agg.total_saved_tokens,
            saved_ratio: if agg.total_estimated_tokens > 0 {
                agg.total_saved_tokens.max(0) as f64 / agg.total_estimated_tokens as f64
            } else {
                0.0
            },
            avg_latency_ms: average_ms(agg.total_latency_ms, agg.calls),
        })
        .collect::<Vec<_>>();
    top_shell_commands.sort_by(|a, b| {
        b.tokens_saved
            .cmp(&a.tokens_saved)
            .then_with(|| b.calls.cmp(&a.calls))
            .then_with(|| a.command.cmp(&b.command))
    });
    top_shell_commands.truncate(request.top_n.max(1));

    let statuses = wm.status();
    let mut active_consumers = BTreeSet::new();
    let mut running = 0usize;
    let mut indexing = 0usize;
    let mut failed = 0usize;
    for status in &statuses {
        match &status.state {
            WatchState::Running => running += 1,
            WatchState::Indexing => indexing += 1,
            WatchState::Failed(_) => failed += 1,
        }
        for consumer in &status.consumers {
            active_consumers.insert(consumer.key());
        }
    }

    Ok(MetricsSummary {
        window: request.window,
        project: request.project.clone(),
        summary: SummaryMetrics {
            total_calls,
            success_rate: ratio(total_calls.saturating_sub(errors), total_calls),
            avg_latency_ms: average_ms(total_latency_ms, total_calls),
            tokens_saved,
            bytes_saved,
            compression_hit_rate: ratio(compression_hits, eligible_calls),
            compression_hits,
            compression_eligible_calls: eligible_calls,
            savings_ratio: savings_ratio(tokens_saved, &eligible_rows),
        },
        by_tool,
        top_shell_commands,
        runtime: RuntimeMetrics {
            watched_projects: statuses.len(),
            running,
            indexing,
            failed,
            active_consumers: active_consumers.len(),
            daemon_uptime_seconds: daemon_started_at.elapsed().as_secs(),
        },
    })
}

fn load_event_rows(conn: &Connection, request: &MetricsRequest) -> Result<Vec<EventRow>> {
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

fn is_savings_eligible(row: &EventRow) -> bool {
    match row.tool.as_str() {
        "so_shell" | "so_search" => true,
        "so_read" => row.read_mode.as_deref() != Some("full"),
        _ => false,
    }
}

fn saved_tokens(row: &EventRow) -> i64 {
    (row.estimated_origin_tokens - row.actual_tokens).max(0)
}

fn saved_bytes(row: &EventRow) -> i64 {
    (row.estimated_origin_size - row.actual_size).max(0)
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn average_ms(total_ms: i64, count: u64) -> f64 {
    if count == 0 {
        0.0
    } else {
        total_ms as f64 / count as f64
    }
}

fn savings_ratio(tokens_saved: i64, eligible_rows: &[&EventRow]) -> f64 {
    let estimated_total: i64 = eligible_rows
        .iter()
        .map(|row| row.estimated_origin_tokens.max(0))
        .sum();
    if estimated_total <= 0 {
        0.0
    } else {
        tokens_saved.max(0) as f64 / estimated_total as f64
    }
}

fn classify_shell_command(params: Option<&str>) -> String {
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
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if argv.is_empty() {
        return value
            .get("family")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
    }

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
        _ => argv
            .iter()
            .take(2)
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" "),
    }
}

pub fn render_metrics_text(summary: &MetricsSummary) -> String {
    let mut out = Vec::new();
    out.push(format!(
        "so-context Metrics ({}){}",
        summary.window.label(),
        summary
            .project
            .as_deref()
            .map(|project| format!(" — {}", truncate_middle(project, 72)))
            .unwrap_or_default()
    ));
    out.push(String::new());

    let summary_rows = [
        ("Total calls", summary.summary.total_calls.to_string()),
        (
            "Success rate",
            format!("{:.1}%", summary.summary.success_rate * 100.0),
        ),
        (
            "Avg latency",
            format!("{:.0}ms", summary.summary.avg_latency_ms),
        ),
        (
            "Token saved",
            format_human_number(summary.summary.tokens_saved),
        ),
        ("Bytes saved", format_bytes(summary.summary.bytes_saved)),
        (
            "Compression hits",
            format!(
                "{}/{} ({:.1}%)",
                summary.summary.compression_hits,
                summary.summary.compression_eligible_calls,
                summary.summary.compression_hit_rate * 100.0
            ),
        ),
        (
            "Savings ratio",
            format!(
                "{} {:.1}%",
                progress_bar(summary.summary.savings_ratio, 24),
                summary.summary.savings_ratio * 100.0
            ),
        ),
    ];
    let label_width = summary_rows
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(0);
    out.push("Summary".to_string());
    for (label, value) in summary_rows {
        out.push(format!(
            "{label:label_width$}  {value}",
            label_width = label_width
        ));
    }
    out.push(String::new());

    out.push("By Tool".to_string());
    let mut tool_table = base_table();
    tool_table.set_header(vec![
        styled_header("Tool"),
        styled_header("Calls"),
        styled_header("Errors"),
        styled_header("Tokens saved"),
        styled_header("Bytes saved"),
    ]);
    for tool in &summary.by_tool {
        tool_table.add_row(vec![
            Cell::new(tool.tool.as_str()),
            Cell::new(tool.calls),
            Cell::new(tool.errors),
            Cell::new(format_human_number(tool.tokens_saved)),
            Cell::new(format_bytes(tool.bytes_saved)),
        ]);
    }
    out.push(tool_table.to_string());
    out.push(String::new());

    out.push(format!(
        "Top Shell Commands (top {} by token savings)",
        summary.top_shell_commands.len()
    ));
    let mut shell_table = base_table();
    shell_table.set_header(vec![
        styled_header("#"),
        styled_header("Command"),
        styled_header("Calls"),
        styled_header("Tokens saved"),
        styled_header("Saved%"),
        styled_header("Avg ms"),
    ]);
    for (index, command) in summary.top_shell_commands.iter().enumerate() {
        shell_table.add_row(vec![
            Cell::new(index + 1),
            Cell::new(truncate_middle(&command.command, 42)),
            Cell::new(command.calls),
            Cell::new(format_human_number(command.tokens_saved)),
            Cell::new(format!("{:.1}%", command.saved_ratio * 100.0)),
            Cell::new(format!("{:.0}", command.avg_latency_ms)),
        ]);
    }
    out.push(shell_table.to_string());
    out.push(String::new());

    out.push("Runtime".to_string());
    let runtime_rows = [
        (
            "Watched projects",
            summary.runtime.watched_projects.to_string(),
        ),
        ("Running", summary.runtime.running.to_string()),
        ("Indexing", summary.runtime.indexing.to_string()),
        ("Failed", summary.runtime.failed.to_string()),
        (
            "Active consumers",
            summary.runtime.active_consumers.to_string(),
        ),
        (
            "Daemon uptime",
            format_duration(summary.runtime.daemon_uptime_seconds),
        ),
    ];
    let runtime_label_width = runtime_rows
        .iter()
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(0);
    for (label, value) in runtime_rows {
        out.push(format!(
            "{label:runtime_label_width$}  {value}",
            runtime_label_width = runtime_label_width
        ));
    }

    out.join("\n")
}

fn base_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

fn styled_header(text: &str) -> Cell {
    Cell::new(text)
        .fg(Color::Green)
        .add_attribute(Attribute::Bold)
}

fn format_human_number(value: i64) -> String {
    let negative = value < 0;
    let abs = value.abs() as f64;
    let rendered = if abs >= 1_000_000_000.0 {
        format!("{:.1}B", abs / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
        format!("{:.1}M", abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}K", abs / 1_000.0)
    } else {
        format!("{}", value.abs())
    };
    if negative {
        format!("-{rendered}")
    } else {
        rendered
    }
}

fn format_bytes(bytes: i64) -> String {
    let negative = bytes < 0;
    let abs = bytes.abs() as f64;
    let rendered = if abs >= 1_073_741_824.0 {
        format!("{:.1} GB", abs / 1_073_741_824.0)
    } else if abs >= 1_048_576.0 {
        format!("{:.1} MB", abs / 1_048_576.0)
    } else if abs >= 1024.0 {
        format!("{:.1} KB", abs / 1024.0)
    } else {
        format!("{} B", bytes.abs())
    };
    if negative {
        format!("-{rendered}")
    } else {
        rendered
    }
}

fn progress_bar(ratio: f64, width: usize) -> String {
    let clamped = ratio.clamp(0.0, 1.0);
    let filled = (clamped * width as f64).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

fn truncate_middle(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    if max_len <= 3 {
        return "...".to_string();
    }
    let keep = (max_len - 3) / 2;
    let start = &value[..keep];
    let end = &value[value.len() - (max_len - 3 - keep)..];
    format!("{start}...{end}")
}

#[cfg(test)]
mod tests {
    use super::{
        EventRow, MetricsWindow, classify_shell_command, is_savings_eligible, progress_bar,
    };

    #[test]
    fn classifies_rg_commands() {
        let params = r#"{"argv":["rg","--files","."],"family":"search"}"#;
        assert_eq!(classify_shell_command(Some(params)), "rg --files");

        let params = r#"{"argv":["rg","-n","TODO","."],"family":"search"}"#;
        assert_eq!(classify_shell_command(Some(params)), "rg -n");
    }

    #[test]
    fn classifies_git_status_short() {
        let params = r#"{"argv":["git","status","--short"],"family":"git_status"}"#;
        assert_eq!(classify_shell_command(Some(params)), "git status --short");
    }

    #[test]
    fn progress_bar_clamps() {
        assert_eq!(progress_bar(1.5, 5), "[█████]");
        assert_eq!(progress_bar(-1.0, 5), "[░░░░░]");
    }

    #[test]
    fn window_labels_are_stable() {
        assert_eq!(MetricsWindow::Last24Hours.label(), "24h");
        assert_eq!(MetricsWindow::Last7Days.label(), "7d");
        assert_eq!(MetricsWindow::All.label(), "all");
    }

    #[test]
    fn read_full_is_not_savings_eligible() {
        let row = EventRow {
            tool: "so_read".to_string(),
            result_ok: true,
            duration_ms: 0,
            estimated_origin_tokens: 0,
            actual_tokens: 0,
            estimated_origin_size: 0,
            actual_size: 0,
            read_mode: Some("full".to_string()),
            shell_params: None,
        };
        assert!(!is_savings_eligible(&row));
    }

    #[test]
    fn read_outline_is_savings_eligible() {
        let row = EventRow {
            tool: "so_read".to_string(),
            result_ok: true,
            duration_ms: 0,
            estimated_origin_tokens: 0,
            actual_tokens: 0,
            estimated_origin_size: 0,
            actual_size: 0,
            read_mode: Some("outline".to_string()),
            shell_params: None,
        };
        assert!(is_savings_eligible(&row));
    }
}
