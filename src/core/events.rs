//! Global event log for tracking MCP tool usage and token savings.
//!
//! Stores one row per tool call in a global SQLite database at
//! `~/.local/share/so-context/events.db`.
//!
//! Writes are offloaded to a background writer thread through a global event
//! queue so tool responses never block on SQLite. The writer batches events and
//! flushes them in a single transaction.
//!
//! Token saving estimate per call:
//!   `tokens_saved = estimated_origin_tokens - actual_tokens`
//!
//! How each tool populates the token fields:
//! - `so_search`       : estimated = sum of matched files' stored token counts
//!   actual            = tokenizer count of result text
//! - `so_read outline` : estimated = tokenizer count of full file content
//!   actual            = tokenizer count of outline result
//! - `so_read full`    : estimated = actual (no saving)
//! - `so_status`       : estimated = 0
//!   actual            = tokenizer count of result text
//! - `so_shell` / `so_shell_output`
//!   estimated         = tokenizer count of full raw output
//!   actual            = tokenizer count of displayed output

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::{Connection, ErrorCode, params};
use serde::{Deserialize, Serialize};

const EVENT_BATCH_SIZE: usize = 64;
const EVENT_FLUSH_INTERVAL: Duration = Duration::from_millis(100);
const EVENT_BUSY_TIMEOUT_MS: u64 = 1_000;
const CLI_PERSIST_RETRY_DELAYS_MS: &[u64] = &[50, 100, 250];
static EVENT_TX: OnceLock<Sender<EventRecord>> = OnceLock::new();

/// Returns the path to the global events database.
fn events_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("so-context")
        .join("events.db")
}

pub(crate) fn open_db() -> Result<Connection, String> {
    let path = events_db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create events dir: {e}"))?;
    }
    let conn = Connection::open(&path).map_err(|e| format!("open events db: {e}"))?;
    conn.execute_batch(&format!(
        "PRAGMA journal_mode = WAL;\nPRAGMA busy_timeout = {EVENT_BUSY_TIMEOUT_MS};"
    ))
    .map_err(|e| format!("configure events db pragmas: {e}"))?;
    conn.execute_batch(include_str!("events_schema.sql"))
        .map_err(|e| format!("init events schema: {e}"))?;
    // Migrations: add columns introduced after initial schema.
    // ALTER TABLE fails with "duplicate column" if already present — safe to ignore.
    for sql in [
        "ALTER TABLE events ADD COLUMN event_id TEXT",
        "ALTER TABLE events ADD COLUMN estimated_origin_size INTEGER",
        "ALTER TABLE events ADD COLUMN actual_size INTEGER",
    ] {
        let _ = conn.execute_batch(sql);
    }
    let _ = conn.execute_batch(
        "UPDATE events SET event_id = lower(hex(randomblob(16))) WHERE event_id IS NULL OR event_id = '';",
    );
    let _ = conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_events_event_id ON events(event_id);",
    );
    Ok(conn)
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct EventRecord {
    pub event_id: String,
    pub client: Option<String>,
    pub client_version: Option<String>,
    pub client_source: String,
    pub agent: Option<String>,
    pub session_id: String,
    pub session_source: String,
    pub project: Option<String>,
    pub tool: String,
    pub params: Option<String>,
    pub result_ok: bool,
    pub duration_ms: Option<i64>,
    pub estimated_origin_tokens: Option<i64>,
    pub actual_tokens: Option<i64>,
    pub estimated_origin_size: Option<i64>,
    pub actual_size: Option<i64>,
}

impl EventRecord {
    pub fn new(session_id: &str, tool: &str) -> Self {
        Self {
            event_id: uuid::Uuid::new_v4().to_string(),
            client_source: "fallback".to_string(),
            session_id: session_id.to_string(),
            session_source: "fallback".to_string(),
            tool: tool.to_string(),
            result_ok: true,
            ..Default::default()
        }
    }
}

