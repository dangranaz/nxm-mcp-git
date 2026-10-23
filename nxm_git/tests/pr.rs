//! Tests for `nxm_git::pr::pr_create` — seam: the public core API.
//!
//! `gh` is never real: a fake `gh` script shadows it on PATH, records the argv
//! it received, and prints a PR URL. No network, no real repository.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::sync::Mutex;

use nxm_git::{pr_create, ResolvedRepo};
use tempfile::TempDir;

/// Serializes every test that mutates PATH: PATH is process-global, so two
/// tests overlapping would drop each other's fake `gh` out of the search path.
static PATH_LOCK: Mutex<()> = Mutex::new(());

/// Shell-quote `s` so any payload can be embedded literally in the fake `gh`.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Install a fake `gh` in `dir`: it writes its argv (one element per line) to
/// `dir/argv.txt`, prints `stdout`, writes `stderr`, exits with `code`.
fn fake_gh(dir: &Path, stdout: &str, stderr: &str, code: u8) {
    let argv_file = sh_quote(&dir.join("argv.txt").to_string_lossy());
    let script = format!(
        "#!/bin/sh\n\
         printf '%s\\n' \"$@\" > {argv_file}\n\
         printf %s {stdout}\n\
         printf %s {stderr} >&2\n\
         exit {code}\n",
        argv_file = argv_file,
        stdout = sh_quote(stdout),
        stderr = sh_quote(stderr),
        code = code,
    );
    let path = dir.join("gh");
    fs::write(&path, script).expect("write fake gh");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod fake gh");
}

/// Prepend `dir` to PATH for the lifetime of the test; restored on drop.
struct PathGuard(Option<String>);

impl Drop for PathGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.0 {
            std::env::set_var("PATH", previous);
        }
    }
}

fn shadow_gh(dir: &Path) -> PathGuard {
    let previous = std::env::var("PATH").ok();
    let prefix = match &previous {
        Some(p) => format!("{}:{}", dir.display(), p),
        None => dir.display().to_string(),
    };
    std::env::set_var("PATH", prefix);
    PathGuard(previous)
}

fn temp_repo() -> (TempDir, ResolvedRepo) {
    let td = TempDir::new().expect("temp repo dir");
    let out = std::process::Command::new("git")
        .current_dir(td.path())
        .args(["init", "-b", "main"])
        .output()
        .expect("run git init");
    assert!(out.status.success(), "git init failed");
    let repo = ResolvedRepo::open(td.path(), &[td.path().to_path_buf()]).expect("open repo");
    (td, repo)
}

#[test]
fn pr_create_runs_gh_with_exact_argv_and_returns_url() {
    let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let gh_dir = TempDir::new().expect("fake gh dir");
    fake_gh(
        gh_dir.path(),
        "https://github.com/draco690456/nxm-git/pull/42\n",
        "",
        0,
    );
    let _guard = shadow_gh(gh_dir.path());
    let (_repo_dir, repo) = temp_repo();

    let title = "feat: add git_pr_create";
    let body = "Quotes 'single' and shell metacharacters stay one argv element: $(rm -rf /)";
    let res =
        pr_create(&repo, title, body, "main", true).expect("pr_create succeeds against fake gh");

    assert_eq!(res.url, "https://github.com/draco690456/nxm-git/pull/42");

    let recorded = fs::read_to_string(gh_dir.path().join("argv.txt")).expect("argv recorded");
    let args: Vec<&str> = recorded.lines().collect();
    assert_eq!(
        args,
        ["pr", "create", "--title", title, "--body", body, "--base", "main"]
    );
    assert!(
        !args.iter().any(|a| a.contains("merge")),
        "auto-merge must never reach gh: {args:?}"
    );
}

#[test]
fn pr_create_rejects_option_like_base_before_spawning_gh() {
    let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let gh_dir = TempDir::new().expect("fake gh dir");
    fake_gh(gh_dir.path(), "https://example.invalid/pr/1\n", "", 0);
    let _guard = shadow_gh(gh_dir.path());
    let (_repo_dir, repo) = temp_repo();

    let err = pr_create(&repo, "feat: x", "body", "--draft", true).expect_err("option-like base");
    assert!(
        matches!(&err, nxm_git::PrError::Other(_)),
        "expected a structured argument error, got {err:?}"
    );
    assert!(err.to_string().contains("invalid git name"), "{err}");

    let err = pr_create(&repo, "feat: x", "body", "", true).expect_err("empty base");
    assert!(err.to_string().contains("invalid git name"), "{err}");

    assert!(
        !gh_dir.path().join("argv.txt").exists(),
        "gh must never run when the base is rejected"
    );
}

