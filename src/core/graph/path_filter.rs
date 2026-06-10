use std::path::Path;

use ignore::gitignore::GitignoreBuilder;

/// Returns whether a path should be excluded from indexing.
///
/// Gitignore-based filtering is handled by [`ignore::Walk`] at the walker level.
/// This predicate catches internal directories that gitignore rules don't cover.
pub(super) fn should_skip(path: &Path) -> bool {
    let path_text = path.to_string_lossy();
    path_text.contains("/.so-context/")
}

/// Gitignore-aware path filter for use outside of `ignore::Walk` (e.g. watchers).
pub(crate) struct GitIgnoreFilter {
    inner: ignore::gitignore::Gitignore,
}

impl GitIgnoreFilter {
    /// Builds a matcher from all `.gitignore` files under `root`.
    pub(crate) fn new(root: &Path) -> Result<Self, String> {
        let mut builder = GitignoreBuilder::new(root);

        for entry in ignore::Walk::new(root).filter_map(|entry| entry.ok()) {
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
            .map_err(|error| format!("failed to build gitignore matcher: {error}"))?;
        Ok(Self { inner })
    }

    /// Returns `true` when `path` is matched by a gitignore rule.
    pub(crate) fn is_ignored(&self, path: &Path) -> bool {
        self.inner.matched(path, false).is_ignore()
    }
}
