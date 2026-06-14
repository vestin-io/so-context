use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::core_events::EventRecord;

const EVENT_SPOOL_DIR: &str = "event-spool";
const SO_CONTEXT_DIR: &str = ".so-context";
const EVENT_SPOOL_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const EVENT_SPOOL_MAX_BYTES: u64 = 64 * 1024 * 1024;

#[cfg(test)]
pub fn spool_project_event_record(
    project_root: &Path,
    event: &EventRecord,
) -> Result<PathBuf, String> {
    let spool_dir = project_root.join(SO_CONTEXT_DIR).join(EVENT_SPOOL_DIR);
    spool_event_record_in_dir(&spool_dir, event)
}

pub fn spool_global_event_record(event: &EventRecord) -> Result<PathBuf, String> {
    let spool_dir = global_event_spool_dir()?;
    spool_event_record_in_dir(&spool_dir, event)
}

#[cfg(test)]
pub(crate) fn spool_global_event_record_for_home(
    home: &Path,
    event: &EventRecord,
) -> Result<PathBuf, String> {
    let spool_dir = global_event_spool_dir_for_home(home);
    spool_event_record_in_dir(&spool_dir, event)
}

fn spool_event_record_in_dir(spool_dir: &Path, event: &EventRecord) -> Result<PathBuf, String> {
    fs::create_dir_all(&spool_dir)
        .map_err(|e| format!("create event spool dir {}: {e}", spool_dir.display()))?;
    let _ = prune_event_spool_dir(&spool_dir);

    let filename = format!(
        "{}-{}-{}.json",
        timestamp_nanos(),
        std::process::id(),
        Uuid::new_v4()
    );
    let path = spool_dir.join(filename);
    let tmp_path = spool_dir.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("event")
    ));
    let payload =
        serde_json::to_vec(event).map_err(|e| format!("serialize event spool payload: {e}"))?;

    fs::write(&tmp_path, payload)
        .map_err(|e| format!("write event spool file {}: {e}", tmp_path.display()))?;
    fs::rename(&tmp_path, &path).map_err(|e| {
        format!(
            "finalize event spool file {} -> {}: {e}",
            tmp_path.display(),
            path.display()
        )
    })?;
    let _ = prune_event_spool_dir(&spool_dir);

    Ok(path)
}

pub fn sync_project_event_spool<F>(project_root: &Path, mut persist: F) -> Result<usize, String>
where
    F: FnMut(&EventRecord) -> Result<(), String>,
{
    let spool_dir = project_root.join(SO_CONTEXT_DIR).join(EVENT_SPOOL_DIR);
    sync_event_spool_dir(&spool_dir, &mut persist)
}

pub fn sync_global_event_spool<F>(mut persist: F) -> Result<usize, String>
where
    F: FnMut(&EventRecord) -> Result<(), String>,
{
    let spool_dir = global_event_spool_dir()?;
    sync_event_spool_dir(&spool_dir, &mut persist)
}

#[cfg(test)]
pub(crate) fn sync_global_event_spool_for_home<F>(
    home: &Path,
    mut persist: F,
) -> Result<usize, String>
where
    F: FnMut(&EventRecord) -> Result<(), String>,
{
    let spool_dir = global_event_spool_dir_for_home(home);
    sync_event_spool_dir(&spool_dir, &mut persist)
}

pub fn resolve_reliable_project_root(project_hint: Option<&Path>) -> Option<PathBuf> {
    let start = project_hint?;
    let current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    if current.join(SO_CONTEXT_DIR).exists() {
        return Some(current);
    }

    resolve_git_root(Some(&current))
}

fn sync_event_spool_dir<F>(spool_dir: &Path, persist: &mut F) -> Result<usize, String>
where
    F: FnMut(&EventRecord) -> Result<(), String>,
{
    if !spool_dir.exists() {
        return Ok(0);
    }
    let _ = prune_event_spool_dir(spool_dir);

    let entries = list_spool_entries(spool_dir)?;

    let mut synced = 0usize;
    for entry in entries {
        let path = entry.path;
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };

        match serde_json::from_str::<EventRecord>(&content) {
            Ok(event) => match persist(&event) {
                Ok(()) => {
                    synced += 1;
                    let _ = fs::remove_file(&path);
                }
                Err(_) => {}
            },
            Err(_) => {
                let _ = fs::remove_file(&path);
            }
        }
    }

    Ok(synced)
}

fn resolve_git_root(project_hint: Option<&Path>) -> Option<PathBuf> {
    let start = project_hint?;
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[derive(Clone)]
struct SpoolEntry {
    path: PathBuf,
    modified: SystemTime,
    len: u64,
}

fn list_spool_entries(spool_dir: &Path) -> Result<Vec<SpoolEntry>, String> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(spool_dir)
        .map_err(|e| format!("read event spool dir {}: {e}", spool_dir.display()))?
    {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        entries.push(SpoolEntry {
            modified: metadata.modified().unwrap_or(UNIX_EPOCH),
            len: metadata.len(),
            path,
        });
    }
    entries.sort_by_key(|entry| entry.modified);
    Ok(entries)
}

fn prune_event_spool_dir(spool_dir: &Path) -> Result<(), String> {
    let mut entries = list_spool_entries(spool_dir)?;
    let now = SystemTime::now();

    entries.retain(|entry| {
        if now
            .duration_since(entry.modified)
            .unwrap_or_default()
            .gt(&event_spool_ttl())
        {
            let _ = fs::remove_file(&entry.path);
            return false;
        }
        true
    });

    let mut total_bytes = entries.iter().map(|entry| entry.len).sum::<u64>();
    for entry in entries {
        if total_bytes <= event_spool_max_bytes() {
            break;
        }
        if fs::remove_file(&entry.path).is_ok() {
            total_bytes = total_bytes.saturating_sub(entry.len);
        }
    }

    Ok(())
}

fn event_spool_ttl() -> Duration {
    std::env::var("SO_CONTEXT_EVENT_SPOOL_TTL_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(EVENT_SPOOL_TTL)
}

fn event_spool_max_bytes() -> u64 {
    std::env::var("SO_CONTEXT_EVENT_SPOOL_MAX_BYTES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(EVENT_SPOOL_MAX_BYTES)
}

fn global_event_spool_dir() -> Result<PathBuf, String> {
    let home =
        std::env::var("HOME").map_err(|_| "HOME is not set for global event spool".to_string())?;
    Ok(global_event_spool_dir_for_home(Path::new(&home)))
}

fn global_event_spool_dir_for_home(home: &Path) -> PathBuf {
    home.join(".local")
        .join("share")
        .join("so-context")
        .join(EVENT_SPOOL_DIR)
}

#[cfg(test)]
#[path = "event_spool_tests.rs"]
mod tests;
