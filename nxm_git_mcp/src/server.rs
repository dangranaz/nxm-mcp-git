use anyhow::{Context, Result};
use nxm_git::{self as core, pr::PrError, write::WriteError, ResolvedRepo};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use std::path::PathBuf;
use tracing::{debug, info};

use crate::inputs::{
    GitBlameInput, GitBranchesInput, GitCommitInput, GitCreateBranchInput,
    GitCreateIntegrationBranchInput, GitDiffInput, GitLogInput, GitPrCreateInput, GitPushInput,
    GitScanForbiddenInput, GitShowInput, GitStatusInput, GitSwitchInput, GitWorktreeAddInput,
};

/// Git MCP server state: configuration and tool router.
#[derive(Clone)]
pub struct GitServer {
    /// Allowed root directories for repository operations.
    pub allowed_roots: Vec<PathBuf>,
    /// Tool router for handling MCP tool invocations.
    pub tool_router: ToolRouter<Self>,
}

impl GitServer {
    /// Build the server by loading configuration and initializing the tool router.
    ///
    /// If the configuration file is missing, an empty allowed roots list is used.
    pub fn new() -> Result<Self> {
        info!(target: "nxm::git::mcp", "loading Git MCP configuration");
        let config =
            core::GitMcpConfig::load().with_context(|| "failed to load Git MCP configuration")?;
        info!(
            target: "nxm::git::mcp",
            allowed_roots = ?config.allowed_roots,
            "Git MCP configuration loaded"
        );
        let server = Self {
            allowed_roots: config.allowed_roots,
            tool_router: Self::tool_router(),
        };
        info!(target: "nxm::git::mcp", "Git MCP server built");
        Ok(server)
    }

    /// Build the server with a specific allowed roots list, bypassing config loading.
    #[doc(hidden)]
    pub fn new_with_allowed_roots(allowed_roots: Vec<PathBuf>) -> Self {
        let server = Self {
            allowed_roots,
            tool_router: Self::tool_router(),
        };
        info!(target: "nxm::git::mcp", allowed_roots = ?server.allowed_roots, "Git MCP server built with explicit roots");
        server
    }
}

/// Map a policy-gated write outcome to an MCP result: successes become
/// JSON content, failures become structured `isError` results the agent can
/// act on (denial reason, forbidden hits, or git stderr).
fn write_result<T: serde::Serialize>(
    outcome: Result<T, WriteError>,
) -> Result<CallToolResult, McpError> {
    match outcome {
        Ok(value) => Ok(CallToolResult::success(vec![Content::json(value)?])),
        Err(e) => Ok(CallToolResult::structured_error(e.to_value())),
    }
}

/// Map a PR outcome to an MCP result: successes become JSON content, failures
/// become a structured `isError` payload with the same `kind`-based shape as
/// [`write_result`].
fn pr_result<T: serde::Serialize>(outcome: Result<T, PrError>) -> Result<CallToolResult, McpError> {
    match outcome {
        Ok(value) => Ok(CallToolResult::success(vec![Content::json(value)?])),
        Err(e) => Ok(CallToolResult::structured_error(e.to_value())),
    }
}

