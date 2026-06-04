//! WatchManager — core daemon service that monitors multiple projects.
//!
//! Responsibilities:
//! - Resolves any path to its canonical project root (nearest `.git/` ancestor)
//! - Deduplicates: multiple agent sessions pointing at the same repo share one
//!   watch thread; a ref-count tracks how many consumers are active
//! - Spawns one OS thread per unique project (rusqlite `Connection` is not `Send`)
//! - Provides `ensure_watching` / `unwatch` with optional agent-ID tracking so
//!   callers can see which agents are consuming each project
//! - The watch thread is only truly stopped when the last consumer unwatches
//!
//! URI helpers (`file_uri_to_path`) are also kept here so MCP tools have a
//! single import point.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};

use crate::core_graph::GraphDb;
use crate::core_graph::watch::is_meaningful_change;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Identifies one agent session consuming a project watch.
#[derive(Debug, Clone)]
pub struct Consumer {
    /// MCP client name — e.g. `"claude"`, `"opencode"`, `"codex"`.
    /// Defaults to `"unknown"` if not provided.
    pub client: String,
    /// Session identifier for this connection. Auto-generated UUID if not provided.
    pub session_id: String,
}

impl Consumer {
    /// Build a `Consumer`, generating a UUID session_id when none is given.
    pub fn new(client: Option<&str>, session_id: Option<&str>) -> Self {
        Self {
            client: client.unwrap_or("unknown").to_string(),
            session_id: session_id
                .map(|s| s.to_string())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        }
    }

    /// The unique key used in the consumer map: `"<client>:<session_id>"`.
    pub fn key(&self) -> String {
        format!("{}:{}", self.client, self.session_id)
    }
}

/// Snapshot of a single watched project's runtime state.
#[derive(Debug, Clone)]
pub struct ProjectStatus {
    pub path: PathBuf,
    pub state: WatchState,
    /// Number of active consumers holding a watch reference.
    pub ref_count: usize,
    /// Active consumers currently watching this project.
    pub consumers: Vec<Consumer>,
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
// Internal stop signal
// ---------------------------------------------------------------------------

/// Sent over the stop channel to request the watch thread to exit cleanly.
struct StopSignal;

// ---------------------------------------------------------------------------
// Internal per-project handle
// ---------------------------------------------------------------------------

struct WatchHandle {
    state: Arc<Mutex<WatchState>>,
    /// Active consumers, keyed by `Consumer::key()` (`"<agent>:<session_id>"`).
    consumers: HashMap<String, Consumer>,
    /// Dropping this sender causes the watch thread to stop.
    _stop_tx: Sender<StopSignal>,
    _thread: JoinHandle<()>,
}

impl WatchHandle {
    fn ref_count(&self) -> usize {
        self.consumers.len()
    }
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
    // Public API
    // -----------------------------------------------------------------------

    /// Resolves `path` to its canonical project root and starts watching it if
    /// not already registered. If already registered, increments the ref-count.
    ///
    /// - `client` — MCP client app name (`"claude"`, `"opencode"`, …).
    ///   Defaults to `"unknown"` if `None`.
    /// - `session_id` — unique ID for this connection. Auto-generated UUID
    ///   if `None`, ensuring multiple anonymous callers are tracked separately.
    pub fn ensure_watching(&self, path: &str, client: Option<&str>, session_id: Option<&str>) {
        let root = match resolve_project_root(path) {
            Some(r) => r,
            None => {
                eprintln!("[watch] could not resolve project root for: {path}");
                return;
            }
        };

        let consumer = Consumer::new(client, session_id);
        let key = consumer.key();

        let mut inner = self.inner.lock().unwrap();

        if let Some(handle) = inner.projects.get_mut(&root) {
            if handle.consumers.contains_key(&key) {
                return; // already registered, no-op
            }
            handle.consumers.insert(key.clone(), consumer);
            eprintln!(
                "[watch] ref+1 for {} (consumer: {key}, total: {})",
                root.display(),
                handle.ref_count()
            );
            return;
        }

        drop(inner);
        match self.start_watch_thread(root.clone(), consumer) {
            Ok(()) => eprintln!("[watch] watching: {}", root.display()),
            Err(e) => eprintln!("[watch] failed to watch {}: {e}", root.display()),
        }
    }

