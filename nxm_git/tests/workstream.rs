//! Integration tests for workstream enforcement wired into `write.rs`.
//! Every test runs against a throwaway `TempDir` repo; no real repository
//! is ever touched.

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

/// Current branch via plain git.
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

/// Stage and commit an empty change via plain git (test setup only).
fn empty_commit(dir: &Path, msg: &str) {
    run(dir, &["commit", "--allow-empty", "-m", msg]);
}

/// Temp repo on `main` with default policy (main protected, no workstream
/// guards unless a policy file is written).
fn temp_repo() -> (TempDir, ResolvedRepo) {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    run(path, &["init", "-b", "main"]);
    run(path, &["config", "user.name", "test"]);
    run(path, &["config", "user.email", "test@example.com"]);
    run(path, &["config", "commit.gpgsign", "false"]);
    let repo = ResolvedRepo::open(path, &[path.to_path_buf()]).unwrap();
    (dir, repo)
}

/// Write a workstream policy file into the repo root.
fn workstream_policy(dir: &Path, toml: &str) {
    std::fs::write(dir.join(".nxm-git-policy.toml"), toml).unwrap();
}

/// Policy activating the workstream guards: `integration/*` landing zones,
/// direct commits on `develop` require confirm.
fn workstream_policy_confirm(dir: &Path) {
    workstream_policy(
        dir,
        r#"
protected_branches = ["main"]
integration_branches = ["integration/*"]
develop_direct_commit = "confirm"
develop_branch = "develop"
work_branch_prefix = "work/"
"#,
    );
}

/// Policy without any integration branches: new guards must stay off.
fn no_integration_policy(dir: &Path) {
    workstream_policy(
        dir,
        r#"
protected_branches = ["main"]
integration_branches = []
"#,
    );
}

#[test]
fn commit_direct_on_develop_denied() {
    let (dir, repo) = temp_repo();
    workstream_policy_confirm(dir.path());
    run(dir.path(), &["checkout", "-b", "develop"]);
    empty_commit(dir.path(), "seed develop");

    // A plain (non-merge) commit on develop: source_branch = None → G1
    // applies develop_direct_commit = Confirm, confirm = false → denied.
    let err = commit(&repo, "direct on develop", false, false).unwrap_err();
    assert!(
        matches!(
            err,
            WriteError::Policy(PolicyDenied {
                reason: DenialReason::DevelopDirectCommit,
                op: GitOp::Commit,
                branch: Some(_),
            })
        ),
        "expected DevelopDirectCommit denial, got {err}"
    );

    // git never ran: HEAD must not have advanced.
    let before = head_branch(dir.path());
    let out = Command::new("git")
        .current_dir(dir.path())
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .unwrap();
    assert!(out.status.success(), "HEAD must still exist");
    assert_eq!(
        head_branch(dir.path()),
        before,
        "HEAD must not have advanced"
    );
}

#[test]
fn commit_direct_on_develop_with_confirm_ok() {
    let (dir, repo) = temp_repo();
    workstream_policy_confirm(dir.path());
    run(dir.path(), &["checkout", "-b", "develop"]);
    empty_commit(dir.path(), "seed develop");
    // A real change so the second commit has something to commit.
    std::fs::write(dir.path().join("x.txt"), "x").unwrap();
    run(dir.path(), &["add", "x.txt"]);

    // confirm = true → G1 allows the direct commit.
    let res = commit(&repo, "direct on develop", true, false).unwrap();
    assert_eq!(res.branch, "develop");
    assert!(!res.sha.is_empty());
}

#[test]
fn merge_from_integration_into_develop_ok() {
    let (dir, repo) = temp_repo();
    workstream_policy_confirm(dir.path());
    run(dir.path(), &["checkout", "-b", "develop"]);
    empty_commit(dir.path(), "seed develop");
    run(dir.path(), &["checkout", "-b", "integration/x"]);
    empty_commit(dir.path(), "on integration");
    run(dir.path(), &["checkout", "develop"]);

    // Start a real merge (not yet committed): MERGE_HEAD is present.
    // `--no-ff` forces a merge commit even though integration/x is a child of
    // develop, so MERGE_HEAD is created and the source is resolvable.
    run(
        dir.path(),
        &["merge", "--no-ff", "--no-commit", "integration/x"],
    );

    // The commit completes the merge; source_branch resolves to integration/x
    // → G1 allows it even without confirm.
    let res = commit(&repo, "merge integration/x", false, false).unwrap();
    assert_eq!(res.branch, "develop");
    assert!(!res.sha.is_empty());
    assert_eq!(head_branch(dir.path()), "develop");
}

