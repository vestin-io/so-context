//! Shared small utilities: content hashing, path-skip predicate, and
//! gitignore-based path filtering.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

use ignore::gitignore::GitignoreBuilder;

/// Returns a hex string hash of the file content for change detection.
/// Uses SipHash (via DefaultHasher) — fast and sufficient for equality checks.
pub(super) fn content_hash(content: &str) -> String {
    let mut h = DefaultHasher::new();
    content.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Returns whether a path should be excluded from indexing.
///
/// Gitignore-based filtering is handled by [`ignore::Walk`] at the walker level.
/// This predicate catches internal directories that gitignore rules don't cover.
pub(super) fn should_skip(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/.so-context/")
}

/// Gitignore-aware path filter for use outside of `ignore::Walk` (e.g. watchers).
pub(crate) struct GitIgnoreFilter {
    inner: ignore::gitignore::Gitignore,
}

impl GitIgnoreFilter {
    /// Builds a matcher from all `.gitignore` files under `root`.
    pub(crate) fn new(root: &Path) -> Result<Self, String> {
        let mut builder = GitignoreBuilder::new(root);

        for entry in ignore::Walk::new(root).filter_map(|e| e.ok()) {
            if entry.file_name() == ".gitignore" {
                let _ = builder.add(entry.path());
            }
        }

        let exclude = root.join(".git/info/exclude");
        if exclude.exists() {
            let _ = builder.add(&exclude);
        }

        let inner = builder
            .build()
            .map_err(|e| format!("failed to build gitignore matcher: {e}"))?;
        Ok(Self { inner })
    }

    /// Returns `true` when `path` is matched by a gitignore rule.
    pub(crate) fn is_ignored(&self, path: &Path) -> bool {
        self.inner.matched(path, false).is_ignore()
    }
}
