use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use super::{
    EventSpoolTarget, resolve_reliable_project_root, spool_event_record, sync_global_event_spool,
    sync_project_event_spool,
};
use crate::core_events::EventRecord;

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn env_lock() -> &'static Mutex<()> {
    ENV_LOCK.get_or_init(|| Mutex::new(()))
}

fn temp_project_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("so-context-event-spool-{name}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join(".git")).unwrap();
    root
}

#[test]
fn spools_and_syncs_events_from_project_root() {
    let _guard = env_lock().lock().unwrap();
    let root = temp_project_root("roundtrip");
    let mut event = EventRecord::new("session-1", "shell");
    event.project = Some(root.to_string_lossy().to_string());

    let path = spool_event_record(EventSpoolTarget::Project(&root), &event).unwrap();
    assert!(path.exists());

    let mut persisted = Vec::new();
    let synced = sync_project_event_spool(&root, |event| {
        persisted.push(event.session_id.clone());
        Ok(())
    })
    .unwrap();

    assert_eq!(synced, 1);
    assert_eq!(persisted, vec!["session-1".to_string()]);
    assert!(!path.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn resolves_nested_paths_to_git_root() {
    let _guard = env_lock().lock().unwrap();
    let root = temp_project_root("nested");
    let nested = root.join("src").join("bin");
    fs::create_dir_all(&nested).unwrap();

    let event = EventRecord::new("session-2", "shell");
    let path = spool_event_record(EventSpoolTarget::Project(&root), &event).unwrap();

    assert!(path.starts_with(root.join(".so-context").join("event-spool")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn keeps_file_when_persist_fails() {
    let _guard = env_lock().lock().unwrap();
    let root = temp_project_root("retry");
    let event = EventRecord::new("session-3", "shell");
    let path = spool_event_record(EventSpoolTarget::Project(&root), &event).unwrap();

    let synced = sync_project_event_spool(&root, |_| Err("db unavailable".to_string())).unwrap();

    assert_eq!(synced, 0);
    assert!(path.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn ttl_cleanup_removes_expired_files() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("SO_CONTEXT_EVENT_SPOOL_TTL_SECS", "0");
    }

    let root = temp_project_root("ttl");
    let event = EventRecord::new("session-4", "shell");
    let path = spool_event_record(EventSpoolTarget::Project(&root), &event).unwrap();

    thread::sleep(Duration::from_millis(5));

    let synced = sync_project_event_spool(&root, |_| Ok(())).unwrap();
    assert_eq!(synced, 0);
    assert!(!path.exists());

    unsafe {
        std::env::remove_var("SO_CONTEXT_EVENT_SPOOL_TTL_SECS");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rotates_oldest_files_when_total_size_exceeds_limit() {
    let _guard = env_lock().lock().unwrap();
    unsafe {
        std::env::set_var("SO_CONTEXT_EVENT_SPOOL_MAX_BYTES", "700");
    }

    let root = temp_project_root("rotate");
    let event = EventRecord::new("session-5", "shell");

    let first = spool_event_record(EventSpoolTarget::Project(&root), &event).unwrap();
    thread::sleep(Duration::from_millis(2));
    let second = spool_event_record(EventSpoolTarget::Project(&root), &event).unwrap();
    thread::sleep(Duration::from_millis(2));
    let third = spool_event_record(EventSpoolTarget::Project(&root), &event).unwrap();

    assert!(!first.exists());
    assert!(second.exists() || third.exists());

    let mut seen = Vec::new();
    let _ = sync_project_event_spool(&root, |event| {
        seen.push(event.session_id.clone());
        Ok(())
    })
    .unwrap();
    assert!(!seen.is_empty());

    unsafe {
        std::env::remove_var("SO_CONTEXT_EVENT_SPOOL_MAX_BYTES");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn falls_back_to_global_spool_when_project_root_is_not_reliable() {
    let _guard = env_lock().lock().unwrap();
    let original_home = std::env::var("HOME").ok();
    let fake_home = std::env::temp_dir().join("so-context-global-spool-home");
    let _ = fs::remove_dir_all(&fake_home);
    fs::create_dir_all(&fake_home).unwrap();
    unsafe {
        std::env::set_var("HOME", &fake_home);
    }
    let path = std::env::temp_dir().join("so-context-non-git-subdir");
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(path.join("nested")).unwrap();

    assert!(resolve_reliable_project_root(Some(&path.join("nested"))).is_none());

    let event = EventRecord::new("session-6", "shell");
    let global_path = spool_event_record(EventSpoolTarget::Global, &event).unwrap();
    assert!(global_path.exists());

    let mut seen = Vec::new();
    let synced = sync_global_event_spool(|event| {
        seen.push(event.session_id.clone());
        Ok(())
    })
    .unwrap();

    assert_eq!(synced, 1);
    assert_eq!(seen, vec!["session-6".to_string()]);

    unsafe {
        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
    }
    let _ = fs::remove_dir_all(path);
    let _ = fs::remove_dir_all(fake_home);
}
