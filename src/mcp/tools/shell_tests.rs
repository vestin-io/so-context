use std::path::PathBuf;

use rmcp::model::JsonObject;

use super::{parse_full_request, parse_shell_request, render_tool_text, resolve_cwd};
use crate::daemon::watch_manager::{Consumer, ProjectStatus, WatchState};
use crate::mcp::tools::shell_contract::build_shell_structured;
use crate::mcp::tools::{infer_connection_project_for_path, infer_connection_project_root};
use crate::shell::types::{CaptureMetadata, ShellInvocation, ShellPattern};
use crate::shell::{RunOutput, ShellOutputMode};

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
    let rendered = render_tool_text(
        "summary line",
        7,
        ShellOutputMode::Compressed,
        false,
        "run-123",
    );
    assert_eq!(rendered, "summary line\n[shell] exit_code=7\n");
}

#[test]
fn codex_compressed_output_includes_follow_up_run_id_hint() {
    let rendered = render_tool_text(
        "summary line\n",
        0,
        ShellOutputMode::Compressed,
        true,
        "run-123",
    );

    assert!(rendered.contains("[so-context follow-up] run_id=run-123 tool=so_shell_output"));
}

#[test]
fn codex_raw_output_does_not_append_follow_up_hint() {
    let rendered = render_tool_text(
        "summary line\n",
        0,
        ShellOutputMode::RawFallback,
        true,
        "run-123",
    );

    assert_eq!(rendered, "summary line\n");
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
fn attributes_subdirectory_cwd_back_to_project_root() {
    let statuses = vec![ProjectStatus {
        path: PathBuf::from("/tmp/alpha"),
        state: WatchState::Running,
        ref_count: 1,
        consumers: vec![Consumer {
            client: "codex".into(),
            session_id: "conn-1".into(),
        }],
    }];

    let project =
        infer_connection_project_for_path(&statuses, "codex", "conn-1", "/tmp/alpha/src/bin")
            .unwrap();

    assert_eq!(project, PathBuf::from("/tmp/alpha"));
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
fn accepts_command_only_shell_request() {
    let args: JsonObject = serde_json::json!({ "command": "git status" })
        .as_object()
        .cloned()
        .unwrap();

    let request = parse_shell_request(&args).expect("valid shell request");
    match request {
        super::ShellToolRequest::Command(command) => assert_eq!(command, "git status"),
        super::ShellToolRequest::Argv(_) => panic!("expected command request"),
    }
}

#[test]
fn prefers_argv_when_command_and_argv_are_both_present() {
    let args: JsonObject = serde_json::json!({
        "command": "git status",
        "argv": ["git", "diff", "--stat"]
    })
    .as_object()
    .cloned()
    .unwrap();

    let request = parse_shell_request(&args).expect("valid shell request");
    match request {
        super::ShellToolRequest::Argv(argv) => {
            assert_eq!(argv, vec!["git", "diff", "--stat"]);
        }
        super::ShellToolRequest::Command(_) => panic!("expected argv request"),
    }
}

#[test]
fn rejects_missing_command_and_argv() {
    let args: JsonObject = serde_json::json!({}).as_object().cloned().unwrap();

    assert!(parse_shell_request(&args).is_err());
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
    let output = RunOutput {
        run_id: "run-123".into(),
        invocation: ShellInvocation::new(vec!["git".into(), "status".into()]),
        pattern: ShellPattern::GitStatus,
        rendered: Some("* main\nM  src/main.rs\n".into()),
        full_output: "M  src/main.rs\n".into(),
        exit_code: 0,
        output_mode: ShellOutputMode::Compressed,
        requested_full: false,
        capture: CaptureMetadata::default(),
    };
    let content = build_shell_structured(&output, "/tmp/project".into(), false);

    assert_eq!(content["command"], "git status");
    assert_eq!(content["content_kind"], "compressed_summary");
    assert_eq!(content["preferred_response_source"], "compressed_summary");
    assert_eq!(content["raw_output_available"], true);
    assert_eq!(content["follow_up_tool"], "so_shell_output");
}

#[test]
fn raw_fallback_output_does_not_advertise_follow_up_tool() {
    let output = RunOutput {
        run_id: "run-123".into(),
        invocation: ShellInvocation::new(vec!["git".into(), "status".into()]),
        pattern: ShellPattern::GitStatus,
        rendered: None,
        full_output: "M  src/main.rs\n".into(),
        exit_code: 0,
        output_mode: ShellOutputMode::RawFallback,
        requested_full: false,
        capture: CaptureMetadata::default(),
    };
    let content = build_shell_structured(&output, "/tmp/project".into(), false);

    assert_eq!(content["command"], "git status");
    assert_eq!(content["content_kind"], "raw_output");
    assert_eq!(content["preferred_response_source"], "current_text_content");
    assert_eq!(content["raw_output_available"], true);
    assert_eq!(content["follow_up_tool"], "so_shell_output");
}

#[test]
fn full_output_is_marked_raw() {
    let output = RunOutput {
        run_id: "run-123".into(),
        invocation: ShellInvocation::shell_command(
            vec![
                "sed".into(),
                "-n".into(),
                "1,10p".into(),
                "src/main.rs".into(),
            ],
            "/bin/zsh".into(),
            "sed -n '1,10p' src/main.rs".into(),
        ),
        pattern: ShellPattern::Cat,
        rendered: None,
        full_output: "fn main() {}\n".into(),
        exit_code: 0,
        output_mode: ShellOutputMode::Full,
        requested_full: true,
        capture: CaptureMetadata::default(),
    };
    let content = build_shell_structured(&output, "/tmp/project".into(), true);

    assert_eq!(content["command"], "sed -n '1,10p' src/main.rs");
    assert_eq!(content["content_kind"], "raw_output");
    assert_eq!(content["preferred_response_source"], "current_text_content");
    assert_eq!(content["raw_output_available"], true);
    assert_eq!(content["follow_up_tool"], "so_shell_output");
}