#[test]
fn pr_create_reports_gh_failure_as_structured_error() {
    let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let gh_dir = TempDir::new().expect("fake gh dir");
    fake_gh(gh_dir.path(), "", "gh: no git remotes found for origin", 1);
    let _guard = shadow_gh(gh_dir.path());
    let (_repo_dir, repo) = temp_repo();

    let err = pr_create(&repo, "feat: x", "body", "main", true).expect_err("gh exits non-zero");
    let nxm_git::PrError::Gh { args, stderr } = &err else {
        panic!("expected PrError::Gh, got {err:?}");
    };
    assert_eq!(
        args,
        &["pr", "create", "--title", "feat: x", "--body", "body", "--base", "main"]
    );
    assert_eq!(stderr, "gh: no git remotes found for origin");

    let payload = err.to_value();
    assert_eq!(payload["kind"], "gh_error");
    assert_eq!(payload["stderr"], "gh: no git remotes found for origin");
}

#[test]
fn pr_create_rejects_empty_title_before_spawning_gh() {
    let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let gh_dir = TempDir::new().expect("fake gh dir");
    fake_gh(gh_dir.path(), "https://example.invalid/pr/1\n", "", 0);
    let _guard = shadow_gh(gh_dir.path());
    let (_repo_dir, repo) = temp_repo();

    let err = pr_create(&repo, "", "body", "main", true).expect_err("empty title");
    assert!(err.to_string().contains("title"), "{err}");
    assert_eq!(err.to_value()["kind"], "error");
    assert!(
        !gh_dir.path().join("argv.txt").exists(),
        "gh must never run when the title is rejected"
    );
}

#[test]
fn pr_create_errors_when_gh_prints_no_url() {
    let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let gh_dir = TempDir::new().expect("fake gh dir");
    // Succeeds, but stdout carries no URL (e.g. gh talking to a pager/editor).
    fake_gh(gh_dir.path(), "\nRendering hints for the terminal\n", "", 0);
    let _guard = shadow_gh(gh_dir.path());
    let (_repo_dir, repo) = temp_repo();

    let err = pr_create(&repo, "feat: x", "body", "main", true).expect_err("no URL on stdout");
    assert!(err.to_string().contains("no PR URL"), "{err}");
    assert_eq!(err.to_value()["kind"], "error");
}

#[test]
fn pr_create_denied_on_protected_base_without_confirm_before_spawning_gh() {
    let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let gh_dir = TempDir::new().expect("fake gh dir");
    fake_gh(
        gh_dir.path(),
        "https://github.com/draco690456/nxm-git/pull/1\n",
        "",
        0,
    );
    let _guard = shadow_gh(gh_dir.path());
    let (_repo_dir, repo) = temp_repo();

    // Default policy protects "main": opening a PR against it needs confirm.
    let err = pr_create(&repo, "feat: x", "body", "main", false)
        .expect_err("protected base without confirm");
    assert!(
        matches!(&err, nxm_git::PrError::Policy(_)),
        "expected a policy denial, got {err:?}"
    );
    assert_eq!(err.to_value()["kind"], "policy_denied");
    assert!(
        !gh_dir.path().join("argv.txt").exists(),
        "gh must never run when the policy denies the PR"
    );

    // The same PR against an unprotected base is allowed without confirm.
    let res = pr_create(&repo, "feat: x", "body", "dev", false)
        .expect("unprotected base needs no confirm");
    assert_eq!(res.url, "https://github.com/draco690456/nxm-git/pull/1");
}

#[test]
fn pr_create_extracts_pr_url_despite_noise_and_ignores_non_pr_lines() {
    let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let gh_dir = TempDir::new().expect("fake gh dir");
    // Real-ish gh output: progress noise, a non-TLS and a non-PR https line
    // first, then the actual PR URL last. Only the PR URL must be returned.
    fake_gh(
        gh_dir.path(),
        "Creating pull request for feature into main\n\
         http://github.com/draco690456/nxm-git/pull/7\n\
         https://github.com/draco690456/nxm-git/tree/main\n\
         https://github.com/draco690456/nxm-git/pull/99\n",
        "",
        0,
    );
    let _guard = shadow_gh(gh_dir.path());
    let (_repo_dir, repo) = temp_repo();

    let res = pr_create(&repo, "feat: x", "body", "main", true).expect("pr_create succeeds");
    assert_eq!(
        res.url, "https://github.com/draco690456/nxm-git/pull/99",
        "must take the last HTTPS /pull/ line, ignoring noise and non-PR URLs"
    );
}
