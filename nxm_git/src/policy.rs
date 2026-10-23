//! Write policy core: parse `.nxm-git-policy.toml` and gate git write
//! operations with a pure, side-effect-free check.
//!
//! The check ([`check_policy`]) is deliberately pure: it never touches git.
//! The caller (T6 write tools) resolves the affected branch and passes it via
//! [`PolicyFlags::target_branch`].

use std::fmt;
use std::path::Path;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::scan::{glob_matches, FORBIDDEN_PATTERNS};

/// Name of the policy file, read from the target repository root.
pub const POLICY_FILE: &str = ".nxm-git-policy.toml";

/// Repository write policy parsed from `.nxm-git-policy.toml`.
///
/// A missing file yields [`Policy::default`]; a malformed file or a file with
/// unknown keys is a load error — a typo must not silently unprotect a branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Policy {
    /// Branches protected from direct commits and pushes.
    pub protected_branches: Vec<String>,
    /// Whether force-push is permitted at all (still requires `confirm`).
    pub allow_force: bool,
    /// Whether destructive operations require an explicit confirm flag.
    pub destructive_confirm: bool,
    /// Content patterns that block a push (defaults to the read-side scan list).
    pub forbidden_patterns: Vec<String>,
    /// Repo-relative paths excluded from the forbidden-content scan, so a repo
    /// does not block its own push on legitimate content (the file that
    /// *defines* the patterns, and test fixtures that use example tokens).
    /// Each entry is a path prefix or a `*`-glob (see
    /// [`crate::scan::path_matches_exclude`]); empty = scan everything. The
    /// default is deliberately narrow — never whole metadata globs like
    /// `*.md`, which could hide a real leak.
    pub scan_exclude: Vec<String>,
    /// Commands that must pass before a push is allowed.
    pub require_pre_push: Vec<String>,
    /// Landing-zone branches for parallel workstreams (e.g. `["integration/*"]`).
    /// Empty = the workstream convention is off: the G1/G2 guards are disabled
    /// and behavior is unchanged (retro-compatible).
    pub integration_branches: Vec<String>,
    /// Whether a direct commit on `develop_branch` is denied outright or
    /// requires `confirm`. A commit based on an integration branch is always
    /// allowed.
    pub develop_direct_commit: DevelopDirectCommit,
    /// Name of the integration/confluence branch (default `develop`). Read
    /// from the policy, never hard-coded.
    pub develop_branch: String,
    /// Prefix of workstream branches (default `work/`).
    pub work_branch_prefix: String,
}

/// Default `scan_exclude`: narrow, repo-agnostic exclusions that cover the
/// structural false positives of a repo scanning itself, without hiding a real
/// leak. Deliberately NOT broad metadata globs (`*.md`, `Cargo.toml`): a real
/// secret pasted into metadata must still surface.
fn default_scan_exclude() -> Vec<String> {
    vec![
        // The file that DEFINES the forbidden patterns matches every one of
        // them by construction; scanning it is always a false positive.
        "nxm_git/src/scan.rs".to_string(),
        // Test fixtures use example tokens/URLs/paths on purpose.
        "**/tests/**".to_string(),
    ]
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            protected_branches: vec!["main".to_string()],
            allow_force: false,
            destructive_confirm: true,
            forbidden_patterns: FORBIDDEN_PATTERNS.iter().map(|p| p.to_string()).collect(),
            scan_exclude: default_scan_exclude(),
            require_pre_push: Vec::new(),
            integration_branches: Vec::new(),
            develop_direct_commit: DevelopDirectCommit::default(),
            develop_branch: "develop".to_string(),
            work_branch_prefix: "work/".to_string(),
        }
    }
}

impl Policy {
    /// Load the policy from `<repo_root>/.nxm-git-policy.toml`.
    ///
    /// Missing file → default policy. Unreadable or malformed file → error.
    pub fn load(repo_root: &Path) -> anyhow::Result<Self> {
        let path = repo_root.join(POLICY_FILE);
        match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(anyhow::Error::new(e).context(format!("cannot read {POLICY_FILE}"))),
            Ok(raw) => toml::from_str(&raw).with_context(|| format!("invalid {POLICY_FILE}")),
        }
    }

    /// Whether `branch` is protected by this policy.
    fn is_protected(&self, branch: &str) -> bool {
        self.protected_branches.iter().any(|b| b == branch)
    }
}

/// Whether a direct commit on [`Policy::develop_branch`] is allowed, and
/// under what condition. A commit based on an integration branch is always
/// allowed (see the G1 guard in [`check_policy`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DevelopDirectCommit {
    /// Allowed only with `flags.confirm`.
    #[default]
    Confirm,
    /// Never allowed, even with `confirm`.
    Deny,
}

/// Git write operation subject to policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GitOp {
    Commit,
    Push,
    ForcePush,
    CreateBranch,
    Switch,
}

/// Per-call flags supplied by the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyFlags {
    /// Explicit confirmation for protected/destructive operations.
    pub confirm: bool,
    /// Per-call force-push opt-in (enables force even if the policy file
    /// does not, alongside `Policy::allow_force`).
    pub allow_force: bool,
    /// Branch the operation targets: current branch for `Commit`, target
    /// branch for `Push`/`ForcePush`, new/destination branch for the rest.
    pub target_branch: Option<String>,
    /// Branch the operation is based on, resolved by the caller via git
    /// (task 05) and passed here. `None` is a fail-safe: the workstream base
    /// guard denies the operation.
    pub source_branch: Option<String>,
}

