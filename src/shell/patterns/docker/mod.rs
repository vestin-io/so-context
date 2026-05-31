mod parse;

use serde_json::Value;

use self::parse::{
    build_detail_lines, build_stage_count, compose_build_services, data_rows, docker_log_source,
    docker_pull_source, docker_pull_summary, looks_like_compose_table, render_compose_ps_row,
    render_images_row, render_inspect_map, render_ps_row, status_column, total_image_size,
};
use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::argv::{first_positional, positional_args};
use super::text::{non_empty_lines, preferred_output, preview, sample_lines};

fn summarize_ps(result: &ShellResult) -> CompressionSummary {
    let rows = data_rows(&result.stdout);
    let details: Vec<String> = rows.iter().take(8).map(|row| render_ps_row(row)).collect();
    let running = rows
        .iter()
        .filter(|row| {
            status_column(row).is_some_and(|status| status == "Up" || status.starts_with("Up "))
        })
        .count();

    CompressionSummary::new(
        ShellPattern::DockerPs,
        format!("{} containers; {running} running", rows.len()),
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_images(result: &ShellResult) -> CompressionSummary {
    let rows = data_rows(&result.stdout);
    let total = rows.len();
    let total_size = total_image_size(&rows);
    let sample: Vec<String> = rows
        .into_iter()
        .take(8)
        .map(|row| render_images_row(&row))
        .collect();

    CompressionSummary::new(
        ShellPattern::DockerImages,
        format!("{total} images ({total_size})"),
        sample,
        preview(&result.stderr),
        result,
    )
}

fn summarize_compose(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let is_compose_table = looks_like_compose_table(&lines);
    let total_services = lines.len().saturating_sub(1);
    let details = if is_compose_table {
        lines
            .iter()
            .skip(1)
            .take(20)
            .map(|row| render_compose_ps_row(row))
            .collect()
    } else {
        sample_lines(lines.iter().cloned(), 6)
    };

    CompressionSummary::new(
        ShellPattern::DockerCompose,
        if is_compose_table {
            format!("{total_services} services")
        } else {
            format!("compose output: {} lines", lines.len())
        },
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_logs(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let source =
        docker_log_source(result.invocation.args()).unwrap_or_else(|| "docker".to_string());
    let error_lines: Vec<String> = lines
        .iter()
        .filter(|line| line.to_ascii_lowercase().contains("error"))
        .cloned()
        .collect();
    let warning_count = lines
        .iter()
        .filter(|line| line.to_ascii_lowercase().contains("warn"))
        .count();
    let error_count = error_lines.len();
    let details = if error_lines.is_empty() {
        sample_lines(lines.iter().cloned(), 8)
    } else {
        sample_lines(error_lines, 8)
    };
    let summary =
        if !details.is_empty() && !lines.is_empty() && (error_count > 0 || warning_count > 0) {
            format!(
                "logs for {source}: {} errors, {} warnings",
                error_count, warning_count
            )
        } else {
            format!("logs for {source}: {} lines", lines.len())
        };

    CompressionSummary::new(
        ShellPattern::DockerLogs,
        summary,
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_build(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let step_lines = build_detail_lines(&lines);
    let step_count = build_stage_count(&lines);
    let build_status = lines
        .iter()
        .rev()
        .find(|line| {
            line.contains("Successfully tagged")
                || line.contains("writing image")
                || (line.contains("naming to ") && !line.contains("sha256:"))
        })
        .cloned()
        .or_else(|| {
            lines
                .iter()
                .find(|line| line.contains("Building") && line.contains("FINISHED"))
                .map(|line| line.trim().to_string())
        })
        .unwrap_or_else(|| "build completed".to_string());
    let mut details = sample_lines(step_lines, 6);
    if let Some(service_line) = compose_build_services(&lines) {
        details.push(service_line);
    }
    let summary = if result.exit_code == 0 {
        "Build successful".to_string()
    } else {
        format!("{build_status}; stages={step_count}")
    };

    CompressionSummary::new(
        ShellPattern::DockerBuild,
        summary,
        details,
        if result.stdout.trim().is_empty() {
            Vec::new()
        } else {
            preview(&result.stderr)
        },
        result,
    )
}

fn summarize_inspect(result: &ShellResult) -> CompressionSummary {
    let output = preferred_output(result);
    let parsed = serde_json::from_str::<Value>(&output).ok();

    let (summary, details) = match parsed {
        Some(Value::Array(items)) => {
            let details = items
                .first()
                .and_then(|value| value.as_object().map(render_inspect_map))
                .unwrap_or_default();
            let summary = if details.is_empty() {
                format!("{} inspect objects", items.len())
            } else {
                format!("{} inspect objects; {}", items.len(), details[0])
            };
            (summary, details.into_iter().skip(1).collect())
        }
        Some(Value::Object(map)) => {
            let details = render_inspect_map(&map);
            let summary = if details.is_empty() {
                format!("inspect keys={}", map.len())
            } else {
                details[0].clone()
            };
            (summary, details.into_iter().skip(1).collect())
        }
        _ => {
            let lines = non_empty_lines(&output);
            (format!("lines={}", lines.len()), sample_lines(lines, 6))
        }
    };

    CompressionSummary::new(
        ShellPattern::DockerInspect,
        summary,
        details,
        preview(&result.stderr),
        result,
    )
}

fn summarize_pull(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let source =
        docker_pull_source(result.invocation.args()).unwrap_or_else(|| "image".to_string());
    let details: Vec<String> = lines
        .iter()
        .filter(|line| {
            line.contains(": Pulling")
                || line.contains(": Download")
                || line.contains(": Extracting")
                || line.contains("Downloaded newer image")
                || line.contains("Image is up to date")
        })
        .take(8)
        .cloned()
        .collect();

    CompressionSummary::new(
        ShellPattern::DockerPull,
        docker_pull_summary(&source, &lines, details.len()),
        details,
        preview(&result.stderr),
        result,
    )
}

pub(super) fn classify(program: &str, args: &[String]) -> Option<ShellPattern> {
    match program {
        "docker" => classify_docker(args),
        "docker-compose" => classify_compose(args),
        _ => None,
    }
}

pub(super) fn summarize_pattern(
    result: &ShellResult,
    pattern: ShellPattern,
) -> Option<CompressionSummary> {
    Some(match pattern {
        ShellPattern::DockerPs => summarize_ps(result),
        ShellPattern::DockerImages => summarize_images(result),
        ShellPattern::DockerCompose => summarize_compose(result),
        ShellPattern::DockerLogs => summarize_logs(result),
        ShellPattern::DockerBuild => summarize_build(result),
        ShellPattern::DockerInspect => summarize_inspect(result),
        ShellPattern::DockerPull => summarize_pull(result),
        _ => return None,
    })
}

fn classify_docker(args: &[String]) -> Option<ShellPattern> {
    let primary = first_positional(args, &["-H", "--host", "--context", "--config"])?;
    if primary == "compose" {
        return classify_compose(args);
    }

    match primary {
        "ps" => Some(ShellPattern::DockerPs),
        "images" => Some(ShellPattern::DockerImages),
        "logs" => Some(ShellPattern::DockerLogs),
        "build" => Some(ShellPattern::DockerBuild),
        "inspect" => Some(ShellPattern::DockerInspect),
        "pull" => Some(ShellPattern::DockerPull),
        _ => None,
    }
}

fn classify_compose(args: &[String]) -> Option<ShellPattern> {
    let mut positionals = positional_args(
        args,
        &[
            "-H",
            "--host",
            "--context",
            "--config",
            "-f",
            "--file",
            "-p",
            "--project-name",
            "--profile",
            "--env-file",
            "--project-directory",
        ],
    )
    .into_iter();

    let first = positionals.next()?;
    let subcommand = if first == "compose" {
        positionals.next()
    } else {
        Some(first)
    };

    match subcommand {
        Some("logs") => Some(ShellPattern::DockerLogs),
        Some("build") => Some(ShellPattern::DockerBuild),
        Some("pull") => Some(ShellPattern::DockerPull),
        Some("ps") | Some("config") | Some("up") | Some("down") | Some("images") => {
            Some(ShellPattern::DockerCompose)
        }
        Some(_) | None => Some(ShellPattern::DockerCompose),
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
