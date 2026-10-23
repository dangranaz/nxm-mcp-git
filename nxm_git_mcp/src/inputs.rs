//! Typed input parameters for every MCP tool, deserialized from the client's
//! `arguments`. Kept separate from the tool-router logic in `server.rs`.

use serde::Deserialize;

/// Input for the `git_status` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitStatusInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
}

/// Input for the `git_diff` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitDiffInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Optional "from" ref (e.g., "HEAD~1", "main"). Default: worktree.
    #[serde(default)]
    pub from: Option<String>,
    /// Optional "to" ref (e.g., "HEAD", "feature"). Default: worktree.
    #[serde(default)]
    pub to: Option<String>,
}

/// Input for the `git_log` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitLogInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Optional revision range (e.g., "main..feature", "HEAD~10..HEAD"). Default: HEAD.
    #[serde(default)]
    pub range: Option<String>,
    /// Optional maximum number of commits to return. Default: 100.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Input for the `git_branches` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitBranchesInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
}

/// Input for the `git_show` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitShowInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Ref to show (e.g., "HEAD", "v1.0", "feature/branch"). Required.
    pub ref_name: String,
    /// Optional path to show file at that ref. If omitted, shows commit info.
    #[serde(default)]
    pub path: Option<String>,
}

/// Input for the `git_blame` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitBlameInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Path to the file to blame.
    pub path: String,
}

/// Input for the `git_scan_forbidden` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitScanForbiddenInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Optional ref to scan (e.g., "HEAD", "main", "v1.0"). Default: HEAD.
    #[serde(default)]
    pub ref_name: Option<String>,
}

/// Input for the `git_commit` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitCommitInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Commit message for the staged changes.
    pub message: String,
    /// Explicit confirmation; required to commit on a protected branch.
    #[serde(default)]
    pub confirm: bool,
    /// Neutral flag, unused for commits (uniform across write tools).
    #[serde(default)]
    pub allow_force: bool,
}

/// Input for the `git_create_branch` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitCreateBranchInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Name of the branch to create (the worktree is not switched).
    pub name: String,
    /// Base to root the new branch at (e.g. "develop", "integration/foo").
    /// Omitted = the policy's develop branch (NOT the current HEAD).
    #[serde(default)]
    pub base: Option<String>,
    /// Explicit confirmation; required for a `work/*` branch based on develop.
    #[serde(default)]
    pub confirm: bool,
    /// Neutral flag, unused for branch creation (uniform across write tools).
    #[serde(default)]
    pub allow_force: bool,
}

/// Input for the `git_create_integration_branch` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitCreateIntegrationBranchInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Workstream slug; the branch created is `integration/<workstream>`.
    /// Must not contain '/'.
    pub workstream: String,
    /// Base to root the integration branch at. Omitted = the policy's develop
    /// branch (NOT the current HEAD).
    #[serde(default)]
    pub base: Option<String>,
    /// Explicit confirmation (passed through to the create gate).
    #[serde(default)]
    pub confirm: bool,
}

/// Input for the `git_worktree_add` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitWorktreeAddInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Workstream slug; drives the default branch and path. Must not contain '/'.
    pub workstream: String,
    /// Branch to check out in the worktree. Omitted = `integration/<workstream>`.
    /// The branch MUST already exist.
    #[serde(default)]
    pub branch: Option<String>,
    /// Repo-relative worktree path. Omitted = `.worktrees/<workstream>`. Must
    /// live under `.worktrees/` and must not already exist.
    #[serde(default)]
    pub path: Option<String>,
}

/// Input for the `git_switch` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitSwitchInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Name of the existing branch to switch to.
    pub name: String,
    /// Neutral flag, unused for switching (uniform across write tools).
    #[serde(default)]
    pub confirm: bool,
    /// Neutral flag, unused for switching (uniform across write tools).
    #[serde(default)]
    pub allow_force: bool,
}

/// Input for the `git_push` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitPushInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Remote to push to. Default: "origin".
    #[serde(default)]
    pub remote: Option<String>,
    /// Force the push (needs allow_force + confirm; checked as ForcePush).
    #[serde(default)]
    pub force: bool,
    /// Explicit confirmation; required to push a protected branch and for force-push.
    #[serde(default)]
    pub confirm: bool,
    /// Per-call force-push opt-in (together with confirm, enables force).
    #[serde(default)]
    pub allow_force: bool,
}

/// Input for the `git_pr_create` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GitPrCreateInput {
    /// Path to the Git repository worktree.
    pub repo_path: String,
    /// Title of the pull request.
    pub title: String,
    /// Body of the pull request (markdown).
    pub body: String,
    /// Base branch the PR targets (e.g., "main"). Must be a branch/ref name.
    pub base: String,
    /// Explicit confirmation; required when the base is a protected branch.
    #[serde(default)]
    pub confirm: bool,
}
