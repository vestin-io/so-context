use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::types::RunOutput;

const SPOOL_TTL_SECS: u64 = 24 * 60 * 60;
const MAX_SPOOL_BYTES: u64 = 256 * 1024 * 1024;
const TEE_DIR_ENV: &str = "SO_CONTEXT_SHELL_TEE_DIR";

#[derive(Debug, Clone)]
pub struct SpooledShellOutput {
    pub run_id: String,
    pub client: Option<String>,
    pub session_id: Option<String>,
    pub argv: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub full_output: String,
    pub exit_code: i32,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub capture_stdout_limit_bytes: usize,
    pub capture_stderr_limit_bytes: usize,
    pub raw_output_complete: bool,
}

#[derive(Debug, Clone)]
pub struct SpoolOwner {
    pub client: Option<String>,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpooledShellOutputMeta {
    run_id: String,
    #[serde(default)]
    client: Option<String>,
    #[serde(default)]
    session_id: Option<String>,
    argv: Vec<String>,
    cwd: Option<PathBuf>,
    exit_code: i32,
    stdout_truncated: bool,
    stderr_truncated: bool,
    capture_stdout_limit_bytes: usize,
    capture_stderr_limit_bytes: usize,
    raw_output_complete: bool,
    created_at_epoch_secs: u64,
}

#[derive(Debug, Clone)]
struct SpoolEntry {
    log_path: PathBuf,
    meta_path: PathBuf,
    modified_at: SystemTime,
    total_bytes: u64,
}

pub fn store_run_output(output: &RunOutput, owner: Option<&SpoolOwner>) {
    store_run_output_in_dir(&tee_dir(), output, owner);
}

pub fn get_spooled_output(
    run_id: &str,
    requester_session_id: &str,
    requester_client: Option<&str>,
) -> Option<SpooledShellOutput> {
    get_spooled_output_in_dir(&tee_dir(), run_id, requester_session_id, requester_client)
}

fn read_spooled_output(tee_dir: &Path, run_id: &str) -> Option<SpooledShellOutput> {
    let log_path = tee_dir.join(format!("{run_id}.log"));
    let meta_path = tee_dir.join(format!("{run_id}.json"));
    let full_output = fs::read_to_string(log_path).ok()?;
    let meta = fs::read_to_string(meta_path).ok()?;
    let meta: SpooledShellOutputMeta = serde_json::from_str(&meta).ok()?;
    Some(SpooledShellOutput {
        run_id: meta.run_id,
        client: meta.client,
        session_id: meta.session_id,
        argv: meta.argv,
        cwd: meta.cwd,
        full_output,
        exit_code: meta.exit_code,
        stdout_truncated: meta.stdout_truncated,
        stderr_truncated: meta.stderr_truncated,
        capture_stdout_limit_bytes: meta.capture_stdout_limit_bytes,
        capture_stderr_limit_bytes: meta.capture_stderr_limit_bytes,
        raw_output_complete: meta.raw_output_complete,
    })
}

fn tee_dir() -> PathBuf {
    if let Ok(path) = std::env::var(TEE_DIR_ENV)
        && !path.trim().is_empty()
    {
        return PathBuf::from(path);
    }

    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("so-context")
            .join("shell-tee");
    }

    PathBuf::from(".so-context").join("shell-tee")
}

fn store_run_output_in_dir(tee_dir: &Path, output: &RunOutput, owner: Option<&SpoolOwner>) {
    if fs::create_dir_all(tee_dir).is_err() {
        return;
    }
    rotate_spool(
        tee_dir,
        Duration::from_secs(SPOOL_TTL_SECS),
        MAX_SPOOL_BYTES,
    );

    let log_path = tee_dir.join(format!("{}.log", output.run_id));
    let meta_path = tee_dir.join(format!("{}.json", output.run_id));
    let meta = SpooledShellOutputMeta {
        run_id: output.run_id.clone(),
        client: owner.and_then(|owner| owner.client.clone()),
        session_id: owner.map(|owner| owner.session_id.clone()),
        argv: output.invocation.argv.clone(),
        cwd: output.invocation.cwd().map(PathBuf::from),
        exit_code: output.exit_code,
        stdout_truncated: output.stdout_truncated,
        stderr_truncated: output.stderr_truncated,
        capture_stdout_limit_bytes: output.capture_stdout_limit_bytes,
        capture_stderr_limit_bytes: output.capture_stderr_limit_bytes,
        raw_output_complete: output.raw_output_complete,
        created_at_epoch_secs: now_epoch_secs(),
    };

    if fs::write(&log_path, &output.full_output).is_err() {
        return;
    }
    let meta_bytes = match serde_json::to_vec(&meta) {
        Ok(bytes) => bytes,
        Err(_) => return,
    };
    if fs::write(&meta_path, meta_bytes).is_err() {
        let _ = fs::remove_file(&log_path);
        return;
    }

    rotate_spool(
        tee_dir,
        Duration::from_secs(SPOOL_TTL_SECS),
        MAX_SPOOL_BYTES,
    );
}

