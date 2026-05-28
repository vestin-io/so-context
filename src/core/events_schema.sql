-- so-context events schema (SQLite)
-- Global database at ~/.local/share/so-context/events.db
-- Tracks every MCP tool call with token usage estimates for savings analysis.

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS events (
  id                       INTEGER PRIMARY KEY,
  ts                       TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  client                   TEXT,                     -- MCP client_info.name
  client_version           TEXT,                     -- MCP client_info.version
  agent                    TEXT    NOT NULL,
  agent_version            TEXT,                     -- MCP client version (e.g. 0.134.0)
  agent_source             TEXT    NOT NULL,         -- forwarded | hook | client_info | env | fallback
  session_id               TEXT    NOT NULL,
  session_source           TEXT    NOT NULL,         -- forwarded | hook | client_info | env | fallback
  project                  TEXT,                     -- absolute project root, nullable
  tool                     TEXT    NOT NULL,          -- so_read / so_search / so_watch / ...
  params                   TEXT,                     -- JSON blob of tool arguments
  result_ok                INTEGER NOT NULL DEFAULT 1, -- 1 = success, 0 = error
  duration_ms              INTEGER,                  -- wall-clock time of tool call
  estimated_origin_tokens  INTEGER,                  -- tokens agent would have used without so-context
  actual_tokens            INTEGER                   -- tokens actually consumed by tool result
);

CREATE INDEX IF NOT EXISTS idx_events_agent     ON events(agent, session_id);
CREATE INDEX IF NOT EXISTS idx_events_tool_ts   ON events(tool, ts);
CREATE INDEX IF NOT EXISTS idx_events_project   ON events(project) WHERE project IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_ts        ON events(ts);