/// Enqueues an event for asynchronous batch insertion. Never blocks on SQLite.
/// If the queue is unavailable, the event is dropped silently.
pub fn enqueue(event: EventRecord) {
    let tx = EVENT_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<EventRecord>();
        thread::Builder::new()
            .name("so-context-events".to_string())
            .spawn(move || run_writer(rx))
            .expect("spawn so-context event writer thread");
        tx
    });
    let _ = tx.send(event);
}

/// Inserts a single event synchronously.
///
/// Use this for short-lived CLI commands that may exit before the background
/// writer thread has a chance to flush queued events.
pub fn persist_now(event: &EventRecord) -> Result<(), String> {
    let conn = open_db()?;
    persist_with_retry(&conn, event).map_err(|e| format!("insert event: {e}"))
}

fn run_writer(rx: Receiver<EventRecord>) {
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("so-context events: {e}");
            return;
        }
    };

    let mut batch = Vec::with_capacity(EVENT_BATCH_SIZE);
    loop {
        match rx.recv_timeout(EVENT_FLUSH_INTERVAL) {
            Ok(ev) => {
                batch.push(ev);
                if batch.len() >= EVENT_BATCH_SIZE {
                    flush_batch(&conn, &mut batch);
                } else {
                    drain_ready(&rx, &mut batch);
                    if batch.len() >= EVENT_BATCH_SIZE {
                        flush_batch(&conn, &mut batch);
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if !batch.is_empty() {
                    flush_batch(&conn, &mut batch);
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                if !batch.is_empty() {
                    flush_batch(&conn, &mut batch);
                }
                break;
            }
        }
    }
}

fn drain_ready(rx: &Receiver<EventRecord>, batch: &mut Vec<EventRecord>) {
    while batch.len() < EVENT_BATCH_SIZE {
        match rx.try_recv() {
            Ok(ev) => batch.push(ev),
            Err(_) => break,
        }
    }
}

fn flush_batch(conn: &Connection, batch: &mut Vec<EventRecord>) {
    if batch.is_empty() {
        return;
    }

    let tx = match conn.unchecked_transaction() {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("so-context events: begin tx failed: {e}");
            batch.clear();
            return;
        }
    };

    for ev in batch.iter() {
        if let Err(e) = insert_event(&tx, ev) {
            eprintln!("so-context events: insert failed: {e}");
        }
    }

    if let Err(e) = tx.commit() {
        eprintln!("so-context events: commit failed: {e}");
    }
    batch.clear();
}

fn insert_event(conn: &Connection, event: &EventRecord) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT OR IGNORE INTO events(
            event_id,
            client, client_version, client_source,
            agent,
            session_id, session_source, project, tool, params,
            result_ok, duration_ms,
            estimated_origin_tokens, actual_tokens,
            estimated_origin_size, actual_size
         ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![
            event.event_id,
            event.client,
            event.client_version,
            event.client_source,
            event.agent,
            event.session_id,
            event.session_source,
            event.project,
            event.tool,
            event.params,
            event.result_ok as i32,
            event.duration_ms,
            event.estimated_origin_tokens,
            event.actual_tokens,
            event.estimated_origin_size,
            event.actual_size,
        ],
    )
}

fn persist_with_retry(conn: &Connection, event: &EventRecord) -> rusqlite::Result<()> {
    for (attempt, delay_ms) in CLI_PERSIST_RETRY_DELAYS_MS.iter().enumerate() {
        match insert_event(conn, event) {
            Ok(_) => return Ok(()),
            Err(error) if is_retryable_sqlite_error(&error) => {
                thread::sleep(Duration::from_millis(*delay_ms));
                if attempt + 1 == CLI_PERSIST_RETRY_DELAYS_MS.len() {
                    return insert_event(conn, event).map(|_| ());
                }
            }
            Err(error) => return Err(error),
        }
    }

    insert_event(conn, event).map(|_| ())
}

fn is_retryable_sqlite_error(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(sqlite_error, _)
            if matches!(sqlite_error.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

/// Convenience timer — wrap around a tool call to auto-measure duration.
pub struct Timer(Instant);

impl Timer {
    pub fn start() -> Self {
        Self(Instant::now())
    }
    pub fn elapsed_ms(&self) -> i64 {
        self.0.elapsed().as_millis() as i64
    }
}
