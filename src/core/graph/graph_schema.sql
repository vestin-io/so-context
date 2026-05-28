-- so-context code graph schema (SQLite + FTS5)
-- Design goals:
-- 1) Graph-first model: nodes + edges
-- 2) Fast symbol/text lookup: FTS5
-- 3) Incremental indexing support: files table with hash/mtime

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projects (
  id INTEGER PRIMARY KEY,
  root_path TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS files (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL,
  path TEXT NOT NULL,                         -- project-relative path
  language TEXT NOT NULL,
  content_hash TEXT,                          -- optional fast-change detection
  mtime_unix INTEGER,                         -- optional fs-based invalidation
  size_bytes INTEGER,
  token_count INTEGER,                        -- canonical tokenizer count of full file content
  indexed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(project_id, path),
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);

-- Symbol/entity nodes
CREATE TABLE IF NOT EXISTS nodes (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL,
  file_id INTEGER NOT NULL,
  kind TEXT NOT NULL,                         -- function/class/method/interface/...
  name TEXT NOT NULL,
  fq_name TEXT,                               -- optional fully-qualified symbol name
  signature TEXT,
  visibility TEXT,                            -- public/private/protected/internal/...
  is_async INTEGER,                           -- 0/1
  is_static INTEGER,                          -- 0/1
  is_abstract INTEGER,                        -- 0/1
  start_line INTEGER NOT NULL,
  start_col INTEGER NOT NULL,
  end_line INTEGER,
  end_col INTEGER,
  doc TEXT,
  metadata_json TEXT,                         -- extensible parser-specific metadata
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(file_id) REFERENCES files(id) ON DELETE CASCADE
);

-- Directed typed relations between nodes
CREATE TABLE IF NOT EXISTS edges (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL,
  from_node_id INTEGER NOT NULL,
  to_node_id INTEGER NOT NULL,
  kind TEXT NOT NULL,                         -- calls/imports/extends/references/...
  line INTEGER,                               -- optional location of relation
  col INTEGER,
  metadata_json TEXT,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(from_node_id) REFERENCES nodes(id) ON DELETE CASCADE,
  FOREIGN KEY(to_node_id) REFERENCES nodes(id) ON DELETE CASCADE
);

-- Optional unresolved references before resolution
CREATE TABLE IF NOT EXISTS unresolved_refs (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL,
  file_id INTEGER NOT NULL,
  ref_kind TEXT NOT NULL,                     -- import/call/type/ref
  ref_text TEXT NOT NULL,
  line INTEGER,
  col INTEGER,
  metadata_json TEXT,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  FOREIGN KEY(file_id) REFERENCES files(id) ON DELETE CASCADE
);

-- FTS index for node search.
-- content='nodes' keeps source-of-truth in nodes table.
CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
  name,
  fq_name,
  signature,
  doc,
  path,
  tokenize='unicode61'
);

-- If you want synchronized content-backed FTS, replace with:
-- CREATE VIRTUAL TABLE nodes_fts USING fts5(
--   name, fq_name, signature, doc, path,
--   content='nodes', content_rowid='id', tokenize='unicode61'
-- );
-- and maintain path via a denormalized column on nodes.

-- Hot-path indexes
CREATE INDEX IF NOT EXISTS idx_files_project_path ON files(project_id, path);
CREATE INDEX IF NOT EXISTS idx_files_project_lang ON files(project_id, language);
CREATE INDEX IF NOT EXISTS idx_nodes_project_name ON nodes(project_id, name);
CREATE INDEX IF NOT EXISTS idx_nodes_project_kind ON nodes(project_id, kind);
CREATE INDEX IF NOT EXISTS idx_nodes_file ON nodes(file_id);
CREATE INDEX IF NOT EXISTS idx_nodes_fq_name ON nodes(project_id, fq_name);
CREATE INDEX IF NOT EXISTS idx_edges_project_kind ON edges(project_id, kind);
CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_node_id);
CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_node_id);
CREATE INDEX IF NOT EXISTS idx_unref_project_kind ON unresolved_refs(project_id, ref_kind);