fn get_spooled_output_in_dir(
    tee_dir: &Path,
    run_id: &str,
    requester_session_id: &str,
    requester_client: Option<&str>,
) -> Option<SpooledShellOutput> {
    rotate_spool(
        tee_dir,
        Duration::from_secs(SPOOL_TTL_SECS),
        MAX_SPOOL_BYTES,
    );
    let output = read_spooled_output(tee_dir, run_id)?;
    if is_spool_access_allowed(&output, requester_session_id, requester_client) {
        Some(output)
    } else {
        None
    }
}

fn is_spool_access_allowed(
    output: &SpooledShellOutput,
    requester_session_id: &str,
    requester_client: Option<&str>,
) -> bool {
    if output.session_id.as_deref() != Some(requester_session_id) {
        return false;
    }

    match (output.client.as_deref(), requester_client) {
        (Some(owner_client), Some(requester_client)) => owner_client == requester_client,
        _ => true,
    }
}

fn rotate_spool(tee_dir: &Path, ttl: Duration, max_bytes: u64) {
    let Ok(entries) = collect_entries(tee_dir) else {
        return;
    };

    let now = SystemTime::now();
    for entry in &entries {
        if now
            .duration_since(entry.modified_at)
            .unwrap_or_default()
            .gt(&ttl)
        {
            remove_entry(entry);
        }
    }

    let Ok(mut entries) = collect_entries(tee_dir) else {
        return;
    };
    entries.sort_by_key(|entry| entry.modified_at);

    let mut total_bytes: u64 = entries.iter().map(|entry| entry.total_bytes).sum();
    for entry in entries {
        if total_bytes <= max_bytes {
            break;
        }
        remove_entry(&entry);
        total_bytes = total_bytes.saturating_sub(entry.total_bytes);
    }
}

fn collect_entries(tee_dir: &Path) -> std::io::Result<Vec<SpoolEntry>> {
    if !tee_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for dir_entry in fs::read_dir(tee_dir)? {
        let dir_entry = dir_entry?;
        let path = dir_entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("log") {
            continue;
        }

        let Some(run_id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let meta_path = tee_dir.join(format!("{run_id}.json"));
        let Ok(log_meta) = fs::metadata(&path) else {
            continue;
        };
        let Ok(log_modified) = log_meta.modified() else {
            continue;
        };
        let meta_bytes = fs::metadata(&meta_path).map(|meta| meta.len()).unwrap_or(0);
        entries.push(SpoolEntry {
            log_path: path,
            meta_path,
            modified_at: log_modified,
            total_bytes: log_meta.len() + meta_bytes,
        });
    }

    Ok(entries)
}

fn remove_entry(entry: &SpoolEntry) {
    let _ = fs::remove_file(&entry.log_path);
    let _ = fs::remove_file(&entry.meta_path);
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{
        SPOOL_TTL_SECS, SpoolOwner, collect_entries, get_spooled_output_in_dir, rotate_spool,
        store_run_output_in_dir,
    };
    use crate::shell::types::{RunOutput, ShellInvocation, ShellOutputMode, ShellPattern};
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
            stdout_bytes: full_output.len(),
            stderr_bytes: 0,
            stdout_truncated: false,
            stderr_truncated: false,
            capture_stdout_limit_bytes: full_output.len(),
            capture_stderr_limit_bytes: 0,
            raw_output_complete: true,
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
        assert!(cached.raw_output_complete);

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

        rotate_spool(&dir, Duration::from_secs(SPOOL_TTL_SECS), 400);
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
}
