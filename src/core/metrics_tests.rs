use super::MetricsWindow;
use super::query::{
    EventRow, classify_shell_command, is_compression_hit_eligible, is_savings_eligible,
};
use super::render::{progress_bar_for_tests, truncate_middle_for_tests};

#[test]
fn classifies_rg_commands() {
    let params = r#"{"argv":["rg","--files","."],"family":"search"}"#;
    assert_eq!(classify_shell_command(Some(params)), "rg --files");

    let params = r#"{"argv":["rg","-n","TODO","."],"family":"search"}"#;
    assert_eq!(classify_shell_command(Some(params)), "rg -n");
}

#[test]
fn classifies_git_status_short() {
    let params = r#"{"argv":["git","status","--short"],"family":"git_status"}"#;
    assert_eq!(classify_shell_command(Some(params)), "git status --short");
}

#[test]
fn progress_bar_clamps() {
    assert_eq!(progress_bar_for_tests(1.5, 5), "[█████]");
    assert_eq!(progress_bar_for_tests(-1.0, 5), "[░░░░░]");
}

#[test]
fn truncate_middle_handles_unicode_without_panicking() {
    let rendered = truncate_middle_for_tests("/tmp/中文/项目/文件.rs", 10);
    assert!(rendered.contains("..."));
    assert!(rendered.chars().count() <= 10);
}

#[test]
fn window_labels_are_stable() {
    assert_eq!(MetricsWindow::Last24Hours.label(), "24h");
    assert_eq!(MetricsWindow::Last7Days.label(), "7d");
    assert_eq!(MetricsWindow::All.label(), "all");
}

#[test]
fn read_full_is_not_savings_eligible() {
    let row = EventRow {
        tool: "so_read".to_string(),
        result_ok: true,
        duration_ms: 0,
        estimated_origin_tokens: 0,
        actual_tokens: 0,
        estimated_origin_size: 0,
        actual_size: 0,
        read_mode: Some("full".to_string()),
        shell_params: None,
    };

    assert!(!is_savings_eligible(&row));
}

#[test]
fn read_outline_is_savings_eligible() {
    let row = EventRow {
        tool: "so_read".to_string(),
        result_ok: true,
        duration_ms: 0,
        estimated_origin_tokens: 0,
        actual_tokens: 0,
        estimated_origin_size: 0,
        actual_size: 0,
        read_mode: Some("outline".to_string()),
        shell_params: None,
    };

    assert!(is_savings_eligible(&row));
}

#[test]
fn search_is_not_compression_hit_eligible() {
    let row = EventRow {
        tool: "so_search".to_string(),
        result_ok: true,
        duration_ms: 0,
        estimated_origin_tokens: 10,
        actual_tokens: 3,
        estimated_origin_size: 0,
        actual_size: 0,
        read_mode: None,
        shell_params: None,
    };

    assert!(is_savings_eligible(&row));
    assert!(!is_compression_hit_eligible(&row));
}
