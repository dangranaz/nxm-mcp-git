//! Tests for `nxm_git::diff` — seam: the public core API.
//!
//! The counts must be exact per line, not hunk-header approximations, so these
//! tests use edits with known +/- shapes (including the cases where the two
//! differ: a hunk with context, and a change where lines are only reordered).

use std::process::Command;

use nxm_git::{diff, ResolvedRepo};
use tempfile::TempDir;

/// A repo with one committed file `f.txt` containing `initial\n`, plus the
/// SHA of that commit.
fn committed_repo() -> (TempDir, ResolvedRepo, String) {
    let td = TempDir::new().expect("temp repo dir");
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .current_dir(td.path())
            .args(args)
            .env("GIT_AUTHOR_NAME", "Ada")
            .env("GIT_AUTHOR_EMAIL", "ada@example.com")
            .env("GIT_COMMITTER_NAME", "Ada")
            .env("GIT_COMMITTER_EMAIL", "ada@example.com")
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["init", "-b", "main", "-q"]);
    git(&["config", "user.name", "Ada"]);
    git(&["config", "user.email", "ada@example.com"]);
    std::fs::write(td.path().join("f.txt"), "initial\n").expect("write f.txt");
    git(&["add", "f.txt"]);
    git(&["commit", "-q", "-m", "initial"]);
    let sha = String::from_utf8_lossy(
        &Command::new("git")
            .current_dir(td.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse")
            .stdout,
    )
    .trim()
    .to_string();
    let repo = ResolvedRepo::open(td.path(), &[td.path().to_path_buf()]).expect("open repo");
    (td, repo, sha)
}

/// The per-file counts for `path`, panicking if it is absent.
fn counts_for(result: &nxm_git::diff::DiffResult, path: &str) -> (u32, u32) {
    let file = result
        .files
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("{path} missing from {:?}", result.files));
    (file.additions, file.deletions)
}

#[test]
fn diff_counts_a_pure_insertion_exactly() {
    let (td, repo, _sha) = committed_repo();
    // "initial" is unchanged, so it is context; only the three new lines count.
    std::fs::write(td.path().join("f.txt"), "initial\ntwo\nthree\nfour\n").expect("edit");

    let res = diff(&repo, None, None).expect("diff succeeds");

    assert_eq!(counts_for(&res, "f.txt"), (3, 0));
    assert_eq!(res.total_additions, 3);
    assert_eq!(res.total_deletions, 0);
}

/// The counts must agree with git itself. `git diff --numstat` is the
/// independent oracle: if the implementation and real git disagree, this fails.
#[test]
fn diff_agrees_with_git_numstat() {
    let cases: Vec<(&str, &str)> = vec![
        ("initial\n", "initial\ntwo\nthree\nfour\n"),
        ("a\nb\nc\n", "a\nB\nc\n"),
        ("a\nb\nc\nd\ne\n", "e\nd\nc\nb\na\n"),
        ("x\ny\n", "x\n"),
        ("", "brand new\n"),
    ];

    for (before, after) in cases {
        let (td, repo, _sha) = committed_repo();
        std::fs::write(td.path().join("f.txt"), before).expect("seed");
        std::fs::write(td.path().join("f.txt"), after).expect("edit");

        let git = Command::new("git")
            .current_dir(td.path())
            .args(["diff", "--numstat"])
            .output()
            .expect("run git");
        assert!(git.status.success());
        let oracle = String::from_utf8_lossy(&git.stdout);
        let mut parts = oracle.split_whitespace();
        let want_add: u32 = parts.next().unwrap_or("0").parse().expect("git additions");
        let want_del: u32 = parts.next().unwrap_or("0").parse().expect("git deletions");

        let res = diff(&repo, None, None).expect("diff succeeds");
        assert_eq!(
            (res.total_additions, res.total_deletions),
            (want_add, want_del),
            "counts disagree with git --numstat for {before:?} -> {after:?}"
        );
    }
}

#[test]
fn diff_counts_a_pure_deletion_exactly() {
    let (td, repo, _sha) = committed_repo();
    // 1 line -> 0 lines
    std::fs::write(td.path().join("f.txt"), "").expect("truncate");

    let res = diff(&repo, None, None).expect("diff succeeds");

    assert_eq!(counts_for(&res, "f.txt"), (0, 1));
    assert_eq!(res.total_additions, 0);
    assert_eq!(res.total_deletions, 1);
}

