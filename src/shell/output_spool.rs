use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::types::{CaptureMetadata, RunOutput};

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
    pub capture: CaptureMetadata,
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
    capture: CaptureMetadata,
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

pub fn spooled_output(
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
        capture: meta.capture,
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
        capture: output.capture,
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
#[path = "output_spool_tests.rs"]
mod tests;
