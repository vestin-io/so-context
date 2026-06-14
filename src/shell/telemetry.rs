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
    pub fn cli_shell(run_id: String, project: Option<PathBuf>) -> Self {
        let client = std::env::var("SO_CONTEXT_CLIENT")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let client_version = std::env::var("SO_CONTEXT_CLIENT_VERSION")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let session_id = std::env::var("SO_CONTEXT_SESSION_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(run_id);
        let session_source = std::env::var("SO_CONTEXT_SESSION_SOURCE")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "generated".to_string());
        let agent = std::env::var("SO_CONTEXT_AGENT")
            .ok()
            .filter(|value| !value.trim().is_empty());

        Self {
            tool: "shell".to_string(),
            client,
            client_version,
            client_source: "cli".to_string(),
            session_id,
            session_source,
            project: project.map(|path| path.to_string_lossy().to_string()),
            agent,
        }
    }

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
    let mut params = serde_json::Map::from_iter([
        ("run_id".into(), output.run_id.clone().into()),
        (
            "argv".into(),
            serde_json::Value::Array(
                redact_argv(&output.invocation.argv)
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ),
        ),
        (
            "command_line".into(),
            render_redacted_command_line(&output.invocation.argv).into(),
        ),
        (
            "command_string".into(),
            output
                .invocation
                .command_string()
                .map(Into::into)
                .unwrap_or(serde_json::Value::Null),
        ),
        (
            "cwd".into(),
            output
                .invocation
                .cwd()
                .map(|path| path.to_string_lossy().to_string())
                .map(Into::into)
                .unwrap_or(serde_json::Value::Null),
        ),
        ("full".into(), output.requested_full.into()),
        ("family".into(), output.pattern.label().into()),
        ("exit_code".into(), output.exit_code.into()),
        ("render_mode".into(), output.output_mode.label().into()),
        (
            "full_output_bytes".into(),
            (output.full_output.len() as i64).into(),
        ),
        (
            "displayed_output_bytes".into(),
            (displayed_output.len() as i64).into(),
        ),
    ]);
    output.capture.insert_json_fields(&mut params);
    serde_json::Value::Object(params)
}

fn render_redacted_command_line(argv: &[String]) -> String {
    let redacted_argv = redact_argv(argv);
    super::types::ShellInvocation::render_argv(&redacted_argv)
}

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;
