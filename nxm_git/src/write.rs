//! Policy-gated git write operations.
//!
//! Every write loads [`Policy`] from the repo root, resolves the branch the
//! operation targets, and calls [`check_policy`] BEFORE running git: a denied
//! write returns [`WriteError::Policy`] and git never runs. `push` additionally
//! runs [`scan_forbidden`] and refuses on hits; it then runs the policy's
//! `require_pre_push` commands as a hard gate before the push itself. All git invocations are
//! shell-outs with argument arrays — never interpolated command strings.

use std::process::Command;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

use crate::argv::ensure_ref_arg;
use crate::policy::{check_policy, GitOp, Policy, PolicyDenied, PolicyFlags};
use crate::prepush;
use crate::resolve::ResolvedRepo;
use crate::scan::{glob_matches, scan_forbidden, ScanResult};

/// Result of `git_commit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitResult {
    /// SHA of the commit just created.
    pub sha: String,
    /// Branch the commit landed on.
    pub branch: String,
    /// The commit message.
    pub message: String,
}

/// Result of `git_create_branch` / `git_switch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchResult {
    /// Branch created or switched to.
    pub branch: String,
}

/// Result of `git_push`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushResult {
    /// Branch that was pushed.
    pub branch: String,
    /// Remote the branch was pushed to.
    pub remote: String,
    /// Whether the push was forced.
    pub forced: bool,
}

