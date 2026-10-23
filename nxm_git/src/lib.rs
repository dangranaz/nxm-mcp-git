pub mod argv;
pub mod blame;
pub mod branches;
pub mod config;
pub mod diff;
pub mod log;
pub mod policy;
pub mod pr;
pub mod prepush;
pub mod resolve;
pub mod scan;
pub mod show;
pub mod status;
pub mod worktree;
pub mod write;

pub use blame::blame;
pub use branches::branches;
pub use config::GitMcpConfig;
pub use diff::diff;
pub use log::log;
pub use policy::{check_policy, DenialReason, GitOp, Policy, PolicyDenied, PolicyFlags};
pub use pr::{pr_create, PrError, PrResult};
pub use resolve::ResolvedRepo;
pub use scan::scan_forbidden;
pub use show::show;
pub use status::status;
pub use worktree::{worktree_add, WorktreeResult};
pub use write::{
    commit, create_branch, create_integration_branch, push, switch_branch, BranchResult,
    CommitResult, PushResult, WriteError,
};
