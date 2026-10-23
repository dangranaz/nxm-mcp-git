use anyhow::Result;
use git2::Status as GitStatus;
use serde::{Deserialize, Serialize};

/// The result of `git status`, split into staged, unstaged, and untracked paths.
///
/// Paths are relative to the repository root and encoded as UTF-8 strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// New, modified, deleted, renamed, or typechanged files in the index (staged).
    pub staged: Vec<String>,
    /// New, modified, deleted, renamed, or typechanged files in the working tree
    /// relative to the index (unstaged).
    pub unstaged: Vec<String>,
    /// Files that are not tracked by Git (neither in index nor worktree of an
    /// existing index entry) and not ignored.
    pub untracked: Vec<String>,
}

/// Compute the status of the repository.
///
/// Equivalent to `git status --porcelain` but returns structured data.
pub fn status(repo: &crate::resolve::ResolvedRepo) -> Result<Status> {
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();

    let statuses = repo.inner.statuses(None)?;
    for i in 0..statuses.len() {
        let entry = statuses.get(i).expect("index within bounds");
        let path_bytes = entry.path_bytes();
        // Convert to lossy UTF-8; if not UTF-8, we skip? Assume UTF-8.
        let path = String::from_utf8_lossy(path_bytes).into_owned();

        let flags = entry.status();
        let is_index = flags.intersects(
            GitStatus::INDEX_NEW
                | GitStatus::INDEX_MODIFIED
                | GitStatus::INDEX_DELETED
                | GitStatus::INDEX_RENAMED
                | GitStatus::INDEX_TYPECHANGE,
        );
        let is_wt = flags.intersects(
            GitStatus::WT_NEW
                | GitStatus::WT_MODIFIED
                | GitStatus::WT_DELETED
                | GitStatus::WT_RENAMED
                | GitStatus::WT_TYPECHANGE,
        );

        if is_index {
            staged.push(path.clone());
        }
        if is_wt {
            // If it's WT_NEW and not indexed, it's untracked.
            if flags.contains(GitStatus::WT_NEW) && !is_index {
                untracked.push(path.clone());
            } else {
                unstaged.push(path.clone());
            }
        }
    }

    Ok(Status {
        staged,
        unstaged,
        untracked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::Repository;
    use std::path::Path;

    fn init_repo() -> (tempfile::TempDir, Repository) {
        let temp_dir = tempfile::TempDir::new().expect("failed to create temp dir");
        let repo_path = temp_dir.path();
        let repo = Repository::init(repo_path).expect("failed to init repo");
        // Configure user for commits
        let mut config = repo.config().expect("failed to get config");
        config
            .set_str("user.name", "Test")
            .expect("failed to set user.name");
        config
            .set_str("user.email", "test@example.com")
            .expect("failed to set user.email");
        (temp_dir, repo)
    }

    #[test]
    fn test_status_basic() {
        let (td, repo) = init_repo();
        let repo_path = td.path();
        let resolved = crate::resolve::ResolvedRepo {
            inner: repo,
            path: repo_path.to_path_buf(),
        };
        let status = status(&resolved).expect("status failed");
        assert!(status.staged.is_empty());
        assert!(status.unstaged.is_empty());
        assert!(status.untracked.is_empty());
    }

    #[test]
    fn test_status_untracked() {
        let (td, repo) = init_repo();
        let repo_path = td.path();
        // Create an untracked file
        std::fs::write(repo_path.join("untracked.txt"), "hello").expect("failed to write file");
        let resolved = crate::resolve::ResolvedRepo {
            inner: repo,
            path: repo_path.to_path_buf(),
        };
        let status = status(&resolved).expect("status failed");
        assert!(status.staged.is_empty());
        assert!(status.unstaged.is_empty());
        assert_eq!(status.untracked, vec!["untracked.txt".to_string()]);
    }

    #[test]
    fn test_status_staged_and_unstaged() {
        let (td, repo) = init_repo();
        let repo_path = td.path();
        // Create a file and stage it
        std::fs::write(repo_path.join("staged.txt"), "content").expect("failed to write file");
        let mut index = repo.index().expect("failed to get index");
        index
            .add_path(Path::new("staged.txt"))
            .expect("failed to stage");
        index.write().expect("failed to write index");
        // Modify the file (now staged + unstaged)
        std::fs::write(repo_path.join("staged.txt"), "modified content").expect("failed to modify");
        // Create another untracked file
        std::fs::write(repo_path.join("untracked.txt"), "world").expect("failed to write file");
        let resolved = crate::resolve::ResolvedRepo {
            inner: repo,
            path: repo_path.to_path_buf(),
        };
        let status = status(&resolved).expect("status failed");
        // staged.txt should appear in both staged and unstaged
        assert_eq!(status.staged, vec!["staged.txt".to_string()]);
        assert_eq!(status.unstaged, vec!["staged.txt".to_string()]);
        assert_eq!(status.untracked, vec!["untracked.txt".to_string()]);
    }

    #[test]
    fn test_diff_basic() {
        let (td, repo) = init_repo();
        let repo_path = td.path();
        let resolved = crate::resolve::ResolvedRepo {
            inner: repo,
            path: repo_path.to_path_buf(),
        };
        // Diff of an empty repo should be empty
        let diff_result = crate::diff::diff(&resolved, None, None).expect("diff failed");
        assert!(diff_result.files.is_empty());
        assert_eq!(diff_result.total_additions, 0);
        assert_eq!(diff_result.total_deletions, 0);
    }

    #[test]
    fn test_diff_add_file() {
        let (td, repo) = init_repo();
        let repo_path = td.path();
        // Create a file
        std::fs::write(repo_path.join("new.txt"), "content").expect("failed to write file");
        let resolved = crate::resolve::ResolvedRepo {
            inner: repo,
            path: repo_path.to_path_buf(),
        };
        // Diff from empty to HEAD (which doesn't exist yet) - we'll compare worktree to index
        // Actually, let's just diff worktree vs index (default)
        let diff_result = crate::diff::diff(&resolved, None, None).expect("diff failed");
        // new.txt should appear as an untracked file, which shows up in diff as an addition?
        // In git diff, untracked files are not shown by default unless we use --untracked-files=all
        // Our diff function includes untracked files via opts.include_untracked(true)
        // So new.txt should appear in the diff as an addition.
        // Since we're returning placeholder zeros for additions/deletions, we just check that the file is present.
        assert_eq!(diff_result.files.len(), 1);
        assert_eq!(diff_result.files[0].path, "new.txt");
        // additions and deletions are placeholders (0) for now
    }
}