#[test]
fn diff_does_not_count_context_lines() {
    let (td, repo, _sha) = committed_repo();
    // Ten lines, only line 5 replaced. A hunk-header approximation would report
    // the whole hunk; per-line counting must report +1/-1.
    let original: String = (1..=10).map(|i| format!("line{i}\n")).collect();
    std::fs::write(td.path().join("f.txt"), &original).expect("seed");
    // Commit that ten-line version so the next diff is a one-line change.
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .current_dir(td.path())
            .args(args)
            .env("GIT_AUTHOR_NAME", "Ada")
            .env("GIT_AUTHOR_EMAIL", "ada@example.com")
            .env("GIT_COMMITTER_NAME", "Ada")
            .env("GIT_COMMITTER_EMAIL", "ada@example.com")
            .output()
            .expect("run git");
        assert!(out.status.success(), "git {args:?} failed");
    };
    git(&["add", "f.txt"]);
    git(&["commit", "-q", "-m", "ten lines"]);

    std::fs::write(
        td.path().join("f.txt"),
        "line1\nline2\nline3\nline4\nCHANGED\nline6\nline7\nline8\nline9\nline10\n",
    )
    .expect("edit");

    let res = diff(&repo, None, None).expect("diff succeeds");

    assert_eq!(
        counts_for(&res, "f.txt"),
        (1, 1),
        "context lines must not be counted; only the replaced line is +1/-1"
    );
}

#[test]
fn diff_totals_sum_the_per_file_counts() {
    let (td, repo, _sha) = committed_repo();
    std::fs::write(td.path().join("f.txt"), "a\nb\nc\n").expect("edit f.txt");
    std::fs::write(td.path().join("g.txt"), "new file\n").expect("write g.txt");

    let res = diff(&repo, None, None).expect("diff succeeds");

    let sum_add: u32 = res.files.iter().map(|f| f.additions).sum();
    let sum_del: u32 = res.files.iter().map(|f| f.deletions).sum();
    assert_eq!(
        res.total_additions, sum_add,
        "totals must equal the sum over files"
    );
    assert_eq!(
        res.total_deletions, sum_del,
        "totals must equal the sum over files"
    );
}

#[test]
fn diff_between_two_commits_counts_exactly() {
    let td = TempDir::new().expect("temp dir");
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .current_dir(td.path())
            .args(args)
            .env("GIT_AUTHOR_NAME", "Ada")
            .env("GIT_AUTHOR_EMAIL", "ada@example.com")
            .env("GIT_COMMITTER_NAME", "Ada")
            .env("GIT_COMMITTER_EMAIL", "ada@example.com")
            .output()
            .expect("run git");
        assert!(out.status.success(), "git {args:?} failed");
    };
    git(&["init", "-b", "main", "-q"]);
    git(&["config", "user.name", "Ada"]);
    git(&["config", "user.email", "ada@example.com"]);
    std::fs::write(td.path().join("f.txt"), "one\n").expect("write");
    git(&["add", "f.txt"]);
    git(&["commit", "-q", "-m", "c1"]);
    let first = String::from_utf8_lossy(
        &Command::new("git")
            .current_dir(td.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .trim()
    .to_string();
    std::fs::write(td.path().join("f.txt"), "one\ntwo\nthree\n").expect("extend");
    git(&["add", "f.txt"]);
    git(&["commit", "-q", "-m", "c2"]);
    let second = String::from_utf8_lossy(
        &Command::new("git")
            .current_dir(td.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .trim()
    .to_string();

    let repo = ResolvedRepo::open(td.path(), &[td.path().to_path_buf()]).expect("open repo");

    let res = diff(&repo, Some(&first), Some(&second)).expect("diff between commits");

    assert_eq!(
        counts_for(&res, "f.txt"),
        (2, 0),
        "one -> three lines is +2/-0"
    );
    assert_eq!(res.total_additions, 2);
    assert_eq!(res.total_deletions, 0);
}

#[test]
fn diff_of_identical_commits_is_empty() {
    let (_td, repo, sha) = committed_repo();

    let res = diff(&repo, Some(&sha), Some(&sha)).expect("diff of a ref against itself");

    assert!(
        res.files.is_empty(),
        "no change means no files, got {:?}",
        res.files
    );
    assert_eq!(res.total_additions, 0);
    assert_eq!(res.total_deletions, 0);
}
