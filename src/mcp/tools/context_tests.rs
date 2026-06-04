use std::path::PathBuf;

use serde_json::json;

use super::{infer_connection_project_root, resolve_project_path_arg};
use crate::daemon::watch_manager::{Consumer, ProjectStatus, WatchState};

fn status(path: &str, client: &str, session_id: &str) -> ProjectStatus {
    ProjectStatus {
        path: PathBuf::from(path),
        state: WatchState::Running,
        ref_count: 1,
        consumers: vec![Consumer {
            client: client.into(),
            session_id: session_id.into(),
        }],
    }
}

#[test]
fn infers_single_project_root_for_connection() {
    let statuses = vec![
        status("/tmp/alpha", "codex", "conn-1"),
        status("/tmp/beta", "codex", "conn-2"),
    ];

    let root = infer_connection_project_root(&statuses, "codex", "conn-1").unwrap();
    assert_eq!(root, PathBuf::from("/tmp/alpha"));
}

#[test]
fn resolves_relative_project_path_against_inferred_root() {
    let args = json!({ "path": "src" }).as_object().cloned().unwrap();
    let statuses = vec![status("/tmp/alpha", "codex", "conn-1")];

    let path = resolve_project_path_arg(&args, "path", &Some("codex".into()), "conn-1", &statuses)
        .unwrap();
    assert_eq!(path, PathBuf::from("/tmp/alpha/src"));
}

#[test]
fn defaults_missing_project_path_to_inferred_root() {
    let args = json!({}).as_object().cloned().unwrap();
    let statuses = vec![status("/tmp/alpha", "codex", "conn-1")];

    let path = resolve_project_path_arg(&args, "path", &Some("codex".into()), "conn-1", &statuses)
        .unwrap();
    assert_eq!(path, PathBuf::from("/tmp/alpha"));
}
