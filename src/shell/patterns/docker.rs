use serde_json::Value;

use super::super::types::{CompressionSummary, ShellPattern, ShellResult};
use super::helpers::{non_empty_lines, preferred_output, preview, sample_lines};

pub fn summarize_ps(result: &ShellResult) -> CompressionSummary {
    let rows = data_rows(&result.stdout);
    let sample = column_values(&rows, 0);

    CompressionSummary {
        pattern: ShellPattern::DockerPs,
        summary: format!("containers={}", rows.len()),
        details: sample,
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_images(result: &ShellResult) -> CompressionSummary {
    let rows = data_rows(&result.stdout);
    let total = rows.len();
    let sample: Vec<String> = rows
        .into_iter()
        .take(5)
        .map(|row| row.split_whitespace().take(2).collect::<Vec<_>>().join(":"))
        .collect();

    CompressionSummary {
        pattern: ShellPattern::DockerImages,
        summary: format!("images={total}"),
        details: sample,
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_compose(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));

    CompressionSummary {
        pattern: ShellPattern::DockerCompose,
        summary: format!("lines={}", lines.len()),
        details: sample_lines(lines, 6),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_logs(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let error_lines = lines
        .iter()
        .filter(|line| line.to_ascii_lowercase().contains("error"))
        .count();

    CompressionSummary {
        pattern: ShellPattern::DockerLogs,
        summary: format!("lines={}; error_lines={error_lines}", lines.len()),
        details: sample_lines(lines, 8),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_build(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let step_count = lines
        .iter()
        .filter(|line| line.starts_with("Step ") || line.starts_with('#'))
        .count();
    let success = lines
        .iter()
        .rev()
        .find(|line| {
            line.contains("Successfully tagged")
                || line.contains("writing image")
                || line.contains("naming to")
        })
        .cloned()
        .unwrap_or_else(|| "no final image line".to_string());

    CompressionSummary {
        pattern: ShellPattern::DockerBuild,
        summary: format!("steps={step_count}; final={success}"),
        details: sample_lines(lines, 8),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_inspect(result: &ShellResult) -> CompressionSummary {
    let output = preferred_output(result);
    let parsed = serde_json::from_str::<Value>(&output).ok();

    let (summary, details) = match parsed {
        Some(Value::Array(items)) => {
            let first_keys: Vec<String> = items
                .first()
                .and_then(|value| value.as_object())
                .map(|obj| obj.keys().take(6).cloned().collect())
                .unwrap_or_default();
            (
                format!("objects={}; first_keys={}", items.len(), first_keys.len()),
                first_keys,
            )
        }
        Some(Value::Object(map)) => {
            let keys: Vec<String> = map.keys().take(8).cloned().collect();
            (format!("keys={}", keys.len()), keys)
        }
        _ => {
            let lines = non_empty_lines(&output);
            (format!("lines={}", lines.len()), sample_lines(lines, 6))
        }
    };

    CompressionSummary {
        pattern: ShellPattern::DockerInspect,
        summary,
        details,
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

pub fn summarize_pull(result: &ShellResult) -> CompressionSummary {
    let lines = non_empty_lines(&preferred_output(result));
    let layers = lines
        .iter()
        .filter(|line| line.contains(": Pulling") || line.contains(": Download"))
        .count();

    CompressionSummary {
        pattern: ShellPattern::DockerPull,
        summary: format!("lines={}; layers={layers}", lines.len()),
        details: sample_lines(lines, 8),
        stderr_preview: preview(&result.stderr),
        exit_code: result.exit_code,
        command_line: result.invocation.command_line(),
    }
}

fn data_rows(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect()
}

fn column_values(rows: &[String], index: usize) -> Vec<String> {
    rows.iter()
        .take(5)
        .filter_map(|row| row.split_whitespace().nth(index).map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::types::ShellInvocation;

    #[test]
    fn summarizes_ps_rows() {
        let result = ShellResult {
            invocation: ShellInvocation::new(vec!["docker".into(), "ps".into()]),
            stdout: "NAMES IMAGE STATUS\nweb nginx Up\ndb postgres Up\n".into(),
            stderr: String::new(),
            exit_code: 0,
        };

        let summary = summarize_ps(&result);
        assert!(summary.summary.contains("containers=2"));
    }

    #[test]
    fn summarizes_inspect_json() {
        let result = ShellResult {
            invocation: ShellInvocation::new(vec!["docker".into(), "inspect".into(), "web".into()]),
            stdout: "[{\"Id\":\"123\",\"Name\":\"/web\",\"Config\":{}}]".into(),
            stderr: String::new(),
            exit_code: 0,
        };

        let summary = summarize_inspect(&result);
        assert!(summary.summary.contains("objects=1"));
    }
}
