//! WatchManager — core daemon service that monitors multiple projects.
//!
//! Responsibilities:
//! - Resolves any path to its canonical project root (nearest `.git/` ancestor)
//! - Deduplicates: multiple sessions pointing at the same repo share one thread
//! - Spawns one OS thread per unique project (rusqlite `Connection` is not `Send`)
//! - Provides `ensure_watching` for auto-discovery and `watch`/`unwatch` for
//!   explicit control
//!
//! URI helpers (`file_uri_to_path`) are also kept here so MCP hooks have a
//! single import point.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::sync::mpsc::RecvTimeoutError;

use notify::{RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};

use crate::core_graph::GraphDb;
use crate::core_graph::watch::is_meaningful_change;

// ---------------------------------------------------------------------------
// Public status types
// ---------------------------------------------------------------------------

/// Snapshot of a single watched project's runtime state.
#[derive(Debug, Clone)]
pub struct ProjectStatus {
    pub path: PathBuf,
    pub state: WatchState,
}

#[derive(Debug, Clone)]
pub enum WatchState {
    /// Initial index is running.
    Indexing,
    /// Watching for changes normally.
    Running,
    /// Watch thread exited with an error.
    Failed(String),
}

// ---------------------------------------------------------------------------
// Internal per-project handle
// ---------------------------------------------------------------------------

struct WatchHandle {
    state: Arc<Mutex<WatchState>>,
    _thread: JoinHandle<()>,
}

// ---------------------------------------------------------------------------
// WatchManager
// ---------------------------------------------------------------------------

/// Manages a registry of watched projects.
/// Pass as `Arc<WatchManager>` — cheap to clone, safe to share across threads.
pub struct WatchManager {
    inner: Arc<Mutex<ManagerInner>>,
}

struct ManagerInner {
    projects: HashMap<PathBuf, WatchHandle>,
}

impl WatchManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ManagerInner {
                projects: HashMap::new(),
            })),
        }
    }

    // -----------------------------------------------------------------------
    // Auto-discovery API
    // -----------------------------------------------------------------------

    /// Returns `true` if the canonical project root for `path` is already watched.
    pub fn is_registered(&self, path: &Path) -> bool {
        match resolve_project_root(path.to_string_lossy().as_ref()) {
            Some(root) => self.inner.lock().unwrap().projects.contains_key(&root),
            None => false,
        }
    }

    /// Resolves `path` to its canonical project root and starts watching it if
    /// not already registered. No-op if already known. Non-fatal on error.
    ///
    /// This is the primary entry point for auto-discovery hooks.
    pub fn ensure_watching(&self, path: &str) {
        let root = match resolve_project_root(path) {
            Some(r) => r,
            None => {
                eprintln!("[watch] could not resolve project root for: {path}");
                return;
            }
        };

        if self.is_registered(root.as_path()) {
            return;
        }

        if let Err(e) = self.start_watch_thread(root.clone()) {
            eprintln!("[watch] failed to watch {}: {e}", root.display());
        } else {
            eprintln!("[watch] watching: {}", root.display());
        }
    }

    /// Returns a snapshot of all currently registered projects and their state.
    pub fn status(&self) -> Vec<ProjectStatus> {
        let inner = self.inner.lock().unwrap();
        inner
            .projects
            .iter()
            .map(|(path, handle)| ProjectStatus {
                path: path.clone(),
                state: handle.state.lock().unwrap().clone(),
            })
            .collect()
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    fn start_watch_thread(&self, root: PathBuf) -> Result<(), String> {
        let state = Arc::new(Mutex::new(WatchState::Indexing));

        let thread = thread::Builder::new()
            .name(format!("watch:{}", root.display()))
            .spawn({
                let state = Arc::clone(&state);
                let root = root.clone();
                move || run_watch_thread(root, state)
            })
            .map_err(|e| format!("failed to spawn watch thread: {e}"))?;

        self.inner
            .lock()
            .unwrap()
            .projects
            .insert(root, WatchHandle { state, _thread: thread });

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Per-project watch thread
// ---------------------------------------------------------------------------

const REINDEX_DEBOUNCE_MS: u64 = 700;
const WATCH_POLL_SECS: u64 = 1;

fn run_watch_thread(
    project_root: PathBuf,
    state: Arc<Mutex<WatchState>>,
) {
    if let Err(e) = watch_loop(&project_root, &state) {
        *state.lock().unwrap() = WatchState::Failed(e);
    }
}

fn watch_loop(
    project_root: &PathBuf,
    state: &Arc<Mutex<WatchState>>,
) -> Result<(), String> {
    let mut db = GraphDb::open(project_root.clone())?;

    let summary = db.index()?;
    eprintln!("[watch] {summary}");
    *state.lock().unwrap() = WatchState::Running;

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher: RecommendedWatcher = recommended_watcher(move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| format!("failed to create watcher: {e}"))?;

    watcher
        .watch(project_root, RecursiveMode::Recursive)
        .map_err(|e| format!("failed to watch path: {e}"))?;

    let mut last_sync = Instant::now() - Duration::from_secs(10);

    loop {
        match rx.recv_timeout(Duration::from_secs(WATCH_POLL_SECS)) {
            Ok(Ok(event)) => {
                if !is_meaningful_change(&event)
                    || last_sync.elapsed() < Duration::from_millis(REINDEX_DEBOUNCE_MS)
                {
                    continue;
                }
                last_sync = Instant::now();
                match db.sync() {
                    Ok(summary) => eprintln!("[watch] {summary}"),
                    Err(err) => eprintln!("[watch] sync failed: {err}"),
                }
            }
            Ok(Err(err)) => eprintln!("[watch] watcher error: {err}"),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                return Err("watch channel disconnected".to_string());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Root resolution
// ---------------------------------------------------------------------------

/// Walks up from `path` to find the nearest directory containing `.git/`.
/// Falls back to `path` itself if no `.git/` ancestor is found.
/// Returns `None` if `path` does not exist or cannot be resolved.
pub fn resolve_project_root(path: &str) -> Option<PathBuf> {
    let start = PathBuf::from(path);
    let start = if start.is_absolute() {
        start
    } else {
        std::env::current_dir().ok()?.join(start)
    };

    let dir = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.clone()
    };

    let mut current = dir.as_path();
    loop {
        if current.join(".git").exists() {
            return Some(current.to_path_buf());
        }
        match current.parent() {
            Some(p) => current = p,
            None => break,
        }
    }

    if dir.is_dir() { Some(dir) } else { None }
}

// ---------------------------------------------------------------------------
// URI helpers
// ---------------------------------------------------------------------------

/// Converts a `file://` URI to a filesystem path string.
pub fn file_uri_to_path(uri: &str) -> String {
    if let Ok(url) = url::Url::parse(uri) {
        if url.scheme() == "file" {
            if let Ok(path) = url.to_file_path() {
                return path.to_string_lossy().to_string();
            }
        }
    }
    uri.trim_start_matches("file://").to_string()
}