#[test]
fn create_work_branch_from_main_denied() {
    let (dir, repo) = temp_repo();
    workstream_policy_confirm(dir.path());
    // HEAD is on `main` (default branch of temp_repo).

    // Explicit base `main`: a work branch rooted on main is refused by G2.
    let err = create_branch(&repo, "work/09-x", Some("main"), false, false).unwrap_err();
    assert!(matches!(
        err,
        WriteError::Policy(PolicyDenied {
            reason: DenialReason::WorkBranchBaseNotAllowed,
            op: GitOp::CreateBranch,
            ..
        })
    ));

    // The branch must not have been created.
    assert!(
        !refs(dir.path()).contains("refs/heads/work/09-x"),
        "work branch must not be created on denial"
    );
}

#[test]
fn create_work_branch_from_integration_ok() {
    let (dir, repo) = temp_repo();
    workstream_policy_confirm(dir.path());
    run(dir.path(), &["checkout", "-b", "integration/foo"]);
    empty_commit(dir.path(), "seed integration");

    let res = create_branch(&repo, "work/09-x", Some("integration/foo"), false, false).unwrap();
    assert_eq!(res.branch, "work/09-x");
    assert!(refs(dir.path()).contains("refs/heads/work/09-x"));
}

#[test]
fn no_integration_branches_commit_on_develop_ok() {
    let (dir, repo) = temp_repo();
    no_integration_policy(dir.path());
    run(dir.path(), &["checkout", "-b", "develop"]);
    empty_commit(dir.path(), "seed develop");
    std::fs::write(dir.path().join("x.txt"), "x").unwrap();
    run(dir.path(), &["add", "x.txt"]);

    // integration_branches empty → G1 disabled → commit on develop proceeds
    // exactly as before the workstream enforcement.
    let res = commit(&repo, "on develop", false, false).unwrap();
    assert_eq!(res.branch, "develop");
    assert!(!res.sha.is_empty());
}

/// R1: worktree secondario — il merge in corso va risolto via `git rev-parse
/// --git-path MERGE_HEAD` (worktree-safe), non via `.git/MERGE_HEAD` hard-coded.
#[test]
fn merge_from_integration_in_worktree_ok() {
    let (dir, _repo) = temp_repo();
    workstream_policy_confirm(dir.path());
    run(dir.path(), &["checkout", "-b", "develop"]);
    empty_commit(dir.path(), "seed develop");
    run(dir.path(), &["checkout", "-b", "integration/x"]);
    empty_commit(dir.path(), "on integration");
    // Libera `develop` nel worktree principale così il secondario può usarlo.
    run(dir.path(), &["checkout", "-b", "temp"]);

    // Worktree secondario sullo stesso repo, su `develop`.
    let wt_dir = TempDir::new().unwrap();
    run(
        dir.path(),
        &[
            "worktree",
            "add",
            wt_dir.path().to_str().unwrap(),
            "develop",
        ],
    );
    // La policy deve essere visibile anche nel worktree (commit la carica da lì).
    workstream_policy_confirm(wt_dir.path());

    // Merge in corso NEL WORKTREE: MERGE_HEAD vive in
    // `.git/worktrees/<nome>/MERGE_HEAD`, non in `<wt>/.git/MERGE_HEAD`.
    run(
        wt_dir.path(),
        &["merge", "--no-ff", "--no-commit", "integration/x"],
    );

    // ResolvedRepo aperto sul PATH DEL WORKTREE (il suo `.git` è un file
    // `gitdir: ...`, quindi il path hard-coded fallisce).
    let wt_repo = ResolvedRepo::open(wt_dir.path(), &[wt_dir.path().to_path_buf()]).unwrap();

    // source_branch deve risolversi a `integration/x` → G1 passa senza confirm.
    let res = commit(&wt_repo, "merge integration/x in worktree", false, false).unwrap();
    assert_eq!(res.branch, "develop");
    assert!(!res.sha.is_empty());
    assert_eq!(head_branch(wt_dir.path()), "develop");
}

#[test]
fn switch_and_push_unaffected() {
    let (dir, repo) = temp_repo();
    workstream_policy_confirm(dir.path());
    run(dir.path(), &["checkout", "-b", "develop"]);
    empty_commit(dir.path(), "seed develop");

    // switch passes source_branch = the base: no new denial for a non-work branch.
    create_branch(&repo, "feature", Some("develop"), false, false).unwrap();
    switch_branch(&repo, "feature", false, false).unwrap();
    assert_eq!(head_branch(dir.path()), "feature");

    // push on an unprotected branch passes source_branch = None: no new denial.
    // A local bare remote is required for push to have a target.
    let remote_dir = TempDir::new().unwrap();
    run(remote_dir.path(), &["init", "--bare", "-b", "main"]);
    let remote_url = remote_dir.path().to_string_lossy().into_owned();
    run(dir.path(), &["remote", "add", "origin", &remote_url]);
    push(&repo, None, false, false, false).unwrap();
    assert!(refs(dir.path()).contains("refs/heads/feature"));
    assert!(refs(remote_dir.path()).contains("refs/heads/feature"));
}
