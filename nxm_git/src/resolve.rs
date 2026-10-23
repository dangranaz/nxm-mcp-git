use anyhow::{Context, Result};
use git2::Repository;
use std::path::{Path, PathBuf};

/// A safely resolved Git repository, guaranteed to be inside allowed roots.
pub struct ResolvedRepo {
    /// The opened Git repository.
    pub inner: Repository,
    /// The canonicalized absolute path to the repository worktree.
    pub path: PathBuf,
}

impl ResolvedRepo {
    /// Open a repository at `repo_path`, ensuring it lies within `allowed_roots`.
    ///
    /// The path is canonicalized and checked for `..` escapes (via canonicalization).
    /// Returns an error if the path is outside allowed roots or not a valid Git repo.
    pub fn open<P: AsRef<Path>>(repo_path: P, allowed_roots: &[PathBuf]) -> Result<Self> {
        // 1. Make the path absolute and canonicalized (removes `..`, symlinks, etc.)
        let abs_path = std::fs::canonicalize(repo_path.as_ref())
            .with_context(|| format!("cannot access path: {}", repo_path.as_ref().display()))?;
        // Ensure it's a directory (git repo worktree)
        if !abs_path.is_dir() {
            anyhow::bail!("not a directory: {}", abs_path.display());
        }

        // 2. Ensure the path is inside one of the allowed roots.
        let mut inside = false;
        for root in allowed_roots {
            // Normalize root to be canonicalized as well for fair comparison.
            let Ok(normalized_root) = std::fs::canonicalize(root) else {
                // If an allowed root is inaccessible, treat it as not matching.
                continue;
            };
            if abs_path.starts_with(&normalized_root) {
                inside = true;
                break;
            }
        }
        if !inside {
            anyhow::bail!(
                "repository path {} is outside allowed roots",
                abs_path.display()
            );
        }

        // 3. Open the Git repository.
        let repo = Repository::open(&abs_path)
            .with_context(|| format!("opening Git repository at {}", abs_path.display()))?;

        Ok(Self {
            inner: repo,
            path: abs_path,
        })
    }
}
