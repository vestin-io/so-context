use super::{
    SPOOL_TTL_SECS, SpoolOwner, collect_entries, get_spooled_output_in_dir, rotate_spool,
    store_run_output_in_dir,
};
use crate::shell::types::{
    CaptureMetadata, RunOutput, ShellInvocation, ShellOutputMode, ShellPattern,
};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

fn temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn sample_output(run_id: &str, full_output: &str) -> RunOutput {
    RunOutput {
        run_id: run_id.to_string(),
        invocation: ShellInvocation::with_cwd(
            vec!["echo".into(), "hello".into()],
            PathBuf::from("/tmp/project"),
        ),
        pattern: ShellPattern::Unknown,
        rendered: Some("compressed".into()),
        full_output: full_output.to_string(),
        exit_code: 0,
        output_mode: ShellOutputMode::Compressed,
        requested_full: false,
        capture: CaptureMetadata {
            stdout_bytes: full_output.len(),
            stderr_bytes: 0,
            capture_stdout_limit_bytes: full_output.len(),
            ..CaptureMetadata::default()
        },
    }
}

#[test]
fn stores_and_reads_spooled_output() {
    let dir = temp_dir("so-context-shell-tee");
    let owner = SpoolOwner {
        client: Some("codex".into()),
        session_id: "session-1".into(),
    };
    store_run_output_in_dir(&dir, &sample_output("run-1", "hello world"), Some(&owner));
    let cached = get_spooled_output_in_dir(&dir, "run-1", "session-1", Some("codex"))
        .expect("spooled output");
    assert_eq!(cached.full_output, "hello world");
    assert_eq!(cached.argv, vec!["echo".to_string(), "hello".to_string()]);
    assert!(cached.capture.raw_output_complete());
    assert!(!cached.capture.timed_out);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn removes_expired_entries_by_ttl() {
    let dir = temp_dir("so-context-shell-tee-expired");
    let log_path = dir.join("run-old.log");
    let meta_path = dir.join("run-old.json");
    fs::write(&log_path, "old").unwrap();
    fs::write(&meta_path, "{}").unwrap();
    std::thread::sleep(Duration::from_millis(1100));

    rotate_spool(&dir, Duration::from_secs(1), 1024 * 1024);
    assert!(!log_path.exists());
    assert!(!meta_path.exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rotates_oldest_entries_when_total_size_exceeds_limit() {
    let dir = temp_dir("so-context-shell-tee-rotate");

    store_run_output_in_dir(&dir, &sample_output("run-1", &"a".repeat(80)), None);
    std::thread::sleep(Duration::from_millis(10));
    store_run_output_in_dir(&dir, &sample_output("run-2", &"b".repeat(80)), None);
    std::thread::sleep(Duration::from_millis(10));
    store_run_output_in_dir(&dir, &sample_output("run-3", &"c".repeat(80)), None);

    rotate_spool(&dir, Duration::from_secs(SPOOL_TTL_SECS), 600);
    let entries = collect_entries(&dir).unwrap();
    let remaining_logs: Vec<String> = entries
        .into_iter()
        .filter_map(|entry| {
            entry
                .log_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(ToString::to_string)
        })
        .collect();
    assert!(!remaining_logs.contains(&"run-1".to_string()));
    assert!(!remaining_logs.is_empty());
    assert!(remaining_logs.contains(&"run-3".to_string()));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn denies_spooled_output_to_other_sessions() {
    let dir = temp_dir("so-context-shell-tee-access");
    let owner = SpoolOwner {
        client: Some("codex".into()),
        session_id: "session-1".into(),
    };
    store_run_output_in_dir(&dir, &sample_output("run-1", "secret"), Some(&owner));

    assert!(get_spooled_output_in_dir(&dir, "run-1", "session-2", Some("codex")).is_none());
    assert!(get_spooled_output_in_dir(&dir, "run-1", "session-1", Some("claude")).is_none());
    assert!(get_spooled_output_in_dir(&dir, "run-1", "session-1", Some("codex")).is_some());

    let _ = fs::remove_dir_all(dir);
}
