use std::collections::{BTreeSet, HashMap};
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::daemon::WatchManager;
use crate::daemon::watch_manager::WatchState;

#[path = "metrics/query.rs"]
mod query;
#[path = "metrics/render.rs"]
mod render;

pub use render::render_metrics_text;

use query::{
    EventRow, classify_shell_command, is_savings_eligible, load_event_rows, saved_bytes,
    saved_tokens,
};

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

    pub(crate) fn sql_time_modifier(self) -> Option<&'static str> {
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

#[derive(Default)]
struct UsageAgg {
    total_calls: u64,
    errors: u64,
    total_latency_ms: i64,
    eligible_calls: u64,
    compression_hits: u64,
    tokens_saved: i64,
    bytes_saved: i64,
    estimated_origin_tokens: i64,
    tool_aggs: HashMap<String, ToolAgg>,
    shell_aggs: HashMap<String, ShellCommandAgg>,
}

pub fn build_metrics_summary(
    request: &MetricsRequest,
    wm: &WatchManager,
    daemon_started_at: Instant,
) -> Result<MetricsSummary> {
    let rows = load_event_rows(request)?;
    let usage = aggregate_usage(&rows);
    let runtime = build_runtime_metrics(wm, daemon_started_at);

    Ok(MetricsSummary {
        window: request.window,
        project: request.project.clone(),
        summary: build_summary_metrics(&usage),
        by_tool: build_tool_metrics(usage.tool_aggs),
        top_shell_commands: build_shell_command_metrics(usage.shell_aggs, request.top_n),
        runtime,
    })
}

fn aggregate_usage(rows: &[EventRow]) -> UsageAgg {
    let mut usage = UsageAgg::default();

    for row in rows {
        usage.total_calls += 1;
        usage.total_latency_ms += row.duration_ms;
        if !row.result_ok {
            usage.errors += 1;
        }

        let tool_entry = usage.tool_aggs.entry(row.tool.clone()).or_default();
        tool_entry.calls += 1;
        tool_entry.total_latency_ms += row.duration_ms;
        if !row.result_ok {
            tool_entry.errors += 1;
        }

        if is_savings_eligible(row) {
            let row_saved_tokens = saved_tokens(row);
            let row_saved_bytes = saved_bytes(row);
            usage.eligible_calls += 1;
            if row_saved_tokens > 0 {
                usage.compression_hits += 1;
            }
            usage.tokens_saved += row_saved_tokens;
            usage.bytes_saved += row_saved_bytes;
            usage.estimated_origin_tokens += row.estimated_origin_tokens.max(0);
            tool_entry.tokens_saved += row_saved_tokens;
            tool_entry.bytes_saved += row_saved_bytes;
        }

        if row.tool == "so_shell" {
            let command = classify_shell_command(row.shell_params.as_deref());
            let shell_entry = usage.shell_aggs.entry(command).or_default();
            shell_entry.calls += 1;
            shell_entry.total_latency_ms += row.duration_ms;
            shell_entry.total_saved_tokens += saved_tokens(row);
            shell_entry.total_estimated_tokens += row.estimated_origin_tokens.max(0);
        }
    }

    usage
}

fn build_summary_metrics(usage: &UsageAgg) -> SummaryMetrics {
    SummaryMetrics {
        total_calls: usage.total_calls,
        success_rate: ratio(
            usage.total_calls.saturating_sub(usage.errors),
            usage.total_calls,
        ),
        avg_latency_ms: average_ms(usage.total_latency_ms, usage.total_calls),
        tokens_saved: usage.tokens_saved,
        bytes_saved: usage.bytes_saved,
        compression_hit_rate: ratio(usage.compression_hits, usage.eligible_calls),
        compression_hits: usage.compression_hits,
        compression_eligible_calls: usage.eligible_calls,
        savings_ratio: savings_ratio(usage.tokens_saved, usage.estimated_origin_tokens),
    }
}

fn build_tool_metrics(tool_aggs: HashMap<String, ToolAgg>) -> Vec<ToolMetrics> {
    let mut by_tool = Vec::with_capacity(tool_aggs.len());
    for (tool, agg) in tool_aggs {
        by_tool.push(ToolMetrics {
            tool,
            calls: agg.calls,
            errors: agg.errors,
            avg_latency_ms: average_ms(agg.total_latency_ms, agg.calls),
            tokens_saved: agg.tokens_saved,
            bytes_saved: agg.bytes_saved,
        });
    }

    by_tool.sort_by(|a, b| b.calls.cmp(&a.calls).then_with(|| a.tool.cmp(&b.tool)));
    by_tool
}

fn build_shell_command_metrics(
    shell_aggs: HashMap<String, ShellCommandAgg>,
    top_n: usize,
) -> Vec<ShellCommandMetrics> {
    let mut top_shell_commands = Vec::with_capacity(shell_aggs.len());
    for (command, agg) in shell_aggs {
        top_shell_commands.push(ShellCommandMetrics {
            command,
            calls: agg.calls,
            tokens_saved: agg.total_saved_tokens,
            saved_ratio: savings_ratio(agg.total_saved_tokens, agg.total_estimated_tokens),
            avg_latency_ms: average_ms(agg.total_latency_ms, agg.calls),
        });
    }

    top_shell_commands.sort_by(|a, b| {
        b.tokens_saved
            .cmp(&a.tokens_saved)
            .then_with(|| b.calls.cmp(&a.calls))
            .then_with(|| a.command.cmp(&b.command))
    });
    top_shell_commands.truncate(top_n.max(1));
    top_shell_commands
}

fn build_runtime_metrics(wm: &WatchManager, daemon_started_at: Instant) -> RuntimeMetrics {
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

    RuntimeMetrics {
        watched_projects: statuses.len(),
        running,
        indexing,
        failed,
        active_consumers: active_consumers.len(),
        daemon_uptime_seconds: daemon_started_at.elapsed().as_secs(),
    }
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

fn savings_ratio(saved_tokens: i64, estimated_origin_tokens: i64) -> f64 {
    if estimated_origin_tokens <= 0 {
        0.0
    } else {
        saved_tokens.max(0) as f64 / estimated_origin_tokens as f64
    }
}

#[cfg(test)]
#[path = "metrics/tests.rs"]
mod tests;
