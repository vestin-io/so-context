//! Shared small utilities: content hashing and path-skip predicate.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// Returns a hex string hash of the file content for change detection.
/// Uses SipHash (via DefaultHasher) — fast and sufficient for equality checks.
pub(super) fn content_hash(content: &str) -> String {
    let mut h = DefaultHasher::new();
    content.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Returns whether a path should be excluded from indexing.
/// todo - use .gitignore as resource, fixed list as fallback
pub(super) fn should_skip(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/.git/")
        || s.contains("/node_modules/")
        || s.contains("/target/")
        || s.contains("/.so-context/")
}