/// Why an operation was denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DenialReason {
    /// Commit/push targets a protected branch without `confirm`.
    ProtectedBranch,
    /// Force-push disabled: neither `Policy::allow_force` nor the per-call
    /// `PolicyFlags::allow_force` is set.
    ForceNotAllowed,
    /// Force-push enabled, but `confirm` is missing.
    ForceNeedsConfirm,
    /// Branch-targeting operation without `target_branch`: protection
    /// cannot be verified, so the operation is denied (fail-safe).
    UnknownBranch,
    /// Direct commit on the develop branch is denied by `develop_direct_commit`.
    DevelopDirectCommit,
    /// Work branch created without a known source branch: fail-safe denial.
    UnknownBase,
    /// Work branch based on a branch that is not an integration branch nor
    /// develop (with confirm).
    WorkBranchBaseNotAllowed,
}

impl fmt::Display for DenialReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::ProtectedBranch => "operation targets a protected branch without confirmation",
            Self::ForceNotAllowed => "force-push is not allowed by policy",
            Self::ForceNeedsConfirm => "force-push requires explicit confirmation",
            Self::UnknownBranch => "target branch unknown, cannot verify protection",
            Self::DevelopDirectCommit => {
                "direct commit to the develop branch is not allowed by policy"
            }
            Self::UnknownBase => "work branch created without a known source branch",
            Self::WorkBranchBaseNotAllowed => {
                "work branch is not based on an integration branch or develop with confirm"
            }
        };
        f.write_str(msg)
    }
}

/// Structured denial: reason + operation + involved branch. No free text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("policy denied: {reason} (op: {op:?})")]
pub struct PolicyDenied {
    /// Why the operation was denied.
    pub reason: DenialReason,
    /// The denied operation.
    pub op: GitOp,
    /// Branch involved in the denial, if any.
    pub branch: Option<String>,
}

/// Check an operation against the policy. Pure: no git access, no side effects.
///
/// The caller must supply the affected branch in `flags.target_branch`.
/// A branch-targeting operation without a target branch is denied
/// ([`DenialReason::UnknownBranch`], fail-safe).
///
/// Denial rules:
/// - `Commit`/`Push` on a protected branch → denied unless `flags.confirm`.
/// - `ForcePush` → denied unless force is enabled (`Policy::allow_force` or
///   `flags.allow_force`); when enabled, `flags.confirm` is *also* required.
/// - `CreateBranch`/`Switch` rewrite no history: always allowed.
pub fn check_policy(op: GitOp, policy: &Policy, flags: &PolicyFlags) -> Result<(), PolicyDenied> {
    match op {
        GitOp::Commit | GitOp::Push => {
            let Some(branch) = flags.target_branch.as_deref() else {
                return Err(PolicyDenied {
                    reason: DenialReason::UnknownBranch,
                    op,
                    branch: None,
                });
            };
            if policy.is_protected(branch) && !flags.confirm {
                return Err(PolicyDenied {
                    reason: DenialReason::ProtectedBranch,
                    op,
                    branch: Some(branch.to_owned()),
                });
            }
            // G1: a direct commit on the develop branch is governed by the
            // workstream convention, but only when the repo declared at least
            // one integration branch (empty list = behavior unchanged).
            if op == GitOp::Commit
                && branch == policy.develop_branch
                && !policy.integration_branches.is_empty()
            {
                let from_integration = flags.source_branch.as_deref().is_some_and(|b| {
                    policy
                        .integration_branches
                        .iter()
                        .any(|g| glob_matches(b, g))
                });
                if from_integration {
                    return Ok(()); // legit milestone sync from an integration branch
                }
                match policy.develop_direct_commit {
                    DevelopDirectCommit::Deny => {
                        return Err(PolicyDenied {
                            reason: DenialReason::DevelopDirectCommit,
                            op,
                            branch: Some(branch.to_owned()),
                        });
                    }
                    DevelopDirectCommit::Confirm if !flags.confirm => {
                        return Err(PolicyDenied {
                            reason: DenialReason::DevelopDirectCommit,
                            op,
                            branch: Some(branch.to_owned()),
                        });
                    }
                    DevelopDirectCommit::Confirm => {}
                }
            }
            Ok(())
        }
        GitOp::ForcePush => {
            if !policy.allow_force && !flags.allow_force {
                return Err(PolicyDenied {
                    reason: DenialReason::ForceNotAllowed,
                    op,
                    branch: flags.target_branch.clone(),
                });
            }
            if !flags.confirm {
                return Err(PolicyDenied {
                    reason: DenialReason::ForceNeedsConfirm,
                    op,
                    branch: flags.target_branch.clone(),
                });
            }
            Ok(())
        }
        GitOp::CreateBranch => {
            // G2: a work branch must be based on an integration branch or on
            // develop (with confirm); never on main, never without a known
            // base. Active only when integration_branches is non-empty.
            if let Some(new_branch) = flags.target_branch.as_deref() {
                if new_branch.starts_with(&policy.work_branch_prefix)
                    && !policy.integration_branches.is_empty()
                {
                    let Some(base) = flags.source_branch.as_deref() else {
                        return Err(PolicyDenied {
                            reason: DenialReason::UnknownBase,
                            op,
                            branch: Some(new_branch.to_owned()),
                        });
                    };
                    if policy
                        .integration_branches
                        .iter()
                        .any(|g| glob_matches(base, g))
                    {
                        return Ok(());
                    }
                    if base == policy.develop_branch && flags.confirm {
                        return Ok(());
                    }
                    return Err(PolicyDenied {
                        reason: DenialReason::WorkBranchBaseNotAllowed,
                        op,
                        branch: Some(new_branch.to_owned()),
                    });
                }
            }
            Ok(())
        }
        GitOp::Switch => Ok(()),
    }
}
