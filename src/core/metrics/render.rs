use comfy_table::{
    Attribute, Cell, Color, ContentArrangement, Table, modifiers::UTF8_ROUND_CORNERS,
    presets::UTF8_FULL,
};

use super::MetricsSummary;

pub fn render_metrics_text(summary: &MetricsSummary) -> String {
    let mut out = Vec::new();
    out.push(render_heading(summary));
    out.push(String::new());

    render_summary(&mut out, summary);
    out.push(String::new());

    render_by_tool(&mut out, summary);
    out.push(String::new());

    render_top_shell_commands(&mut out, summary);
    out.push(String::new());

    render_runtime(&mut out, summary);
    out.join("\n")
}

fn render_heading(summary: &MetricsSummary) -> String {
    let mut heading = format!("so-context Metrics ({})", summary.window.label());
    if let Some(project) = summary.project.as_deref() {
        heading.push_str(" — ");
        heading.push_str(&truncate_middle(project, 72));
    }
    heading
}

fn render_summary(out: &mut Vec<String>, summary: &MetricsSummary) {
    out.push("Summary".to_string());
    let rows = [
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

    let label_width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
    for (label, value) in rows {
        out.push(format!(
            "{label:label_width$}  {value}",
            label_width = label_width
        ));
    }
}

fn render_by_tool(out: &mut Vec<String>, summary: &MetricsSummary) {
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
}

fn render_top_shell_commands(out: &mut Vec<String>, summary: &MetricsSummary) {
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
}

fn render_runtime(out: &mut Vec<String>, summary: &MetricsSummary) {
    out.push("Runtime".to_string());
    let rows = [
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

    let label_width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
    for (label, value) in rows {
        out.push(format!(
            "{label:label_width$}  {value}",
            label_width = label_width
        ));
    }
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
        value.abs().to_string()
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
pub(super) fn progress_bar_for_tests(ratio: f64, width: usize) -> String {
    progress_bar(ratio, width)
}
