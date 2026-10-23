use anyhow::Result;
use git2::BranchType;
use serde::{Deserialize, Serialize};

/// Information about a branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchInfo {
    /// Branch name.
    pub name: String,
    /// Whether this is a local branch.
    pub is_local: bool,
    /// Whether this is a remote branch.
    pub is_remote: bool,
    /// The commit SHA-1 that this branch points to.
    pub commit: String,
    /// Number of commits this branch is ahead of its upstream (if any).
    pub ahead: Option<u32>,
    /// Number of commits this branch is behind its upstream (if any).
    pub behind: Option<u32>,
}

/// The result of `git branch`, returning information about branches.
///
/// Equivalent to `git branch -vv` but returns structured data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchesResult {
    /// List of branches.
    pub branches: Vec<BranchInfo>,
}

/// Compute the branches of the repository.
pub fn branches(repo: &crate::resolve::ResolvedRepo) -> Result<BranchesResult> {
    let mut branches = Vec::new();

    let branch_iter = repo.inner.branches(None)?;
    for branch_result in branch_iter {
        let (branch, branch_type) = branch_result?;
        let name = branch.name()?.unwrap_or("<unknown>").to_string();
        let is_local = matches!(branch_type, BranchType::Local);
        let is_remote = matches!(branch_type, BranchType::Remote);

        let commit = branch.get().peel_to_commit()?;
        let commit_sha = commit.id().to_string();

        let (ahead, behind) = if is_local {
            match branch.upstream() {
                Ok(upstream_branch) => {
                    let upstream_commit = upstream_branch.get().peel_to_commit()?;
                    let ahead_behind = repo
                        .inner
                        .graph_ahead_behind(commit.id(), upstream_commit.id())?;
                    (Some(ahead_behind.0 as u32), Some(ahead_behind.1 as u32))
                }
                Err(_) => (None, None),
            }
        } else {
            (None, None)
        };

        branches.push(BranchInfo {
            name,
            is_local,
            is_remote,
            commit: commit_sha,
            ahead,
            behind,
        });
    }

    Ok(BranchesResult { branches })
}
