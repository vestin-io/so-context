use std::path::PathBuf;

use rmcp::model::JsonObject;

use super::{build_structured_content, parse_full_request, render_tool_text, resolve_cwd};
use crate::daemon::watch_manager::{Consumer, ProjectStatus, WatchState};
use crate::mcp::tools::infer_connection_project_root;
use crate::shell::ShellOutputMode;

#[test]
fn infers_cwd_from_single_matching_project() {
    let statuses = vec![
        ProjectStatus {
            path: PathBuf::from("/tmp/alpha"),
            state: WatchState::Running,
            ref_count: 1,
            consumers: vec![Consumer {
                client: "codex".into(),
                session_id: "conn-1".into(),
            }],
        },
        ProjectStatus {
            path: PathBuf::from("/tmp/beta"),
            state: WatchState::Running,
            ref_count: 1,
            consumers: vec![Consumer {
                client: "codex".into(),
                session_id: "conn-2".into(),
            }],
        },
    ];

    let cwd = infer_connection_project_root(&statuses, "codex", "conn-1").unwrap();
    assert_eq!(cwd, PathBuf::from("/tmp/alpha"));
}

#[test]
fn errors_when_multiple_projects_match_connection() {
    let statuses = vec![
        ProjectStatus {
            path: PathBuf::from("/tmp/alpha"),
            state: WatchState::Running,
            ref_count: 1,
            consumers: vec![Consumer {
                client: "codex".into(),
                session_id: "conn-1".into(),
            }],
        },
        ProjectStatus {
            path: PathBuf::from("/tmp/beta"),
            state: WatchState::Running,
            ref_count: 1,
            consumers: vec![Consumer {
                client: "codex".into(),
                session_id: "conn-1".into(),
            }],
        },
    ];

    assert!(infer_connection_project_root(&statuses, "codex", "conn-1").is_err());
}

#[test]
fn nonzero_exit_code_is_appended_to_tool_text() {
    let rendered = render_tool_text("summary line", 7);
    assert_eq!(rendered, "summary line\n[shell] exit_code=7\n");
}

#[test]
fn resolves_relative_cwd_against_inferred_project_root() {
    let statuses = vec![ProjectStatus {
        path: PathBuf::from("/tmp/alpha"),
        state: WatchState::Running,
        ref_count: 1,
        consumers: vec![Consumer {
            client: "codex".into(),
            session_id: "conn-1".into(),
        }],
    }];

    let args: JsonObject = serde_json::json!({ "cwd": "src/bin" })
        .as_object()
        .cloned()
        .unwrap();

    let cwd = resolve_cwd(&args, &Some("codex".into()), "conn-1", &statuses).unwrap();
    assert_eq!(cwd, PathBuf::from("/tmp/alpha").join("src/bin"));
}

#[test]
fn keeps_absolute_cwd_as_is() {
    let args: JsonObject = serde_json::json!({ "cwd": "/tmp/alpha/src" })
        .as_object()
        .cloned()
        .unwrap();

    let cwd = resolve_cwd(&args, &Some("codex".into()), "conn-1", &[]).unwrap();
    assert_eq!(cwd, PathBuf::from("/tmp/alpha/src"));
}

#[test]
fn rejects_full_without_tee_missing_reason() {
    let args: JsonObject = serde_json::json!({ "full": true })
        .as_object()
        .cloned()
        .unwrap();

    assert!(parse_full_request(&args).is_err());
}

#[test]
fn accepts_full_with_tee_missing_reason() {
    let args: JsonObject = serde_json::json!({
        "full": true,
        "full_reason": "tee_missing_or_expired"
    })
    .as_object()
    .cloned()
    .unwrap();

    assert!(parse_full_request(&args).unwrap());
}

#[test]
fn compressed_output_advertises_follow_up_tool() {
    let content = build_structured_content(
        "run-123",
        vec!["git".into(), "status".into()],
        "/tmp/project".into(),
        0,
        false,
        ShellOutputMode::Compressed,
        false,
        false,
    );

    assert_eq!(content["content_kind"], "compressed_summary");
    assert_eq!(content["preferred_response_source"], "compressed_summary");
    assert_eq!(content["current_text_is_raw_output"], false);
    assert_eq!(content["raw_output_available"], true);
    assert_eq!(content["follow_up_tool"], "so_shell_output");
}

#[test]
fn raw_fallback_output_does_not_advertise_follow_up_tool() {
    let content = build_structured_content(
        "run-123",
        vec!["git".into(), "status".into()],
        "/tmp/project".into(),
        0,
        false,
        ShellOutputMode::RawFallback,
        false,
        true,
    );

    assert_eq!(content["content_kind"], "raw_output");
    assert_eq!(content["preferred_response_source"], "current_text_content");
    assert_eq!(content["current_text_is_raw_output"], true);
    assert!(content.get("raw_output_available").is_none());
    assert!(content.get("follow_up_tool").is_none());
}

#[test]
fn verbatim_compressed_output_does_not_advertise_follow_up_tool() {
    let content = build_structured_content(
        "run-123",
        vec![
            "sed".into(),
            "-n".into(),
            "1,10p".into(),
            "src/main.rs".into(),
        ],
        "/tmp/project".into(),
        0,
        false,
        ShellOutputMode::Compressed,
        false,
        true,
    );

    assert_eq!(content["content_kind"], "raw_output");
    assert_eq!(content["preferred_response_source"], "current_text_content");
    assert_eq!(content["current_text_is_raw_output"], true);
    assert!(content.get("raw_output_available").is_none());
    assert!(content.get("follow_up_tool").is_none());
}
