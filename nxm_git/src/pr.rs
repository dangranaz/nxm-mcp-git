//! Pull request creation: a thin, non-merging wrapper around `gh pr create`.
//!
//! `gh` runs inside the repository worktree with an argument array — title,
//! body and base branch are separate argv elements, never interpolated into a
//! command string. Nothing here can merge a PR: the tool only opens one and
//! returns its URL.

use std::process::Command;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::info;

use crate::argv::ensure_ref_arg;
use crate::policy::{check_policy, GitOp, Policy, PolicyDenied, PolicyFlags};
use crate::resolve::ResolvedRepo;

/// Result of [`pr_create`]: the URL of the PR that was opened.
///
/// # Examples
///
/// ```
/// let pr = nxm_git::PrResult { url: "https://github.com/draco690456/nxm-git/pull/1".into() };
/// assert_eq!(pr.url, "https://github.com/draco690456/nxm-git/pull/1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrResult {
    /// URL of the pull request that was opened.
    pub url: String,
}

/// Error from [`pr_create`], shaped for MCP clients like `WriteError`.
#[derive(Debug, Error)]
pub enum PrError {
    /// Policy denied the operation (gh was not run).
    #[error(transparent)]
    Policy(#[from] PolicyDenied),
    /// `gh` exited non-zero; stderr carries the reason.
    #[error("gh {args:?} failed: {stderr}")]
    Gh {
        /// The `gh` argv that failed (without the leading `gh`).
        args: Vec<String>,
        /// gh's stderr output.
        stderr: String,
    },
    /// Anything else (invalid argument, `gh` missing, no URL on stdout, ...).
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl PrError {
    /// Structured JSON payload for MCP clients: `kind` plus details, in the
    /// same shape as `WriteError::to_value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use nxm_git::PrError;
    /// let payload = PrError::Other(anyhow::anyhow!("gh is not installed")).to_value();
    /// assert_eq!(payload["kind"], "error");
    /// ```
    pub fn to_value(&self) -> serde_json::Value {
        match self {
            Self::Policy(denied) => serde_json::json!({
                "kind": "policy_denied",
                "reason": denied.reason,
                "op": denied.op,
                "branch": denied.branch,
            }),
            Self::Gh { args, stderr } => serde_json::json!({
                "kind": "gh_error",
                "args": args,
                "stderr": stderr,
            }),
            Self::Other(e) => serde_json::json!({ "kind": "error", "message": e.to_string() }),
        }
    }
}

/// A pull request always needs a title; without this check `gh` would fail
/// later with a much worse message.
fn ensure_title(title: &str) -> Result<()> {
    anyhow::ensure!(!title.is_empty(), "title must not be empty");
    Ok(())
}

/// Open a pull request with `gh pr create --title <title> --body <body> --base <base>`.
///
/// Policy-gated like the write tools: opening a PR against a protected base
/// branch is checked as [`GitOp::Push`] on `base` and requires `confirm=true`
/// (a malformed policy file also fails here, before `gh` runs). The command
/// then runs inside `repo` with an argument array, so title, body and base are
/// data, never shell syntax. Only the PR is opened: no `--merge` and no other
/// auto-merge flag is ever passed.
///
/// # Examples
///
/// ```no_run
/// # use nxm_git::{ResolvedRepo, pr_create};
/// # let repo = ResolvedRepo::open("/path/to/repo", &[])?;
/// let pr = pr_create(&repo, "feat: add git_pr_create", "Opened from the MCP tool.", "main", true)?;
/// println!("{}", pr.url);
/// # Ok::<(), nxm_git::PrError>(())
/// ```
pub fn pr_create(
    repo: &ResolvedRepo,
    title: &str,
    body: &str,
    base: &str,
    confirm: bool,
) -> Result<PrResult, PrError> {
    ensure_ref_arg(base)?;
    ensure_title(title)?;
    // Policy gate BEFORE gh runs: a PR targets `base`, so opening one against a
    // protected base requires confirmation, mirroring push. Pure check; a
    // malformed policy file fails loudly here.
    let policy = Policy::load(&repo.path).map_err(PrError::Other)?;
    check_policy(
        GitOp::Push,
        &policy,
        &PolicyFlags {
            confirm,
            allow_force: false,
            target_branch: Some(base.to_owned()),
            source_branch: None,
        },
    )?;
    let args = [
        "pr", "create", "--title", title, "--body", body, "--base", base,
    ];
    info!(target: "nxm::git::pr", op = "pr_create", base = %base, confirm, "opening pull request");
    let out = Command::new("gh")
        .current_dir(&repo.path)
        .args(args)
        .output()
        .with_context(|| format!("spawning gh {args:?} in {}", repo.path.display()))?;
    if !out.status.success() {
        return Err(PrError::Gh {
            args: args.iter().copied().map(String::from).collect(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        });
    }
    // `gh pr create` prints the PR URL as the LAST line of stdout; earlier lines
    // may be progress/hints. Require an HTTPS GitHub PR URL (`/pull/`) and take
    // the last match, so noise lines or a non-TLS `http://` are never mistaken
    // for the PR URL.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let url = stdout
        .lines()
        .map(str::trim)
        .rfind(|line| line.starts_with("https://") && line.contains("/pull/"))
        .context("gh pr create printed no PR URL on stdout")?
        .to_owned();
    info!(target: "nxm::git::pr", op = "pr_create", url = %url, "pull request opened");
    Ok(PrResult { url })
}