/// Structured error from a write operation, fit for MCP clients.
#[derive(Debug, Error)]
pub enum WriteError {
    /// Policy denied the operation (git was not run).
    #[error(transparent)]
    Policy(#[from] PolicyDenied),
    /// Forbidden content blocks the push (git was not run).
    #[error("forbidden content blocks push: {} hit(s)", .0.hits.len())]
    Forbidden(ScanResult),
    /// A `require_pre_push` command failed, so the push was not run.
    #[error(transparent)]
    PrePushFailed(#[from] prepush::PrePushError),
    /// git failed; stderr carries the reason.
    #[error("git {:?} failed: {stderr}", .args)]
    Git {
        /// The git argv that failed (without the leading `git`).
        args: Vec<String>,
        /// git's stderr output.
        stderr: String,
    },
    /// Anything else (invalid argument, unreadable policy file, ...).
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl WriteError {
    /// Structured JSON payload for MCP clients: `kind` plus details.
    pub fn to_value(&self) -> serde_json::Value {
        match self {
            Self::Policy(denied) => serde_json::json!({
                "kind": "policy_denied",
                "reason": denied.reason,
                "op": denied.op,
                "branch": denied.branch,
            }),
            Self::Forbidden(scan) => serde_json::json!({
                "kind": "forbidden_content",
                "hits": scan.hits,
            }),
            Self::PrePushFailed(e) => e.to_value(),
            Self::Git { args, stderr } => serde_json::json!({
                "kind": "git_error",
                "args": args,
                "stderr": stderr,
            }),
            Self::Other(e) => serde_json::json!({
                "kind": "error",
                "message": e.to_string(),
            }),
        }
    }
}

/// Current branch name, read from the HEAD symref (works on unborn HEADs).
fn current_branch(repo: &ResolvedRepo) -> Result<String> {
    let head = repo.inner.find_reference("HEAD").context("reading HEAD")?;
    match head.symbolic_target() {
        Some(target) => Ok(target
            .strip_prefix("refs/heads/")
            .unwrap_or(target)
            .to_owned()),
        None => anyhow::bail!("detached HEAD: no current branch"),
    }
}

/// Run `git <args>` inside the repo, strictly as an argv array.
fn run_git(repo: &ResolvedRepo, args: &[&str]) -> Result<String, WriteError> {
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
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Policy gate shared by every write: load the policy, build the flags with
/// the target branch (and the merge/base source when known), and check
/// BEFORE any git command runs.
fn gate(
    repo: &ResolvedRepo,
    op: GitOp,
    target_branch: &str,
    confirm: bool,
    allow_force: bool,
    source_branch: Option<String>,
) -> Result<(), WriteError> {
    let policy = Policy::load(&repo.path)?;
    check_policy(
        op,
        &policy,
        &PolicyFlags {
            confirm,
            allow_force,
            target_branch: Some(target_branch.to_owned()),
            source_branch,
        },
    )?;
    Ok(())
}

/// Resolve the source branch of an in-progress merge, if any.
///
/// Worktree-safe: locates `MERGE_HEAD` via `git rev-parse --git-path MERGE_HEAD`,
/// which returns `.git/MERGE_HEAD` in the main repo and
/// `.git/worktrees/<name>/MERGE_HEAD` in a secondary worktree — instead of
/// hard-coding `.git/MERGE_HEAD` (which never exists in a secondary worktree,
/// where `.git` is a `gitdir:` file).
///
/// When there is no merge in progress (`rev-parse` fails, or `MERGE_HEAD` is
/// not a file, or its name cannot be resolved), returns `None`
/// (fail-safe: the caller on `develop` will then apply `develop_direct_commit`).
fn merge_source_branch(repo: &ResolvedRepo) -> Result<Option<String>> {
    let git_path = match run_git(repo, &["rev-parse", "--git-path", "MERGE_HEAD"]) {
        Ok(p) => p,
        // `rev-parse` fails when there is no MERGE_HEAD: no merge in progress.
        Err(_) => return Ok(None),
    };
    let merge_head = repo.path.join(git_path.trim());
    if !merge_head.is_file() {
        return Ok(None);
    }
    let name = run_git(repo, &["name-rev", "--name-only", "MERGE_HEAD"])?;
    let name = name.trim();
    if name.is_empty() || name == "undefined" {
        return Ok(None);
    }
    Ok(Some(name.to_owned()))
}

/// Commit the staged changes on the current branch.
///
/// Checked as [`GitOp::Commit`] against the current branch: committing on a
/// protected branch additionally requires `confirm`. `allow_force` is
/// accepted for flag uniformity but plays no role for commits.
pub fn commit(
    repo: &ResolvedRepo,
    message: &str,
    confirm: bool,
    allow_force: bool,
) -> Result<CommitResult, WriteError> {
    let branch = current_branch(repo)?;
    // G1: a plain commit on `develop` has no merge source (source_branch =
    // None → develop_direct_commit applies). An in-progress merge resolves
    // its source branch so a legitimate `integration/* -> develop` sync passes.
    let source_branch = merge_source_branch(repo)?;
    info!(
        target: "nxm::git::write",
        op = "commit",
        branch = %branch,
        confirm,
        ?source_branch,
        "write gate"
    );
    gate(
        repo,
        GitOp::Commit,
        &branch,
        confirm,
        allow_force,
        source_branch,
    )?;
    run_git(repo, &["commit", "-m", message])?;
    let sha = run_git(repo, &["rev-parse", "HEAD"])?;
    Ok(CommitResult {
        sha,
        branch,
        message: message.to_owned(),
    })
}

/// Create a branch from an explicit base (without switching to it).
///
/// The new branch is rooted at `base` when given, otherwise at the policy's
/// [`Policy::develop_branch`] — **never** the current HEAD. This is the fix for
/// the historical bug where `create_branch` branched from whatever the shared
/// worktree happened to be checked out on, so a `work/*` created while HEAD sat
/// on another branch was silently rooted at the wrong commit. The resolved base
/// is both the git start-point (`git branch <name> <base>`) and the
/// `source_branch` handed to the G2 guard, so the policy judges the real base.
///
/// Checked as [`GitOp::CreateBranch`]: with the workstream guards active a
/// `work_branch_prefix` branch must be rooted at an integration branch (or at
/// `develop` with `confirm`); with an empty `integration_branches` list the
/// gate is a no-op and any base is accepted.
pub fn create_branch(
    repo: &ResolvedRepo,
    name: &str,
    base: Option<&str>,
    confirm: bool,
    allow_force: bool,
) -> Result<BranchResult, WriteError> {
    ensure_ref_arg(name)?;
    // The base defaults to the policy's develop branch, NOT the current HEAD.
    let policy = Policy::load(&repo.path)?;
    let base = match base {
        Some(base) => {
            ensure_ref_arg(base)?;
            base.to_owned()
        }
        None => policy.develop_branch.clone(),
    };
    info!(
        target: "nxm::git::write",
        op = "create_branch",
        branch = %name,
        base = %base,
        "write gate"
    );
    gate(
        repo,
        GitOp::CreateBranch,
        name,
        confirm,
        allow_force,
        Some(base.clone()),
    )?;
    run_git(repo, &["branch", name, base.as_str()])?;
    Ok(BranchResult {
        branch: name.to_owned(),
    })
}

/// Create the integration branch of a workstream from an explicit base.
///
/// Builds the branch name `integration/<workstream>` and roots it at `base`
/// (default: the policy's [`Policy::develop_branch`]) — never the current HEAD.
/// This is the ergonomic, typo-proof entry point for the first step of the
/// parallel-workstream convention (see `WORKFLOW.md`): create the workstream's
/// private landing zone from `develop`, then add its isolated worktree with
/// [`worktree::worktree_add`].
///
/// The constructed name is validated against the policy's
/// [`Policy::integration_branches`] globs when that list is non-empty: a name
/// that no glob accepts is rejected with [`WriteError::Other`] before any git
/// runs, so the workstream convention cannot be bypassed by a stray prefix.
pub fn create_integration_branch(
    repo: &ResolvedRepo,
    workstream: &str,
    base: Option<&str>,
    confirm: bool,
) -> Result<BranchResult, WriteError> {
    // The workstream slug is the tail of the branch name: reject anything
    // option-like or that would smuggle a path separator into the prefix.
    ensure_ref_arg(workstream)?;
    if workstream.contains('/') {
        return Err(WriteError::Other(anyhow::anyhow!(
            "invalid workstream {workstream:?}: must not contain '/'"
        )));
    }
    let name = format!("integration/{workstream}");
    let policy = Policy::load(&repo.path)?;
    // Guard against a typo'd prefix: when the repo declares integration globs,
    // the constructed name must match at least one of them.
    if !policy.integration_branches.is_empty()
        && !policy
            .integration_branches
            .iter()
            .any(|g| glob_matches(&name, g))
    {
        return Err(WriteError::Other(anyhow::anyhow!(
            "branch {name:?} does not match any configured integration_branches pattern"
        )));
    }
    create_branch(repo, &name, base, confirm, false)
}

/// Switch the worktree to an existing branch.
///
/// Checked as [`GitOp::Switch`]: never policy-restricted in the current
/// rules, but the gate still loads the policy file.
pub fn switch_branch(
    repo: &ResolvedRepo,
    name: &str,
    confirm: bool,
    allow_force: bool,
) -> Result<BranchResult, WriteError> {
    ensure_ref_arg(name)?;
    info!(target: "nxm::git::write", op = "switch", branch = %name, "write gate");
    gate(repo, GitOp::Switch, name, confirm, allow_force, None)?;
    run_git(repo, &["switch", name])?;
    Ok(BranchResult {
        branch: name.to_owned(),
    })
}

/// Push the current branch to a remote.
///
/// Checked as [`GitOp::Push`] (or [`GitOp::ForcePush`] when `force`): pushing
/// a protected branch requires `confirm`; a forced push requires force to be
/// enabled (policy file or `allow_force` flag) AND `confirm`. Independently of
/// the policy, the pushed content must pass [`scan_forbidden`], and `remote`
/// must be a name configured in the repo — never a URL or path, so pushes can
/// only ever target remotes the repository owner set up.
pub fn push(
    repo: &ResolvedRepo,
    remote: Option<&str>,
    force: bool,
    confirm: bool,
    allow_force: bool,
) -> Result<PushResult, WriteError> {
    let remote = remote.unwrap_or("origin");
    ensure_ref_arg(remote)?;
    let remotes = repo
        .inner
        .remotes()
        .with_context(|| "listing configured remotes")?;
    if !remotes.iter().any(|r| r == Some(remote)) {
        return Err(WriteError::Other(anyhow::anyhow!(
            "unknown remote {remote:?}: must be a name configured in the repo, not a URL or path"
        )));
    }
    let branch = current_branch(repo)?;
    let op = if force { GitOp::ForcePush } else { GitOp::Push };
    info!(
        target: "nxm::git::write",
        op = if force { "force_push" } else { "push" },
        branch = %branch,
        remote = %remote,
        confirm,
        allow_force,
        "write gate"
    );
    gate(repo, op, &branch, confirm, allow_force, None)?;
    // Load the policy once: it drives both the scan exclusions and the
    // pre-push commands. (The `gate` above discards its own copy.)
    let policy = Policy::load(&repo.path)?;
    let scan = scan_forbidden(repo, None, &policy.scan_exclude)?;
    if !scan.clean {
        return Err(WriteError::Forbidden(scan));
    }
    // Pre-push hard gate: run the configured commands AFTER the content
    // scan, BEFORE git push.
    prepush::run(repo, &policy.require_pre_push)?;
    if force {
        run_git(repo, &["push", "--force", remote, branch.as_str()])?;
    } else {
        run_git(repo, &["push", remote, branch.as_str()])?;
    }
    Ok(PushResult {
        branch,
        remote: remote.to_owned(),
        forced: force,
    })
}
