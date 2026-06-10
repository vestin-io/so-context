use crate::shell::types::{CaptureMetadata, ShellInvocation, ShellResult};

use super::super::{
    extract_search_query, summarize_cat, summarize_env, summarize_head, summarize_rg,
    summarize_text_excerpt,
};

#[test]
fn summarizes_text_excerpt_as_verbatim_lines() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "sed".into(),
            "-n".into(),
            "10,12p".into(),
            "src/main.rs".into(),
        ]),
        stdout: "fn demo() {\n    println!(\"hi\");\n}\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_text_excerpt(&result);
    assert_eq!(summary.summary, "");
    assert_eq!(summary.details[0], "fn demo() {");
    assert_eq!(summary.details[1], "    println!(\"hi\");");
    assert_eq!(summary.details[2], "}");
}

#[test]
fn preserves_full_text_excerpt_when_range_is_explicit() {
    let stdout = (1..=120)
        .map(|index| format!("line {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "sed".into(),
            "-n".into(),
            "1,220p".into(),
            "README.md".into(),
        ]),
        stdout: format!("{stdout}\n"),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_text_excerpt(&result);
    assert_eq!(summary.summary, "");
    assert_eq!(summary.details[0], "line 1");
    assert_eq!(summary.details[79], "line 80");
    assert_eq!(summary.details[119], "line 120");
    assert_eq!(summary.details.len(), 120);
}

#[test]
fn text_excerpt_keeps_stderr_out_of_body() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "sed".into(),
            "-n".into(),
            "1,2p".into(),
            "README.md".into(),
        ]),
        stdout: "line 1\nline 2\n".into(),
        stderr: "warning: skipped binary tail\n".into(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_text_excerpt(&result);
    assert_eq!(summary.details, vec!["line 1", "line 2"]);
    assert_eq!(summary.stderr_preview, vec!["warning: skipped binary tail"]);
}

#[test]
fn summarizes_search_hits_with_omitted_tail() {
    let stdout = (0..202)
        .map(|index| format!("src/file{index}.rs:{}: TODO item {index}", index + 1))
        .collect::<Vec<_>>()
        .join("\n");
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["rg".into(), "todo".into()]),
        stdout: format!("{stdout}\n"),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_rg(&result);
    assert_eq!(summary.summary, "");
    assert_eq!(summary.details[0], "src/file0.rs:1: TODO item 0");
    assert_eq!(summary.details[199], "src/file199.rs:200: TODO item 199");
    assert_eq!(
        summary.details.last().map(String::as_str),
        Some("+ 2 more matches")
    );
}

#[test]
fn caps_search_hits_per_file_before_global_limit() {
    let mut hits = (0..30)
        .map(|index| format!("src/main.rs:{}: TODO main {index}", index + 1))
        .collect::<Vec<_>>();
    hits.extend((0..5).map(|index| format!("src/lib.rs:{}: TODO lib {index}", index + 1)));
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["rg".into(), "todo".into()]),
        stdout: format!("{}\n", hits.join("\n")),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_rg(&result);
    assert_eq!(summary.summary, "");
    assert_eq!(summary.details.len(), 31);
    assert_eq!(summary.details[0], "src/main.rs:1: TODO main 0");
    assert_eq!(summary.details[24], "src/main.rs:25: TODO main 24");
    assert_eq!(summary.details[25], "src/lib.rs:1: TODO lib 0");
    assert_eq!(
        summary.details.last().map(String::as_str),
        Some("+ 5 more matches")
    );
}

#[test]
fn keeps_long_search_hit_lines_verbatim() {
    let long_snippet = "session-viewer/src/main.ts:412: const summary = buildInvestigationFinding(session_id, events_db_path, history_jsonl_path, session_index_path, project_root_path, browser_snapshot_manifest_path, release_notes_markdown_path, recommendation_payload_json_path);";
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["rg".into(), "session_id".into()]),
        stdout: format!("{long_snippet}\n"),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_rg(&result);
    assert_eq!(summary.summary, "");
    assert_eq!(summary.details.len(), 1);
    assert!(summary.details[0].starts_with("session-viewer/src/main.ts:412:"));
    assert!(summary.details[0].contains("session_id"));
    assert!(summary.details[0].len() < long_snippet.len());
}

#[test]
fn truncates_long_search_hits_around_query() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["rg".into(), "needle".into()]),
        stdout: format!(
            "src/huge.ts:88:{}needle{}\n",
            "a".repeat(200),
            "b".repeat(200)
        ),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_rg(&result);
    assert_eq!(summary.summary, "");
    assert_eq!(summary.details.len(), 1);
    assert!(summary.details[0].starts_with("src/huge.ts:88:"));
    assert!(summary.details[0].contains("needle"));
    assert!(summary.details[0].contains("..."));
}

#[test]
fn extracts_search_query_from_args_not_program_name() {
    let argv = vec!["rg".into(), "-n".into(), "needle".into(), ".".into()];
    assert_eq!(extract_search_query(&argv), Some("needle".to_string()));
}

#[test]
fn summarizes_head_excerpt() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec![
            "head".into(),
            "-n".into(),
            "2".into(),
            "README.md".into(),
        ]),
        stdout: "# so-context\nintro line\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_head(&result);
    assert!(summary.summary.contains("head_lines=2"));
    assert_eq!(summary.details[0], "# so-context");
}

#[test]
fn file_excerpt_keeps_stderr_out_of_body() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["cat".into(), "README.md".into()]),
        stdout: "line one\nline two\n".into(),
        stderr: "cat: note: metadata changed\n".into(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_cat(&result);
    assert_eq!(summary.details[0], "line one");
    assert_eq!(summary.details[1], "line two");
    assert_eq!(summary.stderr_preview, vec!["cat: note: metadata changed"]);
}

#[test]
fn summarizes_env_keys() {
    let result = ShellResult {
        invocation: ShellInvocation::new(vec!["env".into()]),
        stdout: "PATH=/usr/bin:/bin\nAWS_SECRET_ACCESS_KEY=super-secret\nRUST_LOG=debug\n".into(),
        stderr: String::new(),
        exit_code: 0,
        capture: CaptureMetadata::default(),
    };

    let summary = summarize_env(&result);
    assert!(summary.summary.contains("vars=3"));
    assert_eq!(summary.details[0], "PATH=2 entries");
    assert_eq!(summary.details[1], "RUST_LOG=debug");
    assert_eq!(summary.details[2], "AWS_SECRET_ACCESS_KEY=***");
}
