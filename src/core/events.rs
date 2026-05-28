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
//!                       actual    = tokenizer count of result text
//! - `so_read outline` : estimated = tokenizer count of full file content
//!                       actual    = tokenizer count of outline result
//! - `so_read graph`   : estimated = tokenizer count of full file content
//!                       actual    = tokenizer count of graph result
//! - `so_read full`    : estimated = actual (no saving)
//! - `so_status`       : estimated = 0, actual = tokenizer count of result text

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use rusqlite::{Connection, params};

const EVENT_BATCH_SIZE: usize = 64;
const EVENT_FLUSH_INTERVAL: Duration = Duration::from_millis(100);

static EVENT_TX: OnceLock<Sender<EventRecord>> = OnceLock::new();

/// Returns the path to the global events database.
pub fn events_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("so-context")
        .join("events.db")
}

fn open_db() -> Result<Connection, String> {
    let path = events_db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create events dir: {e}"))?;
    }
    let conn = Connection::open(&path).map_err(|e| format!("open events db: {e}"))?;
    conn.execute_batch(include_str!("events_schema.sql"))
        .map_err(|e| format!("init events schema: {e}"))?;
    Ok(conn)
}

#[derive(Default, Clone)]
pub struct EventRecord {
    pub client:                  Option<String>,
    pub client_version:          Option<String>,
    pub client_source:           String,
    pub agent:                   Option<String>,
    pub session_id:              String,
    pub session_source:          String,
    pub project:                 Option<String>,
    pub tool:                    String,
    pub params:                  Option<String>,
    pub result_ok:               bool,
    pub duration_ms:             Option<i64>,
    pub estimated_origin_tokens: Option<i64>,
    pub actual_tokens:           Option<i64>,
}

impl EventRecord {
    pub fn new(session_id: &str, tool: &str) -> Self {
        Self {
            client_source:  "fallback".to_string(),
            session_id:     session_id.to_string(),
            session_source: "fallback".to_string(),
            tool:           tool.to_string(),
            result_ok:      true,
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
        if let Err(e) = tx.execute(
            "INSERT INTO events(
                client, client_version, client_source,
                agent,
                session_id, session_source, project, tool, params,
                result_ok, duration_ms,
                estimated_origin_tokens, actual_tokens
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                ev.client,
                ev.client_version,
                ev.client_source,
                ev.agent,
                ev.session_id,
                ev.session_source,
                ev.project,
                ev.tool,
                ev.params,
                ev.result_ok as i32,
                ev.duration_ms,
                ev.estimated_origin_tokens,
                ev.actual_tokens,
            ],
        ) {
            eprintln!("so-context events: insert failed: {e}");
        }
    }

    if let Err(e) = tx.commit() {
        eprintln!("so-context events: commit failed: {e}");
    }
    batch.clear();
}

/// Convenience timer — wrap around a tool call to auto-measure duration.
pub struct Timer(Instant);

impl Timer {
    pub fn start() -> Self { Self(Instant::now()) }
    pub fn elapsed_ms(&self) -> i64 { self.0.elapsed().as_millis() as i64 }
}

pub struct EventRow {
    pub id:                      i64,
    pub ts:                      String,
    pub client:                  Option<String>,
    pub client_version:          Option<String>,
    pub client_source:           String,
    pub agent:                   Option<String>,
    pub session_id:              String,
    pub session_source:          String,
    pub project:                 Option<String>,
    pub tool:                    String,
    pub params:                  Option<String>,
    pub result_ok:               bool,
    pub duration_ms:             Option<i64>,
    pub estimated_origin_tokens: Option<i64>,
    pub actual_tokens:           Option<i64>,
}

impl EventRow {
    pub fn tokens_saved(&self) -> Option<i64> {
        match (self.estimated_origin_tokens, self.actual_tokens) {
            (Some(e), Some(a)) => Some(e - a),
            _ => None,
        }
    }
}

