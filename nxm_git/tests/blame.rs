//! Tests for `nxm_git::blame` — seam: the public core API.
//!
//! Blame is only meaningful if each line is attributed to the commit that last
//! touched it, so these tests build a two-commit history and assert the
//! attribution line by line, not merely that the output is non-empty.

use std::process::Command;

use nxm_git::{blame, ResolvedRepo};
use tempfile::TempDir;

/// A repository whose file `tracked.txt` is built up over two commits, plus an
/// uncommitted edit in the worktree (which must be blamed on the last commit).
///
/// After `initial`: tracked.txt is "alpha\nbeta\n".
/// After `second`:   tracked.txt is "alpha\nBETA\ngamma\n".
fn two_commit_repo() -> (TempDir, ResolvedRepo, String, String) {
    let td = TempDir::new().expect("temp repo dir");
    let git_as = |td: &TempDir, args: &[&str], when: i64, name: &str, email: &str| {
        let date = format!("@{when} +0000");
        let out = Command::new("git")
            .current_dir(td.path())
            .args(args)
            .env("GIT_AUTHOR_NAME", name)
            .env("GIT_AUTHOR_EMAIL", email)
            .env("GIT_COMMITTER_NAME", name)
            .env("GIT_COMMITTER_EMAIL", email)
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    let git = |args: &[&str], when: i64| git_as(&td, args, when, "Ada Lovelace", "ada@example.com");

    git(&["init", "-b", "main", "-q"], 1_600_000_000);
    git(&["config", "user.name", "Ada Lovelace"], 1_600_000_000);
    git(&["config", "user.email", "ada@example.com"], 1_600_000_000);

    std::fs::write(td.path().join("tracked.txt"), "alpha\nbeta\n").expect("write tracked.txt");
    git(&["add", "tracked.txt"], 1_600_000_000);
    git(&["commit", "-q", "-m", "initial"], 1_600_000_000);
    let first = git_output(&td, &["rev-parse", "HEAD"]);

    // Rewritten by a *different* author, so attribution cannot be satisfied by
    // hardcoding a single name.
    std::fs::write(td.path().join("tracked.txt"), "alpha\nBETA\ngamma\n").expect("rewrite");
    git_as(
        &td,
        &["add", "tracked.txt"],
        1_700_000_000,
        "Grace Hopper",
        "grace@example.com",
    );
    git_as(
        &td,
        &["commit", "-q", "-m", "second"],
        1_700_000_000,
        "Grace Hopper",
        "grace@example.com",
    );
    let second = git_output(&td, &["rev-parse", "HEAD"]);

    let repo = ResolvedRepo::open(td.path(), &[td.path().to_path_buf()]).expect("open repo");
    (td, repo, first, second)
}

fn git_output(td: &TempDir, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(td.path())
        .args(args)
        .output()
        .expect("run git");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn blame_returns_one_line_per_source_line() {
    let (_td, repo, _first, _second) = two_commit_repo();

    let res = blame(&repo, "tracked.txt").expect("blame succeeds");

    assert_eq!(
        res.lines.len(),
        3,
        "expected 3 blamed lines, got {:?}",
        res.lines
    );
    assert_eq!(
        res.lines.iter().map(|l| l.line_no).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "line numbers are 1-indexed and contiguous"
    );
}

#[test]
fn blame_reports_exact_content_of_each_line() {
    let (_td, repo, _first, _second) = two_commit_repo();

    let res = blame(&repo, "tracked.txt").expect("blame succeeds");

    assert_eq!(res.lines[0].content, "alpha");
    assert_eq!(res.lines[1].content, "BETA");
    assert_eq!(res.lines[2].content, "gamma");
}

#[test]
fn blame_attributes_untouched_line_to_the_first_commit() {
    let (_td, repo, first, _second) = two_commit_repo();

    let res = blame(&repo, "tracked.txt").expect("blame succeeds");

    assert_eq!(
        res.lines[0].commit, first,
        "line 1 ('alpha') was only ever written by the initial commit"
    );
    assert_eq!(res.lines[0].author, "Ada Lovelace");
}

#[test]
fn blame_attributes_modified_lines_to_the_second_commit() {
    let (_td, repo, _first, second) = two_commit_repo();

    let res = blame(&repo, "tracked.txt").expect("blame succeeds");

    assert_eq!(
        res.lines[1].commit, second,
        "line 2 ('BETA') was rewritten by the second commit"
    );
    assert_eq!(
        res.lines[2].commit, second,
        "line 3 ('gamma') was added by the second commit"
    );
    assert_eq!(res.lines[1].author, "Grace Hopper");
    assert_eq!(res.lines[2].author, "Grace Hopper");
}

#[test]
fn blame_reports_a_missing_file_as_an_error() {
    let (_td, repo, _first, _second) = two_commit_repo();

    let err = blame(&repo, "no-such-file.txt").expect_err("blame of a missing file must fail");

    assert!(!err.to_string().is_empty());
}

#[test]
fn blame_covers_every_line_of_the_worktree_file_including_uncommitted_ones() {
    let (td, repo, _first, second) = two_commit_repo();
    // Append an uncommitted line: the file is now 4 lines on disk, but only 3
    // are in any commit. Blame must still report all 4 — a worktree file is
    // what the caller asked about, and dropping trailing lines would silently
    // hide uncommitted work.
    std::fs::write(
        td.path().join("tracked.txt"),
        "alpha\nBETA\ngamma\nuncommitted\n",
    )
    .expect("append uncommitted line");

    let res = blame(&repo, "tracked.txt").expect("blame succeeds");

    assert_eq!(
        res.lines.len(),
        4,
        "every worktree line must be blamed, got {:?}",
        res.lines
    );
    assert_eq!(res.lines[3].line_no, 4);
    assert_eq!(res.lines[3].content, "uncommitted");
    assert_eq!(
        res.lines[3].commit, second,
        "an uncommitted line has no commit of its own; it is attributed to the last commit"
    );
}

#[test]
fn blame_of_an_empty_file_returns_no_lines() {
    let td = TempDir::new().expect("temp dir");
    let out = Command::new("git")
        .current_dir(td.path())
        .args(["init", "-b", "main", "-q"])
        .output()
        .expect("git init");
    assert!(out.status.success());
    std::fs::write(td.path().join("empty.txt"), "").expect("write empty file");
    for args in [
        vec!["add", "empty.txt"],
        vec![
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@e.com",
            "commit",
            "-q",
            "-m",
            "empty",
        ],
    ] {
        let o = Command::new("git")
            .current_dir(td.path())
            .args(&args)
            .output()
            .expect("run git");
        assert!(o.status.success(), "git {args:?} failed");
    }
    let repo = ResolvedRepo::open(td.path(), &[td.path().to_path_buf()]).expect("open repo");

    let res = blame(&repo, "empty.txt").expect("blame of an empty file succeeds");

    assert!(res.lines.is_empty(), "an empty file has no lines to blame");
}
