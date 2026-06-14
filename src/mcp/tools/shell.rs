//! `so_shell` tool — execute a local shell command and return compressed output.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use super::shell_contract::build_shell_structured;
use super::{infer_connection_project_for_path, infer_connection_project_root};
use crate::core_events::{Timer, enqueue};
use crate::daemon::WatchManager;
use crate::daemon::watch_manager::ProjectStatus;
use crate::mcp::prefers_plain_text_tool_output;
use crate::shell::{
    ShellEventContext, ShellOutputMode, ShellRunOptions, ShellRunner, SpoolOwner,
    build_shell_error_event, build_shell_event,
};

const FULL_REASON_TEE_MISSING_OR_EXPIRED: &str = "tee_missing_or_expired";

enum ShellToolRequest {
    Argv(Vec<String>),
    Command(String),
}

pub fn route(wm: Arc<WatchManager>) -> ToolRoute<BuiltinServer> {
    ToolRoute::new_dyn(
        Tool::new(
            "so_shell",
            "Execute a local shell command in the active project. The returned text content is the actual command output to use directly. Prefer this compressed result as the final answer for normal shell requests. Only reach for so_shell_output when the user explicitly asks for verbatim raw output or the compressed result is missing required detail. Do not rerun the same command in a native shell just to confirm stdout.",
            schema(),
        ),
        move |ctx| {
            let wm = Arc::clone(&wm);
            Box::pin(async move { handler(ctx, &wm) })
        },
    )
}

fn handler(
    ctx: ToolCallContext<'_, BuiltinServer>,
    wm: &WatchManager,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let client = ctx.service.client();
    let client_version = ctx.service.client_version();
    let connection_id = ctx.service.connection_id();

    let args = ctx
        .arguments
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing arguments", None))?;

    let (session_id, session_source) = match args
        .get("_so_session_id")
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
    {
        Some(sid) => (sid.to_string(), "hook"),
        None => (connection_id.clone(), "connection"),
    };

    let shell_request = parse_shell_request(&args)?;
    let full = parse_full_request(&args)?;
    let statuses = wm.status();
    let cwd = resolve_cwd(&args, &client, &connection_id, &statuses)?;
    let cwd_display = cwd.display().to_string();
    let project_root = infer_connection_project_for_path(
        &statuses,
        client.as_deref().unwrap_or("unknown"),
        &connection_id,
        &cwd_display,
    )
    .unwrap_or_else(|| cwd.clone());
    let timer = Timer::start();
    let runner = ShellRunner::new(ShellRunOptions::new(full).with_spool_owner(SpoolOwner {
        client: client.clone(),
        session_id: session_id.clone(),
    }));
    let result = match &shell_request {
        ShellToolRequest::Argv(argv) => runner.run_in_dir(argv, Some(cwd.clone())),
        ShellToolRequest::Command(command) => runner.run_command_string(command, Some(cwd.clone())),
    };
    let duration_ms = timer.elapsed_ms();

    match result {
        Ok(output) => {
            let client_prefers_plain_text = prefers_plain_text_tool_output(client.as_deref());
            let text = render_tool_text(
                output.displayed_output(),
                output.exit_code,
                output.output_mode,
                client_prefers_plain_text,
                &output.run_id,
            );
            enqueue(build_shell_event(
                ShellEventContext::mcp_shell(
                    client.clone(),
                    client_version.clone(),
                    session_id,
                    session_source,
                    project_root.clone(),
                ),
                &output,
                &text,
                duration_ms,
            ));

            let mut tool_result = if output.exit_code == 0 {
                CallToolResult::success(vec![Content::text(text)])
            } else {
                CallToolResult::error(vec![Content::text(text)])
            };
            if !client_prefers_plain_text {
                tool_result.structured_content = Some(serde_json::Value::Object(
                    build_shell_structured(&output, cwd_display, full),
                ));
            }
            Ok(tool_result)
        }
        Err(error) => {
            let error_argv = match &shell_request {
                ShellToolRequest::Argv(argv) => argv.clone(),
                ShellToolRequest::Command(command) => output_argv_for_command(command),
            };
            enqueue(build_shell_error_event(
                ShellEventContext::mcp_shell(
                    client,
                    client_version,
                    session_id,
                    session_source,
                    project_root.clone(),
                ),
                &error_argv,
                Some(cwd.as_path()),
                full,
                duration_ms,
                &error.to_string(),
            ));
            Err(rmcp::ErrorData::internal_error(
                format!("failed to execute shell command: {error}"),
                None,
            ))
        }
    }
}

fn parse_shell_request(args: &JsonObject) -> Result<ShellToolRequest, rmcp::ErrorData> {
    let command = parse_command(args)?;
    let argv = parse_optional_argv(args)?;

    match (command, argv) {
        (Some(_command), Some(argv)) => {
            if argv.is_empty() {
                return Err(rmcp::ErrorData::invalid_params(
                    "argv must contain at least one string when provided",
                    None,
                ));
            }
            Ok(ShellToolRequest::Argv(argv))
        }
        (Some(command), None) => Ok(ShellToolRequest::Command(command)),
        (None, Some(argv)) => {
            if argv.is_empty() {
                return Err(rmcp::ErrorData::invalid_params(
                    "argv must contain at least one string when provided",
                    None,
                ));
            }
            Ok(ShellToolRequest::Argv(argv))
        }
        (None, None) => Err(rmcp::ErrorData::invalid_params(
            "missing command input: provide command or argv",
            None,
        )),
    }
}