pub struct EventQuery {
    pub client:     Option<String>,
    pub session_id: Option<String>,
    pub tool:       Option<String>,
    pub project:    Option<String>,
    pub limit:      usize,
}

impl Default for EventQuery {
    fn default() -> Self {
        Self { client: None, session_id: None, tool: None, project: None, limit: 50 }
    }
}

pub fn query_events(q: &EventQuery) -> Result<Vec<EventRow>, String> {
    let conn = open_db()?;

    let mut conditions: Vec<String> = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(a) = &q.client {
        conditions.push(format!("client = ?{}", values.len() + 1));
        values.push(Box::new(a.clone()));
    }
    if let Some(s) = &q.session_id {
        conditions.push(format!("session_id = ?{}", values.len() + 1));
        values.push(Box::new(s.clone()));
    }
    if let Some(t) = &q.tool {
        conditions.push(format!("tool = ?{}", values.len() + 1));
        values.push(Box::new(t.clone()));
    }
    if let Some(p) = &q.project {
        conditions.push(format!("project = ?{}", values.len() + 1));
        values.push(Box::new(p.clone()));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    values.push(Box::new(q.limit as i64));
    let limit_param = values.len();

    let sql = format!(
        "SELECT id, ts, client, client_version, client_source,
                agent, session_id, session_source, project, tool, params,
                result_ok, duration_ms, estimated_origin_tokens, actual_tokens
         FROM events
         {where_clause}
         ORDER BY ts DESC
         LIMIT ?{limit_param}"
    );

    let refs: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();

    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let rows = stmt
        .query_map(refs.as_slice(), |row| {
            Ok(EventRow {
                id:                      row.get(0)?,
                ts:                      row.get(1)?,
                client:                  row.get(2)?,
                client_version:          row.get(3)?,
                client_source:           row.get(4)?,
                agent:                   row.get(5)?,
                session_id:              row.get(6)?,
                session_source:          row.get(7)?,
                project:                 row.get(8)?,
                tool:                    row.get(9)?,
                params:                  row.get(10)?,
                result_ok:               row.get::<_, i32>(11)? != 0,
                duration_ms:             row.get(12)?,
                estimated_origin_tokens: row.get(13)?,
                actual_tokens:           row.get(14)?,
            })
        })
        .map_err(|e| format!("query: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("row: {e}"))?);
    }
    Ok(out)
}

pub fn query_stats(client: Option<&str>, session_id: Option<&str>) -> Result<String, String> {
    let conn = open_db()?;

    let mut conditions = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(a) = client {
        conditions.push(format!("client = ?{}", vals.len() + 1));
        vals.push(Box::new(a.to_string()));
    }
    if let Some(s) = session_id {
        conditions.push(format!("session_id = ?{}", vals.len() + 1));
        vals.push(Box::new(s.to_string()));
    }
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT tool,
                COUNT(*) as calls,
                SUM(COALESCE(estimated_origin_tokens, 0) - COALESCE(actual_tokens, 0)) as saved,
                SUM(COALESCE(actual_tokens, 0)) as used
         FROM events {where_clause}
         GROUP BY tool
         ORDER BY saved DESC"
    );

    let refs: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|v| v.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| format!("prepare stats: {e}"))?;

    let rows = stmt
        .query_map(refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| format!("query stats: {e}"))?;

    let mut lines = vec!["tool            calls   tokens_saved   tokens_used".to_string()];
    let mut total_saved = 0i64;
    let mut total_used = 0i64;
    let mut total_calls = 0i64;

    for row in rows {
        let (tool, calls, saved, used) = row.map_err(|e| format!("row: {e}"))?;
        lines.push(format!("{tool:<16} {calls:>5}   {saved:>12}   {used:>11}"));
        total_calls += calls;
        total_saved += saved;
        total_used += used;
    }

    lines.push(format!("{:<16} {:>5}   {:>12}   {:>11}", "TOTAL", total_calls, total_saved, total_used));
    Ok(lines.join("\n"))
}
