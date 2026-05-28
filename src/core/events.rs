//! Global event log for tracking MCP tool usage and token savings.
//!
//! Stores one row per tool call in a global SQLite database at
//! `~/.local/share/so-context/events.db`.
//!
//! Token saving estimate per call:
//!   `tokens_saved = estimated_origin_tokens - actual_tokens`
//!
//! How each tool populates the token fields:
//! - `so_search`       : estimated = sum of sizes of matched files / 4
//!                       actual    = result chars / 4
//! - `so_read outline` : estimated = full file content chars / 4
//!                       actual    = outline result chars / 4
//! - `so_read graph`   : estimated = full file content chars / 4
//!                       actual    = graph result chars / 4
//! - `so_read full`    : estimated = actual (no saving)
//! - `so_watch/unwatch`: estimated = 0, actual = result chars / 4

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use rusqlite::{Connection, params};

// ---------------------------------------------------------------------------
// Global singleton DB connection
// ---------------------------------------------------------------------------

static EVENTS_DB: OnceLock<Mutex<Connection>> = OnceLock::new();

/// Returns the path to the global events database.
pub fn events_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("so-context")
        .join("events.db")
}

/// Opens (or creates) the global events database and initialises the schema.
/// Idempotent — safe to call multiple times; only the first call does work.
fn open_db() -> &'static Mutex<Connection> {
    EVENTS_DB.get_or_init(|| {
        let path = events_db_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&path).expect("failed to open events.db");
        conn.execute_batch(include_str!("events_schema.sql"))
            .expect("failed to init events schema");
        Mutex::new(conn)
    })
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Chars-to-tokens approximation: 4 chars ≈ 1 token.
const CHARS_PER_TOKEN: usize = 4;

pub fn chars_to_tokens(chars: usize) -> i64 {
    (chars / CHARS_PER_TOKEN) as i64
}

/// Builder for a single event record. Fill fields then call [`EventRecord::insert`].
#[derive(Default)]
pub struct EventRecord {
    pub agent:                   String,
    pub session_id:              String,
    pub project:                 Option<String>,
    pub tool:                    String,
    pub params:                  Option<String>,   // JSON string
    pub result_ok:               bool,
    pub duration_ms:             Option<i64>,
    pub estimated_origin_tokens: Option<i64>,
    pub actual_tokens:           Option<i64>,
}

impl EventRecord {
    pub fn new(agent: &str, session_id: &str, tool: &str) -> Self {
        Self {
            agent:      agent.to_string(),
            session_id: session_id.to_string(),
            tool:       tool.to_string(),
            result_ok:  true,
            ..Default::default()
        }
    }

    /// Persists the event to the global events DB. Silently ignores errors so
    /// a logging failure never breaks tool execution.
    pub fn insert(self) {
        let db = open_db();
        let Ok(conn) = db.lock() else { return };
        let _ = conn.execute(
            "INSERT INTO events(
                agent, session_id, project, tool, params,
                result_ok, duration_ms,
                estimated_origin_tokens, actual_tokens
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                self.agent,
                self.session_id,
                self.project,
                self.tool,
                self.params,
                self.result_ok as i32,
                self.duration_ms,
                self.estimated_origin_tokens,
                self.actual_tokens,
            ],
        );
    }
}

/// Convenience timer — wrap around a tool call to auto-measure duration.
pub struct Timer(Instant);

impl Timer {
    pub fn start() -> Self { Self(Instant::now()) }
    pub fn elapsed_ms(&self) -> i64 { self.0.elapsed().as_millis() as i64 }
}

// ---------------------------------------------------------------------------
// Query helpers (used by so_events tool)
// ---------------------------------------------------------------------------

pub struct EventRow {
    pub id:                      i64,
    pub ts:                      String,
    pub agent:                   String,
    pub session_id:              String,
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
    pub agent:      Option<String>,
    pub session_id: Option<String>,
    pub tool:       Option<String>,
    pub project:    Option<String>,
    pub limit:      usize,
}

impl Default for EventQuery {
    fn default() -> Self {
        Self { agent: None, session_id: None, tool: None, project: None, limit: 50 }
    }
}

/// Queries the events log and returns matching rows ordered by most recent first.
pub fn query_events(q: &EventQuery) -> Result<Vec<EventRow>, String> {
    let db = open_db();
    let conn = db.lock().map_err(|e| format!("events db lock: {e}"))?;

    // Build WHERE clauses dynamically.
    let mut conditions: Vec<String> = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(a) = &q.agent {
        conditions.push(format!("agent = ?{}", values.len() + 1));
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
        "SELECT id, ts, agent, session_id, project, tool, params,
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
                agent:                   row.get(2)?,
                session_id:              row.get(3)?,
                project:                 row.get(4)?,
                tool:                    row.get(5)?,
                params:                  row.get(6)?,
                result_ok:               row.get::<_, i32>(7)? != 0,
                duration_ms:             row.get(8)?,
                estimated_origin_tokens: row.get(9)?,
                actual_tokens:           row.get(10)?,
            })
        })
        .map_err(|e| format!("query: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("row: {e}"))?);
    }
    Ok(out)
}

/// Returns aggregate stats: total calls, total tokens saved, per-tool breakdown.
pub fn query_stats(agent: Option<&str>, session_id: Option<&str>) -> Result<String, String> {
    let db = open_db();
    let conn = db.lock().map_err(|e| format!("events db lock: {e}"))?;

    let mut conditions = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(a) = agent {
        conditions.push(format!("agent = ?{}", vals.len() + 1));
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
