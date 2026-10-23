//! Tests for `nxm_git::prepush` — the `require_pre_push` hard gate run by
//! `push`. Every test runs against a throwaway `TempDir` worktree with a local
//! bare remote; no real repository or remote is ever touched.

use std::path::{Path, PathBuf};
use std::process::Command;

use nxm_git::{create_branch, push, switch_branch, ResolvedRepo, WriteError};
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

/// List all refs in `dir`, one `sha refname` per line.
fn refs(dir: &Path) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["show-ref"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Temp worktree on branch `main` (default policy: main protected) with a
/// local bare `origin` remote, plus the bare dir itself.
fn temp_repo_with_remote() -> (TempDir, TempDir, ResolvedRepo) {
    let repo_dir = TempDir::new().unwrap();
    let remote_dir = TempDir::new().unwrap();
    let path = repo_dir.path();
    run(path, &["init", "-b", "main"]);
    run(path, &["config", "user.name", "test"]);
    run(path, &["config", "user.email", "test@example.com"]);
    run(path, &["config", "commit.gpgsign", "false"]);
    run(remote_dir.path(), &["init", "--bare", "-b", "main"]);
    let remote_url = remote_dir.path().to_string_lossy().into_owned();
    run(path, &["remote", "add", "origin", &remote_url]);
    let repo = ResolvedRepo::open(path, &[path.to_path_buf()]).unwrap();
    (repo_dir, remote_dir, repo)
}

/// Stage and commit an empty change via plain git (test setup only).
fn shell_commit(dir: &Path, msg: &str) {
    run(dir, &["commit", "--allow-empty", "-m", msg]);
}

/// Write a `.nxm-git-policy.toml` with only the given `require_pre_push`
/// commands (other fields fall back to the serde defaults).
fn set_pre_push(dir: &Path, commands: &[&str]) {
    let list = commands
        .iter()
        .map(|c| format!("{c:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        dir.join(".nxm-git-policy.toml"),
        format!("require_pre_push = [{list}]\n"),
    )
    .unwrap();
}

/// Seed a `feature` commit and return the path of a marker the pre-push
/// command will create.
fn feature_commit_with_marker(dir: &Path, repo: &ResolvedRepo) -> PathBuf {
    shell_commit(dir, "seed");
    create_branch(repo, "feature", Some("main"), false, false).unwrap();
    switch_branch(repo, "feature", false, false).unwrap();
    shell_commit(dir, "on feature");
    dir.join("prepush-ran")
}

#[test]
fn push_runs_pre_push_and_proceeds_when_ok() {
    let (repo_dir, remote_dir, repo) = temp_repo_with_remote();
    let marker = feature_commit_with_marker(repo_dir.path(), &repo);
    // A pre-push command that exits 0 must run and must not block the push.
    // Relative name (cwd is the repo): safe even if TMPDIR contains spaces.
    let name = marker.file_name().unwrap().to_string_lossy().into_owned();
    set_pre_push(repo_dir.path(), &[&format!("touch {name}")]);

    push(&repo, None, false, false, false).unwrap();

    assert!(marker.exists(), "pre-push command did not run");
    assert!(refs(remote_dir.path()).contains("refs/heads/feature"));
}

#[test]
fn push_blocked_when_pre_push_fails() {
    let (repo_dir, remote_dir, repo) = temp_repo_with_remote();
    feature_commit_with_marker(repo_dir.path(), &repo);
    set_pre_push(repo_dir.path(), &["false"]);

    let err = push(&repo, None, false, false, false).unwrap_err();
    assert!(
        matches!(err, WriteError::PrePushFailed(_)),
        "expected PrePushFailed, got {err}"
    );
    assert!(
        !refs(remote_dir.path()).contains("refs/heads/feature"),
        "push must not run when the pre-push gate fails"
    );
}

#[test]
fn empty_pre_push_is_noop() {
    let (repo_dir, remote_dir, repo) = temp_repo_with_remote();
    feature_commit_with_marker(repo_dir.path(), &repo);
    set_pre_push(repo_dir.path(), &[]);

    push(&repo, None, false, false, false).unwrap();

    assert!(refs(remote_dir.path()).contains("refs/heads/feature"));
}

#[test]
fn empty_command_string_is_noop() {
    let (repo_dir, remote_dir, repo) = temp_repo_with_remote();
    feature_commit_with_marker(repo_dir.path(), &repo);
    // An empty or whitespace-only entry carries no argv: it must be skipped,
    // not spawned (which would fail and block the push).
    set_pre_push(repo_dir.path(), &["", "   "]);

    push(&repo, None, false, false, false).unwrap();

    assert!(refs(remote_dir.path()).contains("refs/heads/feature"));
}
