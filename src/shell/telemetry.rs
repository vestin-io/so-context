use std::path::{Path, PathBuf};

use crate::core_events::EventRecord;
use crate::core_tokens::count_tokens;

use super::redaction::redact_argv;
use super::types::RunOutput;

#[derive(Debug, Clone)]
pub struct ShellEventContext {
    pub tool: String,
    pub client: Option<String>,
    pub client_version: Option<String>,
    pub client_source: String,
    pub session_id: String,
    pub session_source: String,
    pub project: Option<String>,
    pub agent: Option<String>,
}

impl ShellEventContext {
    pub fn mcp_shell(
        client: Option<String>,
        client_version: Option<String>,
        session_id: String,
        session_source: &str,
        project: PathBuf,
    ) -> Self {
        Self {
            tool: "so_shell".to_string(),
            client,
            client_version,
            client_source: "client_info".to_string(),
            session_id,
            session_source: session_source.to_string(),
            project: Some(project.to_string_lossy().to_string()),
            agent: None,
        }
    }
}

pub fn build_shell_event(
    context: ShellEventContext,
    output: &RunOutput,
    displayed_output: &str,
    duration_ms: i64,
) -> EventRecord {
    let mut event = EventRecord::new(&context.session_id, &context.tool);
    event.client = context.client;
    event.client_version = context.client_version;
    event.client_source = context.client_source;
    event.agent = context.agent;
    event.session_source = context.session_source;
    event.project = context.project;
    event.params = Some(shell_params(output, displayed_output).to_string());
    event.result_ok = output.exit_code == 0;
    event.duration_ms = Some(duration_ms);
    event.estimated_origin_tokens = Some(count_tokens(&output.full_output));
    event.actual_tokens = Some(count_tokens(displayed_output));
    event.estimated_origin_size = Some(output.full_output.len() as i64);
    event.actual_size = Some(displayed_output.len() as i64);
    event
}

pub fn build_shell_error_event(
    context: ShellEventContext,
    argv: &[String],
    cwd: Option<&Path>,
    requested_full: bool,
    duration_ms: i64,
    error: &str,
) -> EventRecord {
    let mut event = EventRecord::new(&context.session_id, &context.tool);
    event.client = context.client;
    event.client_version = context.client_version;
    event.client_source = context.client_source;
    event.agent = context.agent;
    event.session_source = context.session_source;
    event.project = context.project;
    event.params = Some(
        serde_json::json!({
            "argv": redact_argv(argv),
            "command_line": render_redacted_command_line(argv),
            "cwd": cwd.map(|path| path.to_string_lossy().to_string()),
            "full": requested_full,
            "error": error,
        })
        .to_string(),
    );
    event.result_ok = false;
    event.duration_ms = Some(duration_ms);
    event
}

fn shell_params(output: &RunOutput, displayed_output: &str) -> serde_json::Value {
    serde_json::json!({
        "run_id": output.run_id,
        "argv": redact_argv(&output.invocation.argv),
        "command_line": render_redacted_command_line(&output.invocation.argv),
        "cwd": output
            .invocation
            .cwd()
            .map(|path| path.to_string_lossy().to_string()),
        "full": output.requested_full,
        "family": output.pattern.label(),
        "exit_code": output.exit_code,
        "render_mode": output.output_mode.label(),
        "raw_stdout_bytes": output.stdout_bytes,
        "raw_stderr_bytes": output.stderr_bytes,
        "stdout_truncated": output.stdout_truncated,
        "stderr_truncated": output.stderr_truncated,
        "capture_stdout_limit_bytes": output.capture_stdout_limit_bytes,
        "capture_stderr_limit_bytes": output.capture_stderr_limit_bytes,
        "raw_output_complete": output.raw_output_complete,
        "capture_strategy": "bounded",
        "full_output_bytes": output.full_output.len(),
        "displayed_output_bytes": displayed_output.len(),
    })
}

fn render_redacted_command_line(argv: &[String]) -> String {
    let redacted_argv = redact_argv(argv);
    super::types::ShellInvocation::render_argv(&redacted_argv)
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
