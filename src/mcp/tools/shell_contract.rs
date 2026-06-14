use std::path::Path;

use serde_json::{Map, Value};

use crate::shell::{RunOutput, ShellOutputMode, SpooledShellOutput};

const SHELL_OUTPUT_FOLLOW_UP_TOOL: &str = "so_shell_output";
const RAW_OUTPUT_USE_POLICY: &str =
    "only_if_user_explicitly_requests_verbatim_output_or_summary_is_missing_required_detail";
pub const RAW_OUTPUT_FETCH_POLICY: &str =
    "only_for_verbatim_user_request_or_missing_required_detail";

pub fn build_shell_structured(
    output: &RunOutput,
    cwd_display: String,
    full: bool,
) -> Map<String, Value> {
    let mut structured = Map::from_iter([
        ("run_id".into(), output.run_id.clone().into()),
        ("command".into(), output.invocation.command_line().into()),
        ("argv".into(), argv_value(output.invocation.argv.clone())),
        ("cwd".into(), cwd_display.into()),
        ("exit_code".into(), output.exit_code.into()),
        ("full".into(), full.into()),
        ("output_mode".into(), output.output_mode.label().into()),
        ("content_kind".into(), shell_content_kind(output).into()),
        ("raw_output_available".into(), true.into()),
        ("follow_up_tool".into(), SHELL_OUTPUT_FOLLOW_UP_TOOL.into()),
        (
            "preferred_response_source".into(),
            preferred_response_source(output).into(),
        ),
        ("should_fetch_raw_output".into(), false.into()),
        ("raw_output_use_policy".into(), RAW_OUTPUT_USE_POLICY.into()),
        ("output_is_in_text".into(), true.into()),
        ("rerun_not_needed_if_text_sufficient".into(), true.into()),
    ]);
    output.capture.insert_json_fields(&mut structured);
    structured
}

pub fn build_shell_output_structured(
    output: &SpooledShellOutput,
    reason: &str,
) -> Map<String, Value> {
    let mut structured = Map::from_iter([
        ("run_id".into(), output.run_id.clone().into()),
        (
            "command".into(),
            crate::shell::types::ShellInvocation::render_argv(&output.argv).into(),
        ),
        ("argv".into(), argv_value(output.argv.clone())),
        ("cwd".into(), cwd_value(output.cwd.as_deref())),
        ("exit_code".into(), output.exit_code.into()),
        ("content_kind".into(), "raw_output".into()),
        ("source".into(), "spool".into()),
        ("reason".into(), reason.into()),
        ("use_policy".into(), RAW_OUTPUT_FETCH_POLICY.into()),
        ("output_is_in_text".into(), true.into()),
        ("rerun_not_needed_if_text_sufficient".into(), true.into()),
    ]);
    output.capture.insert_json_fields(&mut structured);
    structured
}

fn shell_content_kind(output: &RunOutput) -> &'static str {
    if output.requested_full || output.output_mode == ShellOutputMode::RawFallback {
        "raw_output"
    } else {
        "compressed_summary"
    }
}

fn preferred_response_source(output: &RunOutput) -> &'static str {
    if output.requested_full || output.output_mode == ShellOutputMode::RawFallback {
        "current_text_content"
    } else {
        "compressed_summary"
    }
}

fn argv_value(argv: Vec<String>) -> Value {
    Value::Array(argv.into_iter().map(Into::into).collect())
}

fn cwd_value(path: Option<&Path>) -> Value {
    path.map(|path| path.to_string_lossy().to_string())
        .map(Into::into)
        .unwrap_or(Value::Null)
}
