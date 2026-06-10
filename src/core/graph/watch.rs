//! File-system watcher: debounced incremental sync on every meaningful change.

use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};

use super::db::{GraphDb, validate_project_root};
use super::path_filter::GitIgnoreFilter;
use super::{REINDEX_DEBOUNCE_MS, WATCH_POLL_SECS};

// ---------------------------------------------------------------------------
// Watch loop
// ---------------------------------------------------------------------------

/// Performs an initial full index and then watches the project for file changes,
/// running an incremental sync on each meaningful event.
/// A single [`GraphDb`] connection is held open for the lifetime of the watch loop.
pub fn watch_project(project_path: &str) -> Result<(), String> {
    let project_root = validate_project_root(project_path)?;

    let mut db = GraphDb::open(project_root.clone())?;

    let first = db.index()?;
    println!("{first}");
    println!("Watching for file changes: {}", project_root.display());

    let ignore_filter = GitIgnoreFilter::new(&project_root)?;

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher: RecommendedWatcher = recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| format!("failed to create watcher: {e}"))?;

    watcher
        .watch(&project_root, RecursiveMode::Recursive)
        .map_err(|e| format!("failed to watch path: {e}"))?;

    let mut last_sync = Instant::now() - Duration::from_secs(10);
    loop {
        match rx.recv_timeout(Duration::from_secs(WATCH_POLL_SECS)) {
            Ok(Ok(event)) => {
                if !is_meaningful_change(&event, &ignore_filter)
                    || last_sync.elapsed() < Duration::from_millis(REINDEX_DEBOUNCE_MS)
                {
                    continue;
                }
                last_sync = Instant::now();
                match db.sync() {
                    Ok(summary) => println!("{summary}"),
                    Err(err) => eprintln!("sync failed: {err}"),
                }
            }
            Ok(Err(err)) => eprintln!("watch error: {err}"),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                return Err("watch channel disconnected".to_string());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Event filter
// ---------------------------------------------------------------------------

/// Returns whether a filesystem event should trigger a sync.
pub fn is_meaningful_change(event: &Event, filter: &GitIgnoreFilter) -> bool {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {}
        _ => return false,
    }
    event
        .paths
        .iter()
        .any(|p| !should_ignore_path(p, filter))
}

fn should_ignore_path(path: &std::path::Path, filter: &GitIgnoreFilter) -> bool {
    let s = path.to_string_lossy();
    s.contains("/.so-context/") || filter.is_ignored(path)
}
