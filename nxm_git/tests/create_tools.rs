//! Tests for the creation tools: the `create_branch` base fix,
//! `create_integration_branch`, and `worktree_add`. Every test runs against a
//! throwaway `TempDir` repo; no real repository is ever touched.

use std::path::Path;
use std::process::Command;

use nxm_git::{
    create_branch, create_integration_branch, worktree_add, ResolvedRepo, WorktreeResult,
    WriteError,
};
use tempfile::TempDir;

/// Run git in `dir`, asserting success.
fn run(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// SHA a ref points at.
fn rev(dir: &Path, refname: &str) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", refname])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Current branch via plain git.
fn head_branch(dir: &Path) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn empty_commit(dir: &Path, msg: &str) {
    run(dir, &["commit", "--allow-empty", "-m", msg]);
}

/// Temp repo with a `develop` branch and a diverged `other` branch, HEAD left
/// on `other`. Default policy unless a policy file is written.
fn temp_repo_develop_other() -> (TempDir, ResolvedRepo) {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    run(path, &["init", "-b", "main"]);
    run(path, &["config", "user.name", "test"]);
    run(path, &["config", "user.email", "test@example.com"]);
    run(path, &["config", "commit.gpgsign", "false"]);
    empty_commit(path, "root");
    run(path, &["checkout", "-b", "develop"]);
    empty_commit(path, "on develop");
    run(path, &["checkout", "-b", "other"]);
    empty_commit(path, "on other"); // diverge other from develop
    let repo = ResolvedRepo::open(path, &[path.to_path_buf()]).unwrap();
    (dir, repo)
}

/// Write a workstream policy file into the repo root.
fn workstream_policy(dir: &Path) {
    std::fs::write(
        dir.join(".nxm-git-policy.toml"),
        r#"
protected_branches = ["main"]
integration_branches = ["integration/*"]
develop_direct_commit = "confirm"
develop_branch = "develop"
work_branch_prefix = "work/"
"#,
    )
    .unwrap();
}

#[test]
fn create_branch_roots_at_explicit_base_not_head() {
    let (dir, repo) = temp_repo_develop_other();
    // HEAD is on `other`; we branch explicitly from `develop`.
    assert_eq!(head_branch(dir.path()), "other");

    create_branch(&repo, "feature/x", Some("develop"), false, false).unwrap();

    // The new branch points at develop, NOT at the current HEAD (other).
    assert_eq!(
        rev(dir.path(), "feature/x"),
        rev(dir.path(), "develop"),
        "feature/x must be rooted at develop"
    );
    assert_ne!(
        rev(dir.path(), "feature/x"),
        rev(dir.path(), "other"),
        "feature/x must NOT be rooted at the current HEAD"
    );
}

#[test]
fn create_branch_default_base_is_develop_not_head() {
    let (dir, repo) = temp_repo_develop_other();
    // HEAD is on `other`; no base given → defaults to policy develop_branch.
    create_branch(&repo, "feature/y", None, false, false).unwrap();

    assert_eq!(
        rev(dir.path(), "feature/y"),
        rev(dir.path(), "develop"),
        "default base must be develop, not the current HEAD"
    );
    assert_ne!(rev(dir.path(), "feature/y"), rev(dir.path(), "other"));
}

#[test]
fn create_integration_branch_from_develop() {
    let (dir, repo) = temp_repo_develop_other();
    workstream_policy(dir.path());

    let res = create_integration_branch(&repo, "filone3", None, false).unwrap();
    assert_eq!(res.branch, "integration/filone3");
    assert_eq!(
        rev(dir.path(), "integration/filone3"),
        rev(dir.path(), "develop"),
        "integration branch must be rooted at develop"
    );
}

#[test]
fn create_integration_branch_rejects_duplicate() {
    let (dir, repo) = temp_repo_develop_other();
    workstream_policy(dir.path());
    create_integration_branch(&repo, "dup", None, false).unwrap();

    // A second create of the same name fails at the git layer (branch exists).
    let err = create_integration_branch(&repo, "dup", None, false).unwrap_err();
    assert!(
        matches!(err, WriteError::Git { .. }),
        "expected git failure on duplicate, got {err}"
    );
}

#[test]
fn create_integration_branch_rejects_slash_in_workstream() {
    let (dir, repo) = temp_repo_develop_other();
    workstream_policy(dir.path());

    let err = create_integration_branch(&repo, "a/b", None, false).unwrap_err();
    assert!(matches!(err, WriteError::Other(_)), "got {err}");
}

#[test]
fn worktree_add_from_integration_branch() {
    let (dir, repo) = temp_repo_develop_other();
    workstream_policy(dir.path());
    create_integration_branch(&repo, "ws", None, false).unwrap();

    let res: WorktreeResult = worktree_add(&repo, "ws", None, None).unwrap();
    assert_eq!(res.branch, "integration/ws");
    assert_eq!(res.path, ".worktrees/ws");
    assert!(
        dir.path().join(".worktrees/ws").is_dir(),
        "worktree directory must exist"
    );
}

#[test]
fn worktree_add_refuses_missing_branch() {
    let (dir, repo) = temp_repo_develop_other();
    workstream_policy(dir.path());
    // No integration branch created first.
    let err = worktree_add(&repo, "ws", None, None).unwrap_err();
    assert!(matches!(err, WriteError::Other(_)), "got {err}");
    assert!(!dir.path().join(".worktrees/ws").exists());
}

#[test]
fn worktree_add_refuses_path_outside_worktrees_root() {
    let (dir, repo) = temp_repo_develop_other();
    workstream_policy(dir.path());
    create_integration_branch(&repo, "ws", None, false).unwrap();

    let err = worktree_add(&repo, "ws", None, Some("elsewhere/ws")).unwrap_err();
    assert!(matches!(err, WriteError::Other(_)), "got {err}");
    assert!(!dir.path().join("elsewhere/ws").exists());
}

#[test]
fn worktree_add_refuses_existing_path() {
    let (dir, repo) = temp_repo_develop_other();
    workstream_policy(dir.path());
    create_integration_branch(&repo, "ws", None, false).unwrap();
    std::fs::create_dir_all(dir.path().join(".worktrees/ws")).unwrap();

    let err = worktree_add(&repo, "ws", None, None).unwrap_err();
    assert!(matches!(err, WriteError::Other(_)), "got {err}");
}