fn parse_command(args: &JsonObject) -> Result<Option<String>, rmcp::ErrorData> {
    match args.get("command") {
        Some(serde_json::Value::String(command)) => {
            let trimmed = command.trim();
            if trimmed.is_empty() {
                return Err(rmcp::ErrorData::invalid_params(
                    "command must not be empty when provided",
                    None,
                ));
            }
            Ok(Some(trimmed.to_string()))
        }
        Some(_) => Err(rmcp::ErrorData::invalid_params(
            "command must be a string when provided",
            None,
        )),
        None => Ok(None),
    }
}

fn parse_optional_argv(args: &JsonObject) -> Result<Option<Vec<String>>, rmcp::ErrorData> {
    let Some(argv) = args.get("argv") else {
        return Ok(None);
    };
    let argv = argv
        .as_array()
        .ok_or_else(|| rmcp::ErrorData::invalid_params("argv must be an array of strings", None))?;

    argv.iter()
        .map(|value| {
            value.as_str().map(ToString::to_string).ok_or_else(|| {
                rmcp::ErrorData::invalid_params("argv entries must be strings", None)
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn output_argv_for_command(command: &str) -> Vec<String> {
    crate::shell::logical_argv_for_shell_command(command)
}

fn parse_full_request(args: &JsonObject) -> Result<bool, rmcp::ErrorData> {
    let full = args
        .get("full")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    if !full {
        return Ok(false);
    }

    let reason = args
        .get("full_reason")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            rmcp::ErrorData::invalid_params(
                "full=true requires full_reason=tee_missing_or_expired; use compressed output first, then so_shell_output, and only rerun full when tee is unavailable and you need a larger bounded raw capture",
                None,
            )
        })?;

    if reason != FULL_REASON_TEE_MISSING_OR_EXPIRED {
        return Err(rmcp::ErrorData::invalid_params(
            "full_reason must be tee_missing_or_expired; use compressed output first, then so_shell_output, and only rerun full when tee is unavailable and you need a larger bounded raw capture",
            None,
        ));
    }

    Ok(true)
}

fn resolve_cwd(
    args: &JsonObject,
    client: &Option<String>,
    connection_id: &str,
    statuses: &[ProjectStatus],
) -> Result<PathBuf, rmcp::ErrorData> {
    if let Some(cwd) = args.get("cwd").and_then(serde_json::Value::as_str) {
        if cwd.trim().is_empty() {
            return Err(rmcp::ErrorData::invalid_params(
                "cwd must not be empty when provided",
                None,
            ));
        }
        let cwd_path = PathBuf::from(cwd);
        if cwd_path.is_absolute() {
            return Ok(cwd_path);
        }

        let project_root = infer_connection_project_root(
            statuses,
            client.as_deref().unwrap_or("unknown"),
            connection_id,
        )?;
        return Ok(project_root.join(cwd_path));
    }

    infer_connection_project_root(
        statuses,
        client.as_deref().unwrap_or("unknown"),
        connection_id,
    )
}

fn render_tool_text(
    rendered: &str,
    exit_code: i32,
    output_mode: ShellOutputMode,
    client_prefers_plain_text: bool,
    run_id: &str,
) -> String {
    let mut text = if exit_code == 0 {
        rendered.to_string()
    } else if rendered.is_empty() {
        format!("[shell] exit_code={exit_code}\n")
    } else {
        format!(
            "{}[shell] exit_code={exit_code}\n",
            ensure_trailing_newline(rendered)
        )
    };

    if client_prefers_plain_text && output_mode == ShellOutputMode::Compressed {
        text = append_codex_follow_up_hint(&text, run_id);
    }

    text
}

fn ensure_trailing_newline(text: &str) -> String {
    if text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}\n")
    }
}

fn append_codex_follow_up_hint(text: &str, run_id: &str) -> String {
    format!(
        "{}[so-context follow-up] run_id={run_id} tool=so_shell_output\n",
        ensure_trailing_newline(text)
    )
}

fn schema() -> Arc<JsonObject> {
    Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Preferred for shell-style commands and host UI display. Executes through the current shell when argv is omitted. When both command and argv are provided, argv remains the exact command that is executed."
                },
                "argv": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 1,
                    "description": "Exact command and arguments to execute without a shell wrapper. Use this when you need precise argv semantics."
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory for the command. Defaults to the sole watched project for the current MCP connection."
                },
                "full": {
                    "type": "boolean",
                    "default": false,
                    "description": "Deprecated escape hatch. Leave this false by default. Normal workflow is: use compressed output first, then so_shell_output for cached raw output, and only rerun with full=true when tee is unavailable and you need a larger bounded raw capture."
                },
                "full_reason": {
                    "type": "string",
                    "enum": ["tee_missing_or_expired"],
                    "description": "Required only when full=true. Use this only after so_shell_output could not return raw output because tee is missing or expired, and you need a larger bounded raw capture."
                },
                "_so_session_id": {
                    "type": "string",
                    "description": "Agent session ID injected by the so-context PreToolUse hook. Do not set manually."
                }
            },
            "anyOf": [
                { "required": ["command"] },
                { "required": ["argv"] }
            ]
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    )
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
