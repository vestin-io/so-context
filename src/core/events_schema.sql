-- so-context events schema (SQLite)
-- Global database at ~/.local/share/so-context/events.db
-- Tracks every MCP tool call with token usage estimates for savings analysis.

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS events (
  id                       INTEGER PRIMARY KEY,
  event_id                 TEXT    NOT NULL,
  ts                       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  client                   TEXT,                     -- MCP client app name (client_info.name, e.g. "opencode")
  client_version           TEXT,                     -- MCP client app version (client_info.version, e.g. "1.15.12")
  client_source            TEXT    NOT NULL,         -- client_info | cli | fallback
  agent                    TEXT,                     -- sub-agent within client (e.g. "explorer", "build"); NULL if not known
  session_id               TEXT    NOT NULL,
  session_source           TEXT    NOT NULL,         -- hook | connection | generated | fallback
  project                  TEXT,                     -- absolute project root, nullable
  tool                     TEXT    NOT NULL,          -- so_read / so_search / so_shell / so_shell_output / ...
  params                   TEXT,                     -- JSON blob of tool arguments
  result_ok                INTEGER NOT NULL DEFAULT 1, -- 1 = success, 0 = error
  duration_ms              INTEGER,                  -- wall-clock time of tool call
  estimated_origin_tokens  INTEGER,                  -- tokens agent would have used without so-context
  actual_tokens            INTEGER,                  -- tokens actually consumed by tool result
  estimated_origin_size    INTEGER,                  -- bytes of origin content (before so-context compression)
  actual_size              INTEGER                   -- bytes of actual tool result
);

CREATE INDEX IF NOT EXISTS idx_events_agent     ON events(agent, session_id);
CREATE INDEX IF NOT EXISTS idx_events_tool_ts   ON events(tool, ts);
CREATE INDEX IF NOT EXISTS idx_events_project   ON events(project) WHERE project IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_project_ts ON events(project, ts) WHERE project IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_ts        ON events(ts);
