use std::path::PathBuf;

use crate::core_event_spool::{
    resolve_reliable_project_root, spool_global_event_record_for_home, spool_project_event_record,
    sync_global_event_spool_for_home, sync_project_event_spool,
};
use crate::core_events::EventRecord;

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("so-context-event-spool-{name}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn sample_event() -> EventRecord {
    let mut event = EventRecord::new("session-1", "shell");
    event.project = Some("/tmp/project".into());
    event.params = Some("{\"ok\":true}".into());
    event
}

#[test]
fn spools_and_syncs_events_from_project_root() {
    let root = temp_root("project");
    std::fs::create_dir_all(root.join(".so-context")).unwrap();
    let event = sample_event();

    let path = spool_project_event_record(&root, &event).unwrap();
    assert!(path.exists());

    let mut persisted = Vec::new();
    let synced = sync_project_event_spool(&root, |event| {
        persisted.push(event.event_id.clone());
        Ok(())
    })
    .unwrap();

    assert_eq!(synced, 1);
    assert_eq!(persisted, vec![event.event_id]);
}

#[test]
fn keeps_project_spool_when_persist_fails() {
    let root = temp_root("persist-fail");
    std::fs::create_dir_all(root.join(".so-context")).unwrap();
    let event = sample_event();

    let path = spool_project_event_record(&root, &event).unwrap();
    let synced = sync_project_event_spool(&root, |_| Err("db unavailable".to_string())).unwrap();

    assert_eq!(synced, 0);
    assert!(path.exists());
}

#[test]
fn falls_back_to_global_spool_when_project_root_is_not_reliable() {
    let fake_home = std::env::temp_dir().join("so-context-global-spool-home");
    let _ = std::fs::remove_dir_all(&fake_home);
    std::fs::create_dir_all(&fake_home).unwrap();

    let event = sample_event();
    let global_path = spool_global_event_record_for_home(&fake_home, &event).unwrap();
    assert!(global_path.exists());

    let mut persisted = Vec::new();
    let synced = sync_global_event_spool_for_home(&fake_home, |event| {
        persisted.push(event.event_id.clone());
        Ok(())
    })
    .unwrap();

    assert_eq!(synced, 1);
    assert_eq!(persisted, vec![event.event_id]);
}

#[test]
fn resolves_git_root_as_reliable_project_root() {
    let root = temp_root("git-root");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(root.join("src/nested")).unwrap();

    let resolved = resolve_reliable_project_root(Some(&root.join("src/nested/file.rs"))).unwrap();
    assert_eq!(resolved, root);
}
