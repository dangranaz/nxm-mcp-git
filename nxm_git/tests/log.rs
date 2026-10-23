//! Tests for `nxm_git::log` — seam: the public core API.
//!
//! Every test builds a real repository with known commits (fixed author,
//! subject and timestamp) so the expected values are literals, not a
//! restatement of how the implementation walks history.

use std::path::Path;
use std::process::Command;

use nxm_git::{log, ResolvedRepo};
use tempfile::TempDir;

/// One commit with fully known metadata.
struct Commit {
    subject: &'static str,
    /// Seconds since epoch — fixed so `CommitEntry::date` is assertable.
    time: i64,
}

const C1: Commit = Commit {
    subject: "first commit",
    time: 1_000_000,
};
const C2: Commit = Commit {
    subject: "second commit",
    time: 2_000_000,
};
const C3: Commit = Commit {
    subject: "third commit\n\nwith a body paragraph",
    time: 3_000_000,
};

/// A repository with `main` checked out and the given commits applied in order.
/// The returned SHAs are tagged with the subjects above, newest last.
fn repo_with(commits: &[Commit]) -> (TempDir, ResolvedRepo, Vec<String>) {
    let td = TempDir::new().expect("temp repo dir");
    // Both dates are pinned: `log` orders by commit time, and git otherwise
    // stamps every commit made in the same second with the same value.
    let git_at = |args: &[&str], when: i64| {
        let date = format!("@{when} +0000");
        let out = Command::new("git")
            .current_dir(td.path())
            .args(args)
            .env("GIT_AUTHOR_NAME", "Ada Lovelace")
            .env("GIT_AUTHOR_EMAIL", "ada@example.com")
            .env("GIT_COMMITTER_NAME", "Ada Lovelace")
            .env("GIT_COMMITTER_EMAIL", "ada@example.com")
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
    let git = |args: &[&str]| git_at(args, 1_700_000_000);

    git(&["init", "-b", "main", "-q"]);
    git(&["config", "user.name", "Ada Lovelace"]);
    git(&["config", "user.email", "ada@example.com"]);

    let mut shas = Vec::new();
    for (i, c) in commits.iter().enumerate() {
        let file = format!("file{i}.txt");
        std::fs::write(td.path().join(&file), format!("content of {}\n", c.subject))
            .expect("write file");
        git(&["add", &file]);
        git_at(&["commit", "-q", "-m", c.subject], c.time);
        let sha = Command::new("git")
            .current_dir(td.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse");
        shas.push(String::from_utf8_lossy(&sha.stdout).trim().to_string());
    }

    let repo = ResolvedRepo::open(td.path(), &[td.path().to_path_buf()]).expect("open repo");
    (td, repo, shas)
}

#[test]
fn log_returns_every_commit_newest_first() {
    let (_td, repo, shas) = repo_with(&[C1, C2, C3]);

    let res = log(&repo, None, None).expect("log succeeds");

    assert_eq!(
        res.commits.len(),
        3,
        "expected all three commits, got {:?}",
        res.commits
    );
    // Newest first.
    assert_eq!(
        res.commits
            .iter()
            .map(|c| c.sha.as_str())
            .collect::<Vec<_>>(),
        vec![shas[2].as_str(), shas[1].as_str(), shas[0].as_str()],
    );
}

#[test]
fn log_reports_author_email_and_timestamp_per_commit() {
    let (_td, repo, shas) = repo_with(&[C1, C2]);

    let res = log(&repo, None, None).expect("log succeeds");

    assert_eq!(res.commits[0].sha, shas[1]);
    assert_eq!(res.commits[0].author, "Ada Lovelace");
    assert_eq!(res.commits[0].author_email, "ada@example.com");
    assert_eq!(res.commits[0].date, C2.time);
    assert_eq!(res.commits[0].subject, "second commit");
}

#[test]
fn log_subject_is_the_first_line_only() {
    let (_td, repo, _shas) = repo_with(&[C3]);

    let res = log(&repo, None, None).expect("log succeeds");

    assert_eq!(
        res.commits[0].subject, "third commit",
        "body paragraph must not leak into the subject"
    );
}

#[test]
fn log_honours_limit() {
    let (_td, repo, shas) = repo_with(&[C1, C2, C3]);

    let res = log(&repo, None, Some(2)).expect("log succeeds");

    assert_eq!(res.commits.len(), 2);
    assert_eq!(res.commits[0].sha, shas[2]);
    assert_eq!(res.commits[1].sha, shas[1]);
}

#[test]
fn log_honours_range() {
    let (_td, repo, shas) = repo_with(&[C1, C2, C3]);

    // c1..c3 == everything reachable from c3 but not c1.
    let range = format!("{}..{}", shas[0], shas[2]);
    let res = log(&repo, Some(&range), None).expect("log succeeds");

    assert_eq!(
        res.commits
            .iter()
            .map(|c| c.sha.as_str())
            .collect::<Vec<_>>(),
        vec![shas[2].as_str(), shas[1].as_str()],
    );
}

#[test]
fn log_on_repo_without_commits_returns_empty() {
    let td = TempDir::new().expect("temp repo dir");
    let out = Command::new("git")
        .current_dir(td.path())
        .args(["init", "-b", "main", "-q"])
        .output()
        .expect("git init");
    assert!(out.status.success());
    let repo = ResolvedRepo::open(td.path(), &[td.path().to_path_buf()]).expect("open repo");

    let res = log(&repo, None, None).expect("log on empty repo is not an error");

    assert!(
        res.commits.is_empty(),
        "empty repo has no commits, got {:?}",
        res.commits
    );
}

#[test]
fn log_rejects_an_unknown_range() {
    let (_td, repo, _shas) = repo_with(&[C1]);

    let err = log(&repo, Some("no-such-ref..HEAD"), None).expect_err("unknown rev must error");

    assert!(!err.to_string().is_empty());
}

/// Guards the fixture itself: the SHAs the assertions rely on must be real,
/// distinct commits — otherwise a passing log test could be vacuous.
#[test]
fn fixture_commits_are_distinct_and_present() {
    let (_td, repo, shas) = repo_with(&[C1, C2]);
    assert_eq!(shas.len(), 2);
    assert_ne!(shas[0], shas[1]);
    let res = log(&repo, None, None).expect("log succeeds");
    assert_eq!(
        res.commits.len(),
        2,
        "fixture must yield two commits to attribute"
    );
    assert!(Path::new(&res.commits[0].sha).is_file() || res.commits[0].sha.len() == 40);
}
