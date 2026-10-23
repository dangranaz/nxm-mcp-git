//! Policy-gated `git worktree add` for the parallel-workstream convention.
//!
//! Step two of the convention (see `WORKFLOW.md` → "Parallel workstreams in
//! isolated worktrees"): after the workstream's integration branch exists
//! (created from `develop` by [`crate::write::create_integration_branch`]), its
//! isolated worktree is materialised here. The order is forced and mechanical —
//! the worktree can only be rooted at a branch that already exists — so the
//! historical "branch from the wrong checkout" mistake cannot recur, and a
//! workstream's uncommitted work never shares a tree with another's.
//!
//! Every precondition is checked BEFORE git runs and surfaced as a typed
//! [`WriteError`], so the caller (agent or TUI) learns exactly why an add was
//! refused. git itself is always invoked as an argv array, never a shell
//! string.

use std::path::Path;
use std::process::Command;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::argv::ensure_ref_arg;
use crate::policy::Policy;
use crate::resolve::ResolvedRepo;
use crate::write::WriteError;

/// Directory, relative to the repo root, under which every workstream worktree
/// must live. A worktree outside it is refused so trees never scatter.
pub const WORKTREE_ROOT: &str = ".worktrees";

/// Result of `git_worktree_add`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeResult {
    /// Repo-relative path of the worktree that was added.
    pub path: String,
    /// Branch the worktree was checked out on.
    pub branch: String,
}

/// Whether a local branch `name` exists in the repo.
fn branch_exists(repo: &ResolvedRepo, name: &str) -> Result<bool, WriteError> {
    Ok(repo
        .inner
        .find_branch(name, git2::BranchType::Local)
        .is_ok())
}

/// Run `git <args>` in the repo as a strict argv array, mapping failure to a
/// typed [`WriteError::Git`].
fn run_git(repo: &ResolvedRepo, args: &[&str]) -> Result<(), WriteError> {
    let out = Command::new("git")
        .current_dir(&repo.path)
        .args(args)
        .output()
        .with_context(|| format!("spawning git {:?} in {}", args, repo.path.display()))?;
    if !out.status.success() {
        return Err(WriteError::Git {
            args: args.iter().copied().map(String::from).collect(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        });
    }
    Ok(())
}

/// Add the isolated worktree of a workstream, rooted at an existing branch.
///
/// Defaults follow the convention: `branch = integration/<workstream>` and
/// `path = .worktrees/<workstream>` (repo-relative). Both can be overridden,
/// but the path is always resolved relative to the repo root and MUST stay
/// under [`WORKTREE_ROOT`].
///
/// Preconditions, each a typed refusal before git runs:
/// - the target `branch` MUST already exist (forced order: integration branch
///   first, then its worktree) → [`WriteError::Other`];
/// - the resolved `path` MUST NOT already exist → [`WriteError::Other`];
/// - the resolved `path` MUST be under [`WORKTREE_ROOT`] → [`WriteError::Other`].
///
/// Policy-gated only in that it loads the repo policy first: a malformed
/// `.nxm-git-policy.toml` fails loudly here too, exactly like the other writes.
pub fn worktree_add(
    repo: &ResolvedRepo,
    workstream: &str,
    branch: Option<&str>,
    path: Option<&str>,
) -> Result<WorktreeResult, WriteError> {
    ensure_ref_arg(workstream)?;
    if workstream.contains('/') {
        return Err(WriteError::Other(anyhow::anyhow!(
            "invalid workstream {workstream:?}: must not contain '/'"
        )));
    }
    // Load the policy so a malformed file fails loudly, consistent with the
    // other write operations.
    let _policy = Policy::load(&repo.path)?;

    let branch = match branch {
        Some(b) => {
            ensure_ref_arg(b)?;
            b.to_owned()
        }
        None => format!("integration/{workstream}"),
    };
    let rel_path = match path {
        Some(p) => {
            ensure_ref_arg(p)?;
            p.to_owned()
        }
        None => format!("{WORKTREE_ROOT}/{workstream}"),
    };

    // The path must stay under `.worktrees/` — compared on path boundaries so
    // a sibling like `.worktrees-evil/x` cannot slip through.
    let under_root = rel_path == WORKTREE_ROOT
        || rel_path.starts_with(&format!("{WORKTREE_ROOT}/"))
            && !rel_path.contains("..")
            && Path::new(&rel_path).is_relative();
    if !under_root {
        return Err(WriteError::Other(anyhow::anyhow!(
            "worktree path {rel_path:?} must be a relative path under {WORKTREE_ROOT}/"
        )));
    }

    // The branch must already exist: the integration branch is created first.
    if !branch_exists(repo, &branch)? {
        return Err(WriteError::Other(anyhow::anyhow!(
            "branch {branch:?} does not exist: create the integration branch before its worktree"
        )));
    }

    // The worktree path must not already exist.
    let abs_path = repo.path.join(&rel_path);
    if abs_path.exists() {
        return Err(WriteError::Other(anyhow::anyhow!(
            "worktree path {rel_path:?} already exists"
        )));
    }

    info!(
        target: "nxm::git::worktree",
        op = "worktree_add",
        path = %rel_path,
        branch = %branch,
        "worktree add"
    );
    run_git(
        repo,
        &["worktree", "add", rel_path.as_str(), branch.as_str()],
    )?;
    Ok(WorktreeResult {
        path: rel_path,
        branch,
    })
}