    /// Decrements the ref-count for the project at `path`.
    ///
    /// The consumer is matched by `"<client>:<session_id>"`. If the ref-count
    /// reaches zero the watch thread is stopped.
    ///
    /// Returns `true` if the project was registered, `false` if unknown.
    pub fn unwatch(&self, path: &str, client: Option<&str>, session_id: Option<&str>) -> bool {
        let root = match resolve_project_root(path) {
            Some(r) => r,
            None => return false,
        };

        let key = Consumer::new(client, session_id).key();

        let mut inner = self.inner.lock().unwrap();
        let handle = match inner.projects.get_mut(&root) {
            Some(h) => h,
            None => return false,
        };

        handle.consumers.remove(&key);

        if handle.ref_count() == 0 {
            inner.projects.remove(&root);
            eprintln!("[watch] stopped (no consumers): {}", root.display());
        } else {
            eprintln!(
                "[watch] ref-1 for {} (removed: {key}, remaining: {})",
                root.display(),
                handle.ref_count()
            );
        }

        true
    }

    /// Called on MCP connection close to decrement the ref count for any path
    /// this consumer was watching, without needing to know the specific path.
    /// Uses the same `client:session_id` key as `unwatch` — no extra bookkeeping.
    pub fn unwatch_by_session(&self, client: Option<&str>, session_id: &str) {
        let key = Consumer::new(client, Some(session_id)).key();
        let mut inner = self.inner.lock().unwrap();
        let mut empty_roots = Vec::new();

        for (root, handle) in inner.projects.iter_mut() {
            if handle.consumers.remove(&key).is_some() {
                if handle.ref_count() == 0 {
                    empty_roots.push(root.clone());
                } else {
                    eprintln!(
                        "[watch] ref-1 for {} (connection closed: {key}, remaining: {})",
                        root.display(),
                        handle.ref_count()
                    );
                }
            }
        }

        for root in empty_roots {
            inner.projects.remove(&root);
            eprintln!(
                "[watch] stopped (connection closed: {key}): {}",
                root.display()
            );
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
                ref_count: handle.ref_count(),
                consumers: handle.consumers.values().cloned().collect(),
            })
            .collect()
    }

    fn start_watch_thread(&self, root: PathBuf, consumer: Consumer) -> Result<(), String> {
        let state = Arc::new(Mutex::new(WatchState::Indexing));
        let (stop_tx, stop_rx) = mpsc::channel::<StopSignal>();

        let thread = thread::Builder::new()
            .name(format!("watch:{}", root.display()))
            .spawn({
                let state = Arc::clone(&state);
                let root = root.clone();
                move || run_watch_thread(root, state, stop_rx)
            })
            .map_err(|e| format!("failed to spawn watch thread: {e}"))?;

        let mut consumers = HashMap::new();
        consumers.insert(consumer.key(), consumer);

        self.inner.lock().unwrap().projects.insert(
            root,
            WatchHandle {
                state,
                consumers,
                _stop_tx: stop_tx,
                _thread: thread,
            },
        );

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
    stop_rx: mpsc::Receiver<StopSignal>,
) {
    if let Err(e) = watch_loop(&project_root, &state, &stop_rx) {
        *state.lock().unwrap() = WatchState::Failed(e);
    }
}

fn watch_loop(
    project_root: &PathBuf,
    state: &Arc<Mutex<WatchState>>,
    stop_rx: &mpsc::Receiver<StopSignal>,
) -> Result<(), String> {
    eprintln!("[watch] initial index start: {}", project_root.display());
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
        // Check for stop signal (non-blocking).
        match stop_rx.try_recv() {
            Ok(_) | Err(mpsc::TryRecvError::Disconnected) => {
                eprintln!("[watch] stopping: {}", project_root.display());
                return Ok(());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

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