#[tool_router]
impl GitServer {
    /// Get the status of a Git repository.
    ///
    /// Returns staged, unstaged, and untracked file paths relative to the repository root.
    #[tool(
        description = "Get the status of a Git repository: staged, unstaged, and untracked files."
    )]
    pub async fn git_status(
        &self,
        Parameters(input): Parameters<GitStatusInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_status", repo_path = %input.repo_path, "ENTER git_status");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let status =
            core::status(&repo).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        debug!(target: "nxm::git::mcp::git_status", staged = %status.staged.len(), unstaged = %status.unstaged.len(), untracked = %status.untracked.len(), "EXIT git_status");
        Ok(CallToolResult::success(vec![Content::json(status)?]))
    }

    /// Get the diff of a Git repository.
    ///
    /// Returns file-level additions/deletions between two refs, or worktree vs index.
    #[tool(
        description = "Get the diff of a Git repository: file additions/deletions between refs or worktree vs index."
    )]
    pub async fn git_diff(
        &self,
        Parameters(input): Parameters<GitDiffInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_diff", repo_path = %input.repo_path, from = ?input.from, to = ?input.to, "ENTER git_diff");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let diff = core::diff(&repo, input.from.as_deref(), input.to.as_deref())
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        debug!(target: "nxm::git::mcp::git_diff", files = %diff.files.len(), "EXIT git_diff");
        Ok(CallToolResult::success(vec![Content::json(diff)?]))
    }

    /// Get the log of a Git repository.
    ///
    /// Returns commits with SHA, author, date, and subject for the given range.
    #[tool(
        description = "Get the log of a Git repository: commits with SHA, author, date, and subject."
    )]
    pub async fn git_log(
        &self,
        Parameters(input): Parameters<GitLogInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_log", repo_path = %input.repo_path, range = ?input.range, limit = ?input.limit, "ENTER git_log");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let log = core::log(&repo, input.range.as_deref(), input.limit)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        debug!(target: "nxm::git::mcp::git_log", commits = %log.commits.len(), "EXIT git_log");
        Ok(CallToolResult::success(vec![Content::json(log)?]))
    }

    /// Get the branches of a Git repository.
    ///
    /// Returns local and remote branches with ahead/behind vs upstream.
    #[tool(
        description = "Get the branches of a Git repository: local/remote with ahead/behind vs upstream."
    )]
    pub async fn git_branches(
        &self,
        Parameters(input): Parameters<GitBranchesInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_branches", repo_path = %input.repo_path, "ENTER git_branches");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let branches =
            core::branches(&repo).map_err(|e| McpError::internal_error(e.to_string(), None))?;
        debug!(target: "nxm::git::mcp::git_branches", branches = %branches.branches.len(), "EXIT git_branches");
        Ok(CallToolResult::success(vec![Content::json(branches)?]))
    }

    /// Show a file at a given ref, or show commit info.
    ///
    /// If `path` is provided, shows file contents at that ref.
    /// If `path` is omitted, shows commit info.
    #[tool(description = "Show a file at a given ref, or show commit info if path is omitted.")]
    pub async fn git_show(
        &self,
        Parameters(input): Parameters<GitShowInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_show", repo_path = %input.repo_path, ref_name = %input.ref_name, path = ?input.path, "ENTER git_show");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let show = core::show(&repo, &input.ref_name, input.path.as_deref())
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        debug!(target: "nxm::git::mcp::git_show", "EXIT git_show");
        Ok(CallToolResult::success(vec![Content::json(show)?]))
    }

    /// Blame a file in the working tree.
    ///
    /// Returns per-line commit, author, and content information.
    #[tool(description = "Blame a file in the working tree: per-line commit, author, and content.")]
    pub async fn git_blame(
        &self,
        Parameters(input): Parameters<GitBlameInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_blame", repo_path = %input.repo_path, path = %input.path, "ENTER git_blame");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let blame = core::blame(&repo, &input.path)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        debug!(target: "nxm::git::mcp::git_blame", lines = %blame.lines.len(), "EXIT git_blame");
        Ok(CallToolResult::success(vec![Content::json(blame)?]))
    }

    /// Scan a ref/worktree for forbidden patterns (private traces, secrets, etc.).
    ///
    /// Returns hits or CLEAN.
    #[tool(
        description = "Scan a ref/worktree for forbidden patterns (private traces, secrets, etc.). Returns hits or CLEAN."
    )]
    pub async fn git_scan_forbidden(
        &self,
        Parameters(input): Parameters<GitScanForbiddenInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_scan_forbidden", repo_path = %input.repo_path, ref_name = ?input.ref_name, "ENTER git_scan_forbidden");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let policy = core::Policy::load(&repo.path)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let scan = core::scan_forbidden(&repo, input.ref_name.as_deref(), &policy.scan_exclude)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        debug!(target: "nxm::git::mcp::git_scan_forbidden", clean = %scan.clean, hits = %scan.hits.len(), "EXIT git_scan_forbidden");
        Ok(CallToolResult::success(vec![Content::json(scan)?]))
    }

    /// Commit the staged changes on the current branch.
    ///
    /// Policy-checked: committing on a protected branch requires confirm=true.
    #[tool(
        description = "Commit staged changes on the current branch. Policy-checked: protected branches require confirm=true."
    )]
    pub async fn git_commit(
        &self,
        Parameters(input): Parameters<GitCommitInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_commit", repo_path = %input.repo_path, "ENTER git_commit");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let outcome = core::commit(&repo, &input.message, input.confirm, input.allow_force);
        debug!(target: "nxm::git::mcp::git_commit", "EXIT git_commit");
        write_result(outcome)
    }

    /// Create a branch from an explicit base without switching to it.
    #[tool(
        description = "Create a branch from an explicit base (does not switch to it). The base defaults to the policy's develop branch, NOT the current HEAD. Policy-checked."
    )]
    pub async fn git_create_branch(
        &self,
        Parameters(input): Parameters<GitCreateBranchInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_create_branch", repo_path = %input.repo_path, name = %input.name, base = ?input.base, "ENTER git_create_branch");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let outcome = core::create_branch(
            &repo,
            &input.name,
            input.base.as_deref(),
            input.confirm,
            input.allow_force,
        );
        debug!(target: "nxm::git::mcp::git_create_branch", "EXIT git_create_branch");
        write_result(outcome)
    }

    /// Create the integration branch of a workstream from an explicit base.
    #[tool(
        description = "Create the integration branch `integration/<workstream>` of a parallel workstream from an explicit base (default: the policy's develop branch, never the current HEAD). Validates the name against the configured integration_branches. Step one of the worktree convention; pair with git_worktree_add."
    )]
    pub async fn git_create_integration_branch(
        &self,
        Parameters(input): Parameters<GitCreateIntegrationBranchInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_create_integration_branch", repo_path = %input.repo_path, workstream = %input.workstream, base = ?input.base, "ENTER git_create_integration_branch");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let outcome = core::create_integration_branch(
            &repo,
            &input.workstream,
            input.base.as_deref(),
            input.confirm,
        );
        debug!(target: "nxm::git::mcp::git_create_integration_branch", "EXIT git_create_integration_branch");
        write_result(outcome)
    }

    /// Add the isolated worktree of a workstream, rooted at an existing branch.
    #[tool(
        description = "Add the isolated worktree of a workstream (default path .worktrees/<workstream>, default branch integration/<workstream>). Refuses if the branch does not exist, the path already exists, or the path is outside .worktrees/. Step two of the worktree convention; run after git_create_integration_branch."
    )]
    pub async fn git_worktree_add(
        &self,
        Parameters(input): Parameters<GitWorktreeAddInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_worktree_add", repo_path = %input.repo_path, workstream = %input.workstream, branch = ?input.branch, path = ?input.path, "ENTER git_worktree_add");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let outcome = core::worktree_add(
            &repo,
            &input.workstream,
            input.branch.as_deref(),
            input.path.as_deref(),
        );
        debug!(target: "nxm::git::mcp::git_worktree_add", "EXIT git_worktree_add");
        write_result(outcome)
    }

    /// Switch the worktree to an existing branch.
    #[tool(description = "Switch the repository worktree to an existing branch. Policy-checked.")]
    pub async fn git_switch(
        &self,
        Parameters(input): Parameters<GitSwitchInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_switch", repo_path = %input.repo_path, name = %input.name, "ENTER git_switch");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let outcome = core::switch_branch(&repo, &input.name, input.confirm, input.allow_force);
        debug!(target: "nxm::git::mcp::git_switch", "EXIT git_switch");
        write_result(outcome)
    }

    /// Push the current branch to a remote.
    ///
    /// Policy-checked: protected branches require confirm=true; force-push
    /// (force=true) requires allow_force + confirm and is additionally blocked
    /// by the forbidden-content scan.
    #[tool(
        description = "Push the current branch to a remote (default origin). Policy-checked: protected branches need confirm; force needs allow_force + confirm; refused if forbidden content is detected."
    )]
    pub async fn git_push(
        &self,
        Parameters(input): Parameters<GitPushInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_push", repo_path = %input.repo_path, remote = ?input.remote, force = input.force, "ENTER git_push");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let outcome = core::push(
            &repo,
            input.remote.as_deref(),
            input.force,
            input.confirm,
            input.allow_force,
        );
        debug!(target: "nxm::git::mcp::git_push", "EXIT git_push");
        write_result(outcome)
    }

    /// Open a pull request with `gh pr create`.
    ///
    /// Policy-checked: opening a PR against a protected base branch requires
    /// confirm=true. Opens the PR only: no `--merge` or any other auto-merge
    /// flag is ever passed, and the returned payload carries the PR URL.
    #[tool(
        description = "Open a pull request with `gh pr create --title --body --base` and return its URL. Policy-checked: a protected base needs confirm=true. Opens the PR only: it never merges."
    )]
    pub async fn git_pr_create(
        &self,
        Parameters(input): Parameters<GitPrCreateInput>,
    ) -> Result<CallToolResult, McpError> {
        debug!(target: "nxm::git::mcp::git_pr_create", repo_path = %input.repo_path, base = %input.base, "ENTER git_pr_create");
        let repo = ResolvedRepo::open(&input.repo_path, &self.allowed_roots)
            .with_context(|| format!("opening repository at {}", input.repo_path))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let outcome = core::pr_create(&repo, &input.title, &input.body, &input.base, input.confirm);
        debug!(target: "nxm::git::mcp::git_pr_create", "EXIT git_pr_create");
        pr_result(outcome)
    }
}

#[tool_handler]
impl ServerHandler for GitServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Typed, safe Git operations via MCP Streamable HTTP. \
                 All paths are validated against allowed roots for safety."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
