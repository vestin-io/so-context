//! In-memory file-visit cache.
//!
//! Tracks which files have been read within a given MCP connection + context
//! window, along with their token count and last-visit timestamp.
//!
//! # Key structure
//!
//! The cache is a two-level map:
//!
//! ```text
//! connection_id  →  cw_id  →  file_path  →  FileEntry
//! ```
//!
//! # Thread safety
//!
//! `FileVisitCache` wraps its state in `Arc<Mutex<…>>` and is cheap to clone.
//! All public methods acquire the lock internally, so callers never touch the
//! mutex directly.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single file visit record stored in the cache.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Absolute (or normalised) path to the file.
    pub file_path: String,
    /// Token count of the file content at the time it was added/updated.
    pub token_count: i64,
    /// Wall-clock time of the most recent visit.
    pub last_visit: Instant,
    /// SHA-256 hex digest of the file content at the time of the last read.
    /// Used to detect whether the file has been modified since it was cached.
    pub content_hash: String,
}

// ---------------------------------------------------------------------------
// Hash helper
// ---------------------------------------------------------------------------

/// Compute a lightweight content hash (SHA-256 hex) for change detection.
pub fn hash_content(content: &str) -> String {
    // FNV-1a 64-bit hash — fast, no extra deps, sufficient for change detection.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in content.as_bytes() {
        h ^= *byte as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// In-memory store for file-visit state, keyed by `(connection_id, cw_id)`.
///
/// Clone is cheap — all clones share the same inner state.
#[derive(Clone, Default)]
pub struct FileVisitCache {
    /// `connection_id` → `cw_id` → `file_path` → `FileEntry`
    inner: Arc<Mutex<CacheInner>>,
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

#[derive(Default)]
struct CacheInner {
    data: HashMap<String, HashMap<String, HashMap<String, FileEntry>>>,
}

// ---------------------------------------------------------------------------
// FileVisitCache implementation
// ---------------------------------------------------------------------------

impl FileVisitCache {
    /// Create a new, empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    // -----------------------------------------------------------------------
    // Write operations
    // -----------------------------------------------------------------------

    /// Insert or update a file entry for the given connection + context window.
    ///
    /// If the file was already present its `token_count`, `content_hash`, and
    /// `last_visit` are refreshed.
    pub fn add_file(
        &self,
        connection_id: &str,
        cw_id: &str,
        file_path: &str,
        token_count: i64,
        content_hash: String,
    ) {
        let mut guard = self.inner.lock().unwrap();
        guard
            .data
            .entry(connection_id.to_string())
            .or_default()
            .entry(cw_id.to_string())
            .or_default()
            .insert(
                file_path.to_string(),
                FileEntry {
                    file_path: file_path.to_string(),
                    token_count,
                    last_visit: Instant::now(),
                    content_hash,
                },
            );
    }

    /// Remove a single file entry from the given connection + context window.
    ///
    /// Returns `true` if the entry existed and was removed, `false` otherwise.
    pub fn expire_file(
        &self,
        connection_id: &str,
        cw_id: &str,
        file_path: &str,
    ) -> bool {
        let mut guard = self.inner.lock().unwrap();
        guard
            .data
            .get_mut(connection_id)
            .and_then(|cw_map| cw_map.get_mut(cw_id))
            .and_then(|file_map| file_map.remove(file_path))
            .is_some()
    }

    /// Remove all file entries for a specific context window within a connection.
    ///
    /// Returns `true` if the context window existed and was removed.
    pub fn delete_context_window(
        &self,
        connection_id: &str,
        cw_id: &str,
    ) -> bool {
        let mut guard = self.inner.lock().unwrap();
        guard
            .data
            .get_mut(connection_id)
            .and_then(|cw_map| cw_map.remove(cw_id))
            .is_some()
    }

    /// Remove all context windows and file entries associated with a connection.
    ///
    /// Returns `true` if the connection existed and was removed.
    pub fn delete_connection(&self, connection_id: &str) -> bool {
        let mut guard = self.inner.lock().unwrap();
        guard.data.remove(connection_id).is_some()
    }

    // -----------------------------------------------------------------------
    // Read operations
    // -----------------------------------------------------------------------

    /// Return `true` if the file has been visited within this connection +
    /// context window.
    pub fn is_visited(
        &self,
        connection_id: &str,
        cw_id: &str,
        file_path: &str,
    ) -> bool {
        let guard = self.inner.lock().unwrap();
        guard
            .data
            .get(connection_id)
            .and_then(|cw_map| cw_map.get(cw_id))
            .map(|file_map| file_map.contains_key(file_path))
            .unwrap_or(false)
    }

    /// Return the `FileEntry` for the given file, if it exists.
    pub fn get_file(
        &self,
        connection_id: &str,
        cw_id: &str,
        file_path: &str,
    ) -> Option<FileEntry> {
        let guard = self.inner.lock().unwrap();
        guard
            .data
            .get(connection_id)
            .and_then(|cw_map| cw_map.get(cw_id))
            .and_then(|file_map| file_map.get(file_path))
            .cloned()
    }

    /// Return all file entries for the given connection + context window.
    ///
    /// Returns an empty `Vec` if the key pair does not exist.
    pub fn list_files(
        &self,
        connection_id: &str,
        cw_id: &str,
    ) -> Vec<FileEntry> {
        let guard = self.inner.lock().unwrap();
        guard
            .data
            .get(connection_id)
            .and_then(|cw_map| cw_map.get(cw_id))
            .map(|file_map| file_map.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Return all context window IDs registered for a connection.
    pub fn list_context_windows(&self, connection_id: &str) -> Vec<String> {
        let guard = self.inner.lock().unwrap();
        guard
            .data
            .get(connection_id)
            .map(|cw_map| cw_map.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Return the total number of unique (connection, cw) pairs.
    pub fn len(&self) -> usize {
        let guard = self.inner.lock().unwrap();
        guard
            .data
            .values()
            .map(|cw_map| cw_map.len())
            .sum()
    }

    /// Return `true` when the cache contains no entries at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cache() -> FileVisitCache {
        FileVisitCache::new()
    }

    #[test]
    fn add_and_query_file() {
        let cache = make_cache();
        cache.add_file("conn-1", "cw-1", "/src/main.rs", 120, "hash1".to_string());
        assert!(cache.is_visited("conn-1", "cw-1", "/src/main.rs"));
        assert!(!cache.is_visited("conn-1", "cw-1", "/src/lib.rs"));
    }

    #[test]
    fn add_updates_token_count_and_last_visit() {
        let cache = make_cache();
        cache.add_file("conn-1", "cw-1", "/src/main.rs", 100, "hash-a".to_string());
        let first = cache.get_file("conn-1", "cw-1", "/src/main.rs").unwrap();

        // Re-add with a different token count.
        cache.add_file("conn-1", "cw-1", "/src/main.rs", 200, "hash-b".to_string());
        let second = cache.get_file("conn-1", "cw-1", "/src/main.rs").unwrap();

        assert_eq!(second.token_count, 200);
        assert_eq!(second.content_hash, "hash-b");
        assert!(second.last_visit >= first.last_visit);
    }

    #[test]
    fn expire_file() {
        let cache = make_cache();
        cache.add_file("conn-1", "cw-1", "/src/main.rs", 120, "h".to_string());
        assert!(cache.expire_file("conn-1", "cw-1", "/src/main.rs"));
        assert!(!cache.is_visited("conn-1", "cw-1", "/src/main.rs"));
        // Expiring again returns false.
        assert!(!cache.expire_file("conn-1", "cw-1", "/src/main.rs"));
    }

    #[test]
    fn delete_context_window() {
        let cache = make_cache();
        cache.add_file("conn-1", "cw-1", "/src/a.rs", 10, "h1".to_string());
        cache.add_file("conn-1", "cw-1", "/src/b.rs", 20, "h2".to_string());
        cache.add_file("conn-1", "cw-2", "/src/c.rs", 30, "h3".to_string());

        assert!(cache.delete_context_window("conn-1", "cw-1"));
        assert!(!cache.is_visited("conn-1", "cw-1", "/src/a.rs"));
        assert!(!cache.is_visited("conn-1", "cw-1", "/src/b.rs"));
        // cw-2 must be unaffected.
        assert!(cache.is_visited("conn-1", "cw-2", "/src/c.rs"));
    }

    #[test]
    fn delete_connection() {
        let cache = make_cache();
        cache.add_file("conn-1", "cw-1", "/src/a.rs", 10, "h1".to_string());
        cache.add_file("conn-1", "cw-2", "/src/b.rs", 20, "h2".to_string());
        cache.add_file("conn-2", "cw-1", "/src/c.rs", 30, "h3".to_string());

        assert!(cache.delete_connection("conn-1"));
        assert!(!cache.is_visited("conn-1", "cw-1", "/src/a.rs"));
        assert!(!cache.is_visited("conn-1", "cw-2", "/src/b.rs"));
        // conn-2 must be unaffected.
        assert!(cache.is_visited("conn-2", "cw-1", "/src/c.rs"));
    }

    #[test]
    fn list_files() {
        let cache = make_cache();
        cache.add_file("conn-1", "cw-1", "/a.rs", 1, "h1".to_string());
        cache.add_file("conn-1", "cw-1", "/b.rs", 2, "h2".to_string());

        let mut files: Vec<String> = cache
            .list_files("conn-1", "cw-1")
            .into_iter()
            .map(|e| e.file_path)
            .collect();
        files.sort();
        assert_eq!(files, vec!["/a.rs", "/b.rs"]);
    }

    #[test]
    fn list_context_windows() {
        let cache = make_cache();
        cache.add_file("conn-1", "cw-a", "/a.rs", 1, "h1".to_string());
        cache.add_file("conn-1", "cw-b", "/b.rs", 2, "h2".to_string());

        let mut windows = cache.list_context_windows("conn-1");
        windows.sort();
        assert_eq!(windows, vec!["cw-a", "cw-b"]);
    }

    #[test]
    fn len_and_is_empty() {
        let cache = make_cache();
        assert!(cache.is_empty());

        cache.add_file("conn-1", "cw-1", "/a.rs", 1, "h1".to_string());
        cache.add_file("conn-1", "cw-2", "/b.rs", 2, "h2".to_string());
        assert_eq!(cache.len(), 2); // 2 (conn, cw) pairs

        cache.delete_connection("conn-1");
        assert!(cache.is_empty());
    }
}
