//! Tests for `nxm_git::scan_forbidden` honoring `scan_exclude`. Each test runs
//! against a throwaway `TempDir` repo committed with plain git; no real
//! repository is ever touched. The forbidden-content detection itself must
//! stay intact for every file that is NOT excluded.

use std::path::Path;
use std::process::Command;

use nxm_git::scan::path_matches_exclude;
use nxm_git::{scan_forbidden, Policy, ResolvedRepo};
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

/// A temp repo with `files` (relative path → content) committed on `main`,
/// resolved against itself as the sole allowed root.
fn repo_with(files: &[(&str, &str)]) -> (TempDir, ResolvedRepo) {
    let dir = TempDir::new().unwrap();
    let path = dir.path();
    run(path, &["init", "-b", "main"]);
    run(path, &["config", "user.name", "test"]);
    run(path, &["config", "user.email", "test@example.com"]);
    for (rel, content) in files {
        let full = path.join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full, content).unwrap();
    }
    run(path, &["add", "-A"]);
    run(path, &["commit", "-m", "seed"]);
    let canonical = std::fs::canonicalize(path).unwrap();
    let repo = ResolvedRepo::open(
        canonical.to_str().unwrap(),
        std::slice::from_ref(&canonical),
    )
    .unwrap();
    (dir, repo)
}

#[test]
fn scan_skips_excluded_paths() {
    // A forbidden token lives only in an excluded file → no hits.
    let (_dir, repo) = repo_with(&[("nxm_git/src/scan.rs", "const P = \"gho_realtoken\";\n")]);
    let excludes = vec!["nxm_git/src/scan.rs".to_string()];
    let res = scan_forbidden(&repo, None, &excludes).unwrap();
    assert!(
        res.clean,
        "excluded file must not produce hits: {:?}",
        res.hits
    );
}

#[test]
fn scan_still_flags_real_secret_in_non_excluded_file() {
    // The SAME token in a normal source file is still a hit.
    let (_dir, repo) = repo_with(&[
        ("nxm_git/src/scan.rs", "// defines patterns\n"),
        ("src/config.rs", "let t = \"gho_realtoken\";\n"),
    ]);
    let excludes = vec!["nxm_git/src/scan.rs".to_string(), "**/tests/**".to_string()];
    let res = scan_forbidden(&repo, None, &excludes).unwrap();
    assert!(
        !res.clean,
        "a real secret in a non-excluded file must be flagged"
    );
    assert!(
        res.hits.iter().any(|h| h.file == "src/config.rs"),
        "hit must point at the non-excluded file: {:?}",
        res.hits
    );
}

#[test]
fn empty_scan_exclude_scans_everything() {
    // Empty list = today's behavior: the defining file is flagged.
    let (_dir, repo) = repo_with(&[("nxm_git/src/scan.rs", "token \"gho_x\"\n")]);
    let res = scan_forbidden(&repo, None, &[]).unwrap();
    assert!(!res.clean, "empty exclude must scan everything");
}

#[test]
fn test_fixtures_are_excluded_by_glob() {
    // `**/tests/**` must cover both crates' test trees.
    let (_dir, repo) = repo_with(&[
        (
            "nxm_git/tests/pr.rs",
            "url = \"https://x/draco690456/y\";\n",
        ),
        ("nxm_git_mcp/tests/http_tool.rs", "path /Users/secret\n"),
    ]);
    let excludes = vec!["**/tests/**".to_string()];
    let res = scan_forbidden(&repo, None, &excludes).unwrap();
    assert!(res.clean, "test fixtures must be excluded: {:?}", res.hits);
}

#[test]
fn scan_rs_self_is_excluded_by_default() {
    // The spec's named end-to-end case: the DEFAULT policy excludes the file
    // that defines the patterns, so scanning it produces no hits even though
    // it necessarily contains every forbidden token.
    let (_dir, repo) = repo_with(&[(
        "nxm_git/src/scan.rs",
        "const FORBIDDEN = [\"draco690456\", \"/Users/\", \"gho_\"];\n",
    )]);
    let res = scan_forbidden(&repo, None, &Policy::default().scan_exclude).unwrap();
    assert!(
        res.clean,
        "the pattern-defining file must be excluded by default: {:?}",
        res.hits
    );
}

// --- Pure matcher unit tests (moved out of scan.rs: RULES forbids inline
// `#[cfg(test)]` modules in production code). They exercise the public
// `path_matches_exclude`; `is_excluded` is covered end-to-end by the
// `scan_forbidden` tests above.

#[test]
fn exact_file_prefix_matches_only_that_file() {
    assert!(path_matches_exclude(
        "nxm_git/src/scan.rs",
        "nxm_git/src/scan.rs"
    ));
    assert!(!path_matches_exclude(
        "nxm_git/src/scan_other.rs",
        "nxm_git/src/scan.rs"
    ));
}

#[test]
fn directory_prefix_matches_nested_only_on_boundary() {
    assert!(path_matches_exclude("nxm_git/tests/pr.rs", "nxm_git/tests"));
    assert!(path_matches_exclude("nxm_git/tests", "nxm_git/tests"));
    // Must not match a sibling that merely shares the prefix string.
    assert!(!path_matches_exclude(
        "nxm_git/tests_helpers.rs",
        "nxm_git/tests"
    ));
}

#[test]
fn glob_star_crosses_separators() {
    assert!(path_matches_exclude("nxm_git/tests/pr.rs", "**/tests/**"));
    assert!(path_matches_exclude(
        "nxm_git_mcp/tests/http_tool.rs",
        "**/tests/**"
    ));
    assert!(!path_matches_exclude("nxm_git/src/scan.rs", "**/tests/**"));
}

#[test]
fn glob_suffix_matches_extension() {
    assert!(path_matches_exclude("Cargo.lock", "*.lock"));
    assert!(!path_matches_exclude("Cargo.toml", "*.lock"));
}

#[test]
fn empty_pattern_never_matches() {
    assert!(!path_matches_exclude("anything", ""));
}
