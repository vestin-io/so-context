//! `so_shell` tool — execute a local shell command and return compressed output.

use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRoute;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Content, JsonObject, Tool};

use super::BuiltinServer;
use super::infer_connection_project_root;
use crate::core_events::{Timer, enqueue};
use crate::daemon::WatchManager;
use crate::daemon::watch_manager::ProjectStatus;
use crate::mcp::prefers_plain_text_tool_output;
use crate::shell::{
    ShellEventContext, ShellOutputMode, ShellRunOptions, ShellRunner, SpoolOwner,
    build_shell_error_event, build_shell_event,
};

const FULL_REASON_TEE_MISSING_OR_EXPIRED: &str = "tee_missing_or_expired";

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

    let argv = parse_argv(&args)?;
    let full = parse_full_request(&args)?;
    let cwd = resolve_cwd(&args, &client, &connection_id, &wm.status())?;
    let cwd_display = cwd.display().to_string();
    let timer = Timer::start();
    let runner = ShellRunner::new(ShellRunOptions::new(full).with_spool_owner(SpoolOwner {
        client: client.clone(),
        session_id: session_id.clone(),
    }));
    let result = runner.run_in_dir(&argv, Some(cwd.clone()));
    let duration_ms = timer.elapsed_ms();

    match result {
        Ok(output) => {
            let text = render_tool_text(&output.rendered, output.exit_code);
            enqueue(build_shell_event(
                ShellEventContext::mcp_shell(
                    client.clone(),
                    client_version.clone(),
                    session_id,
                    session_source,
                    cwd.clone(),
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
            if !prefers_plain_text_tool_output(client.as_deref()) {
                tool_result.structured_content = Some(build_structured_content(
                    &output.run_id,
                    argv,
                    cwd_display,
                    output.exit_code,
                    full,
                    output.output_mode,
                    output.requested_full,
                    output.rendered == output.full_output,
                ));
            }
            Ok(tool_result)
        }
        Err(error) => {
            enqueue(build_shell_error_event(
                ShellEventContext::mcp_shell(
                    client,
                    client_version,
                    session_id,
                    session_source,
                    cwd.clone(),
                ),
                &argv,
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

fn parse_argv(args: &JsonObject) -> Result<Vec<String>, rmcp::ErrorData> {
    let argv = args
        .get("argv")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| rmcp::ErrorData::invalid_params("missing array argument: argv", None))?;
    if argv.is_empty() {
        return Err(rmcp::ErrorData::invalid_params(
            "argv must contain at least one string",
            None,
        ));
    }

    argv.iter()
        .map(|value| {
            value.as_str().map(ToString::to_string).ok_or_else(|| {
                rmcp::ErrorData::invalid_params("argv entries must be strings", None)
            })
        })
        .collect()
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
                "full=true requires full_reason=tee_missing_or_expired; use compressed output first, then so_shell_output, and only rerun full when tee is unavailable",
                None,
            )
        })?;

    if reason != FULL_REASON_TEE_MISSING_OR_EXPIRED {
        return Err(rmcp::ErrorData::invalid_params(
            "full_reason must be tee_missing_or_expired; use compressed output first, then so_shell_output, and only rerun full when tee is unavailable",
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

fn build_structured_content(
    run_id: &str,
    argv: Vec<String>,
    cwd_display: String,
    exit_code: i32,
    full: bool,
    output_mode: ShellOutputMode,
    requested_full: bool,
    current_text_matches_full: bool,
) -> serde_json::Value {
    let current_text_is_raw =
        requested_full || output_mode == ShellOutputMode::RawFallback || current_text_matches_full;
    let mut content = serde_json::json!({
        "run_id": run_id,
        "argv": argv,
        "cwd": cwd_display,
        "exit_code": exit_code,
        "full": full,
        "output_mode": output_mode.label(),
        "content_kind": if current_text_is_raw {
            "raw_output"
        } else {
            "compressed_summary"
        },
        "preferred_response_source": if current_text_is_raw {
            "current_text_content"
        } else {
            "compressed_summary"
        },
        "current_text_is_raw_output": current_text_is_raw,
        "output_is_in_text": true,
        "rerun_not_needed_if_text_sufficient": true,
    });

    if !current_text_is_raw {
        content["raw_output_available"] = serde_json::Value::Bool(true);
        content["follow_up_tool"] = serde_json::Value::String("so_shell_output".to_string());
        content["should_fetch_raw_output"] = serde_json::Value::Bool(false);
        content["raw_output_use_policy"] = serde_json::Value::String(
            "only_if_user_explicitly_requests_verbatim_output_or_summary_is_missing_required_detail"
                .to_string(),
        );
    }

    content
}

fn render_tool_text(rendered: &str, exit_code: i32) -> String {
    if exit_code == 0 {
        return rendered.to_string();
    }

    if rendered.is_empty() {
        return format!("[shell] exit_code={exit_code}\n");
    }

    format!(
        "{}[shell] exit_code={exit_code}\n",
        ensure_trailing_newline(rendered)
    )
}

fn ensure_trailing_newline(text: &str) -> String {
    if text.ends_with('\n') {
        text.to_string()
    } else {
        format!("{text}\n")
    }
}

fn schema() -> Arc<JsonObject> {
    Arc::new(
        serde_json::json!({
            "type": "object",
            "properties": {
                "argv": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 1,
                    "description": "Command and arguments to execute without a shell wrapper."
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory for the command. Defaults to the sole watched project for the current MCP connection."
                },
                "full": {
                    "type": "boolean",
                    "default": false,
                    "description": "Deprecated escape hatch. Leave this false by default. Normal workflow is: use compressed output first, then so_shell_output for cached raw output, and only rerun with full=true when tee is unavailable."
                },
                "full_reason": {
                    "type": "string",
                    "enum": ["tee_missing_or_expired"],
                    "description": "Required only when full=true. Use this only after so_shell_output could not return raw output because tee is missing or expired."
                },
                "_so_session_id": {
                    "type": "string",
                    "description": "Agent session ID injected by the so-context PreToolUse hook. Do not set manually."
                }
            },
            "required": ["argv"]
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    )
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
