//! Tests for `nxm_git::write` — policy-gated git write operations. Every test
//! runs against a throwaway `TempDir` worktree with a local bare remote; no
//! real repository or remote is ever touched.

use std::path::Path;
use std::process::Command;

use nxm_git::policy::DenialReason;
use nxm_git::{
    commit, create_branch, push, switch_branch, GitOp, PolicyDenied, ResolvedRepo, WriteError,
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

/// Current branch via plain git (replaces the crate-private `current_branch`).
fn head_branch(dir: &Path) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
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

#[test]
fn commit_on_main_without_confirm_is_denied_without_running_git() {
    let (repo_dir, _remote, repo) = temp_repo_with_remote();
    std::fs::write(repo_dir.path().join("a.txt"), "hello").unwrap();
    run(repo_dir.path(), &["add", "a.txt"]);

    let err = commit(&repo, "add a", false, false).unwrap_err();
    assert!(matches!(
        err,
        WriteError::Policy(PolicyDenied {
            reason: DenialReason::ProtectedBranch,
            op: GitOp::Commit,
            branch: Some(_),
        })
    ));

    // git never ran: HEAD must still be unborn.
    let out = Command::new("git")
        .current_dir(repo_dir.path())
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "HEAD is not unborn: git ran despite the policy denial"
    );
}

#[test]
fn commit_create_switch_and_push_roundtrip() {
    let (repo_dir, remote_dir, repo) = temp_repo_with_remote();

    // First commit on protected main with confirm.
    std::fs::write(repo_dir.path().join("a.txt"), "hello").unwrap();
    run(repo_dir.path(), &["add", "a.txt"]);
    let res = commit(&repo, "first", true, false).unwrap();
    assert_eq!(res.branch, "main");
    assert!(!res.sha.is_empty());

    // Create + switch (never restricted by the current rules).
    create_branch(&repo, "feature", Some("main"), false, false).unwrap();
    switch_branch(&repo, "feature", false, false).unwrap();
    assert_eq!(head_branch(repo_dir.path()), "feature");

    // Commit on the unprotected branch: no confirm needed.
    std::fs::write(repo_dir.path().join("b.txt"), "world").unwrap();
    run(repo_dir.path(), &["add", "b.txt"]);
    let res = commit(&repo, "second", false, false).unwrap();
    assert_eq!(res.branch, "feature");

    // Push the unprotected branch without confirm.
    push(&repo, None, false, false, false).unwrap();

    let remote_refs = refs(remote_dir.path());
    assert!(remote_refs.contains("refs/heads/feature"));
}

#[test]
fn push_on_main_is_denied_by_default_and_passes_with_confirm() {
    let (repo_dir, remote_dir, repo) = temp_repo_with_remote();
    shell_commit(repo_dir.path(), "seed");

    // Denied by default (main protected, no confirm).
    let err = push(&repo, None, false, false, false).unwrap_err();
    assert!(matches!(
        err,
        WriteError::Policy(PolicyDenied {
            reason: DenialReason::ProtectedBranch,
            op: GitOp::Push,
            ..
        })
    ));
    assert!(
        !refs(remote_dir.path()).contains("refs/heads/main"),
        "nothing must reach the remote on denial"
    );

    // Passes with confirm.
    push(&repo, None, false, true, false).unwrap();
    assert!(refs(remote_dir.path()).contains("refs/heads/main"));
}

#[test]
fn force_push_denied_without_allow_force_and_passes_with_both_flags() {
    let (repo_dir, remote_dir, repo) = temp_repo_with_remote();
    std::fs::write(repo_dir.path().join("a.txt"), "hello").unwrap();
    run(repo_dir.path(), &["add", "a.txt"]);
    commit(&repo, "first", true, false).unwrap();
    push(&repo, None, false, true, false).unwrap();

    // Diverge local main from the remote.
    run(repo_dir.path(), &["commit", "--amend", "-m", "amended"]);

    // Force without allow_force: denied even with confirm.
    let err = push(&repo, None, true, true, false).unwrap_err();
    assert!(matches!(
        err,
        WriteError::Policy(PolicyDenied {
            reason: DenialReason::ForceNotAllowed,
            op: GitOp::ForcePush,
            ..
        })
    ));

    // Force enabled but confirm missing: still denied.
    let err = push(&repo, None, true, false, true).unwrap_err();
    assert!(matches!(
        err,
        WriteError::Policy(PolicyDenied {
            reason: DenialReason::ForceNeedsConfirm,
            ..
        })
    ));

    // A plain (non-force) push now fails at the git level: non-FF.
    let err = push(&repo, None, false, true, false).unwrap_err();
    assert!(
        matches!(err, WriteError::Git { .. }),
        "expected git non-FF failure, got {err}"
    );

    // Force with allow_force + confirm passes.
    push(&repo, None, true, true, true).unwrap();
    let out = Command::new("git")
        .current_dir(remote_dir.path())
        .args(["log", "-1", "--format=%s", "main"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "amended");
}

#[test]
fn push_with_forbidden_content_is_refused() {
    let (repo_dir, remote_dir, repo) = temp_repo_with_remote();
    std::fs::write(
        repo_dir.path().join("leak.txt"),
        "build path /Users/secret/here",
    )
    .unwrap();
    run(repo_dir.path(), &["add", "leak.txt"]);
    commit(&repo, "leak", true, false).unwrap();

    // Scan gate refuses even with confirm.
    let err = push(&repo, None, false, true, false).unwrap_err();
    match err {
        WriteError::Forbidden(scan) => {
            assert!(!scan.clean);
            assert!(scan.hits.iter().any(|h| h.file == "leak.txt"));
        }
        other => panic!("expected Forbidden, got {other}"),
    }
    assert!(
        !refs(remote_dir.path()).contains("refs/heads/main"),
        "nothing must reach the remote on forbidden content"
    );
}

#[test]
fn push_rejects_url_or_path_remotes() {
    let (repo_dir, remote_dir, repo) = temp_repo_with_remote();
    shell_commit(repo_dir.path(), "seed");
    // Unprotected branch, so the policy gate cannot mask this check.
    create_branch(&repo, "feature", Some("main"), false, false).unwrap();
    switch_branch(&repo, "feature", false, false).unwrap();
    shell_commit(repo_dir.path(), "on feature");

    // A bare-repo path is a perfectly valid push target for git:
    // the tool must still refuse anything that is not a configured name.
    let remote_url = remote_dir.path().to_string_lossy().into_owned();
    let err = push(&repo, Some(&remote_url), false, true, false).unwrap_err();
    assert!(
        err.to_string().contains("configured"),
        "expected unknown-remote rejection, got {err}"
    );
    assert!(!refs(remote_dir.path()).contains("refs/heads/feature"));
}

#[test]
fn option_like_names_are_rejected_not_injected() {
    let (repo_dir, remote_dir, repo) = temp_repo_with_remote();
    shell_commit(repo_dir.path(), "seed");

    assert!(create_branch(&repo, "--force", None, false, false).is_err());
    assert!(switch_branch(&repo, "--force", false, false).is_err());
    assert!(push(&repo, Some("--push-option=x"), false, false, false).is_err());

    // The rejected names never reached git: the only local ref is main
    // (hex SHAs cannot contain "--"), and the remote stays empty.
    let all_refs = refs(repo_dir.path());
    assert!(
        all_refs.ends_with(" refs/heads/main"),
        "unexpected refs: {all_refs}"
    );
    assert!(
        !refs(remote_dir.path()).contains("refs/heads/main"),
        "remote must stay untouched"
    );
}
