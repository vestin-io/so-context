use serde_json::Value;

use super::super::text::compact_whitespace;

pub(super) fn data_rows(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect()
}

pub(super) fn render_ps_row(row: &str) -> String {
    let columns = split_columns(row);
    match columns.as_slice() {
        [] => String::new(),
        [single] => single.clone(),
        [name, image, status] => format!("{name} — {image} ({status})"),
        [container_id, image, rest @ ..] => {
            let name = rest.last().cloned().unwrap_or_else(|| container_id.clone());
            let status = status_column(row).unwrap_or_else(|| compact_whitespace(row));
            format!("{name} — {image} ({status})")
        }
    }
}

pub(super) fn render_images_row(row: &str) -> String {
    let columns = split_columns(row);
    match columns.as_slice() {
        [] => String::new(),
        [single] => single.clone(),
        [repository, tag] => format!("{repository}:{tag}"),
        [repository, tag, image_id, _created, size, ..] => {
            format!("{repository}:{tag} — {image_id} ({size})")
        }
        [repository, tag, image_id, ..] => format!("{repository}:{tag} — {image_id}"),
    }
}

pub(super) fn render_compose_ps_row(row: &str) -> String {
    let columns = split_columns(row);
    match columns.as_slice() {
        [name, image, status, ports, ..] => {
            let short_image = image.rsplit('/').next().unwrap_or(image);
            let port_label = compact_ports(ports);
            if port_label == "-" {
                format!("{name} ({short_image}) {status}")
            } else {
                format!("{name} ({short_image}) {status} [{port_label}]")
            }
        }
        _ => render_ps_row(row),
    }
}

pub(super) fn status_column(row: &str) -> Option<String> {
    split_columns(row)
        .into_iter()
        .find(|part| part == "Up" || part.starts_with("Up ") || part.starts_with("Exited "))
}

pub(super) fn looks_like_compose_table(lines: &[String]) -> bool {
    lines.first().is_some_and(|line| {
        line.contains("NAME") && line.contains("IMAGE") && line.contains("STATUS")
    })
}

pub(super) fn render_inspect_map(map: &serde_json::Map<String, Value>) -> Vec<String> {
    let mut details = Vec::new();
    if let Some(name) = map.get("Name").and_then(Value::as_str) {
        details.push(format!("name={}", name.trim_start_matches('/')));
    }
    if let Some(id) = map.get("Id").and_then(Value::as_str) {
        details.push(format!("id={}", &id[..id.len().min(12)]));
    }
    if let Some(image) = map.get("Image").and_then(Value::as_str) {
        details.push(format!("image={}", &image[..image.len().min(20)]));
    }
    if let Some(state) = map.get("State").and_then(Value::as_object)
        && let Some(status) = state.get("Status").and_then(Value::as_str)
    {
        details.push(format!("status={status}"));
    }
    if details.is_empty() {
        details = map.keys().take(6).cloned().collect();
    }
    details
}

pub(super) fn total_image_size(rows: &[String]) -> String {
    let total_mb = rows
        .iter()
        .map(|row| split_columns(row))
        .filter_map(|columns| columns.last().cloned())
        .map(|size| parse_size_mb(&size))
        .sum::<f64>();

    if total_mb >= 1024.0 {
        format!("{:.1}GB", total_mb / 1024.0)
    } else if total_mb > 0.0 {
        if total_mb < 1.0 {
            format!("{:.1}kB", total_mb * 1024.0)
        } else {
            format!("{:.0}MB", total_mb)
        }
    } else {
        "?".to_string()
    }
}

pub(super) fn docker_log_source(args: &[String]) -> Option<String> {
    args.iter()
        .rev()
        .find(|arg| !arg.starts_with('-') && *arg != "logs")
        .cloned()
}

pub(super) fn build_detail_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| {
            line.starts_with("Step ")
                || (line.starts_with('#')
                    && (line.contains('[') || line.contains("exporting to image")))
        })
        .cloned()
        .collect()
}

pub(super) fn build_stage_count(lines: &[String]) -> usize {
    let mut ids = Vec::new();
    for line in lines.iter().filter(|line| line.starts_with('#')) {
        if let Some((id, _)) = line.split_once(' ')
            && !ids.iter().any(|existing| existing == id)
        {
            ids.push(id.to_string());
        }
    }
    ids.len()
}

pub(super) fn compose_build_services(lines: &[String]) -> Option<String> {
    let mut services = Vec::new();
    for line in lines {
        if let Some(start) = line.find('[')
            && let Some(end) = line[start + 1..].find(']')
        {
            let bracket = &line[start + 1..start + 1 + end];
            let svc = bracket.split_whitespace().next().unwrap_or("");
            if !svc.is_empty() && svc != "+" && !services.iter().any(|existing| existing == svc) {
                services.push(svc.to_string());
            }
        }
    }

    if services.is_empty() {
        None
    } else {
        Some(format!("services: {}", services.join(", ")))
    }
}

pub(super) fn docker_pull_source(args: &[String]) -> Option<String> {
    args.iter()
        .rev()
        .find(|arg| !arg.starts_with('-') && *arg != "pull")
        .cloned()
}

pub(super) fn docker_pull_summary(source: &str, lines: &[String], layers: usize) -> String {
    if let Some(line) = lines
        .iter()
        .find(|line| line.contains("Downloaded newer image"))
    {
        return format!("{source} ok | {}", line.trim());
    }
    if let Some(line) = lines
        .iter()
        .find(|line| line.contains("Image is up to date"))
    {
        return format!("{source} ok | {}", line.trim());
    }
    format!("{source} pull | {layers} layer updates")
}

fn split_columns(row: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut spaces = 0usize;

    for ch in row.chars() {
        if ch == ' ' {
            spaces += 1;
            if spaces >= 2 {
                if !current.trim().is_empty() {
                    out.push(current.trim().to_string());
                    current.clear();
                }
                continue;
            }
        } else {
            if spaces == 1 {
                current.push(' ');
            }
            spaces = 0;
            current.push(ch);
        }
    }

    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }

    if out.len() <= 1 {
        return row.split_whitespace().map(str::to_string).collect();
    }

    out
}

fn parse_size_mb(size: &str) -> f64 {
    let normalized = size.trim().to_ascii_lowercase();
    if let Some(value) = normalized.strip_suffix("gb") {
        value
            .trim()
            .parse::<f64>()
            .map(|n| n * 1024.0)
            .unwrap_or(0.0)
    } else if let Some(value) = normalized.strip_suffix("mb") {
        value.trim().parse::<f64>().unwrap_or(0.0)
    } else if let Some(value) = normalized.strip_suffix("kb") {
        value
            .trim()
            .parse::<f64>()
            .map(|n| n / 1024.0)
            .unwrap_or(0.0)
    } else if let Some(value) = normalized.strip_suffix('b') {
        value
            .trim()
            .parse::<f64>()
            .map(|n| n / (1024.0 * 1024.0))
            .unwrap_or(0.0)
    } else {
        0.0
    }
}

fn compact_ports(ports: &str) -> String {
    let ports = ports.trim();
    if ports.is_empty() || ports == "-" {
        return "-".to_string();
    }

    let collected: Vec<String> = ports
        .split(',')
        .filter_map(|entry| {
            entry
                .split("->")
                .next()
                .and_then(|part| part.split(':').next_back())
                .map(|part| part.trim().to_string())
        })
        .collect();

    if collected.is_empty() {
        "-".to_string()
    } else if collected.len() <= 3 {
        collected.join(", ")
    } else {
        format!("{}, … +{}", collected[..2].join(", "), collected.len() - 2)
    }
}
