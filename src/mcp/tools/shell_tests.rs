use std::path::PathBuf;

use rmcp::model::JsonObject;

use super::{infer_connection_cwd, render_tool_text, resolve_cwd};
use crate::daemon::watch_manager::{Consumer, ProjectStatus, WatchState};

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

    let cwd = infer_connection_cwd(&statuses, "codex", "conn-1").unwrap();
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

    assert!(infer_connection_cwd(&statuses, "codex", "conn-1").is_err());
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
