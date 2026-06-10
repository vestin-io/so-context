use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Returns a hex string hash of the file content for change detection.
/// Uses SipHash (via DefaultHasher) which is fast and sufficient for equality checks.
pub(super) fn content_hash(content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
