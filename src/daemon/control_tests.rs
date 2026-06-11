use std::path::PathBuf;

use super::{command_line_matches_daemon, daemon_binary_from_command_line};

#[test]
fn matches_so_context_daemon_command_line() {
    assert!(command_line_matches_daemon(
        "/Users/test/bin/so-context daemon"
    ));
    assert!(command_line_matches_daemon(
        "/opt/homebrew/bin/so-context daemon --flag"
    ));
}

#[test]
fn rejects_non_daemon_processes() {
    assert!(!command_line_matches_daemon(
        "/Users/test/bin/so-context mcp"
    ));
    assert!(!command_line_matches_daemon("/usr/bin/python worker.py"));
}

#[test]
fn extracts_daemon_binary_path_from_command_line() {
    assert_eq!(
        daemon_binary_from_command_line("/opt/homebrew/bin/so-context daemon --flag"),
        Some(PathBuf::from("/opt/homebrew/bin/so-context"))
    );
}
