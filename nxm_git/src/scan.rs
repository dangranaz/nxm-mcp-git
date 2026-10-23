use anyhow::Result;
use git2::{ObjectType, Tree};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// A single hit from the forbidden patterns scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanHit {
    /// Path of the file (relative to repo root).
    pub file: String,
    /// Line number (1-indexed).
    pub line: u32,
    /// The line content that matched.
    pub content: String,
}

/// The result of `git_scan_forbidden`.
///
/// Indicates whether the scan is clean, and if not, lists the hits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanResult {
    /// True if no forbidden patterns were found.
    pub clean: bool,
    /// List of hits (empty if clean is true).
    pub hits: Vec<ScanHit>,
}

/// Forbidden content patterns.
/// Single source of truth: used by [`scan_forbidden`] and as the default
/// `forbidden_patterns` of the write policy (`.nxm-git-policy.toml`).
pub const FORBIDDEN_PATTERNS: &[&str] = &[
    "draco690456",
    "nxm-private",
    "gitlab",
    "/Users/",
    "source of truth",
    "gho_",
];

/// Whether a repo-relative `path` is excluded by a single `pattern`.
///
/// Patterns are matched against the forward-slash repo-relative path. Two
/// shapes are supported, chosen to cover the structural false positives of a
/// repo scanning itself without hiding a real leak:
///
/// - A pattern containing `*` is a glob where each `*` matches any run of
///   characters, path separators included (so `**/tests/**` and `*/tests/*`
///   both match `nxm_git/tests/pr.rs`). Non-`*` characters match literally.
/// - A pattern with no `*` matches a path that equals it or is nested under it
///   (prefix on a path boundary): `nxm_git/src/scan.rs` matches that exact
///   file, and `nxm_git/tests` matches everything under that directory.
///
/// Pure and side-effect-free: it only inspects the two strings.
///
/// # Examples
///
/// ```
/// use nxm_git::scan::path_matches_exclude;
///
/// assert!(path_matches_exclude("nxm_git/tests/pr.rs", "**/tests/**"));
/// assert!(path_matches_exclude("nxm_git/src/scan.rs", "nxm_git/src/scan.rs"));
/// assert!(!path_matches_exclude("nxm_git/src/write.rs", "**/tests/**"));
/// ```
pub fn path_matches_exclude(path: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    if pattern.contains('*') {
        return glob_matches(path, pattern);
    }
    path == pattern || path.starts_with(&format!("{pattern}/"))
}

/// Whether `path` is excluded by ANY entry of `excludes`.
pub(crate) fn is_excluded(path: &str, excludes: &[String]) -> bool {
    excludes.iter().any(|p| path_matches_exclude(path, p))
}

/// Minimal glob: every `*` in `pattern` matches any (possibly empty) run of
/// characters, path separators included; all other characters match
/// literally. Backtracking over the segments between stars.
pub(crate) fn glob_matches(path: &str, pattern: &str) -> bool {
    let segments: Vec<&str> = pattern.split('*').collect();
    // No leading `*`: the path must start with the first literal segment.
    let mut pos = 0usize;
    let last = segments.len() - 1;
    for (i, seg) in segments.iter().enumerate() {
        if seg.is_empty() {
            continue;
        }
        if i == 0 {
            // Anchored at the start.
            if !path[pos..].starts_with(seg) {
                return false;
            }
            pos += seg.len();
        } else if i == last {
            // Anchored at the end.
            return path[pos..].ends_with(seg) && path.len() - pos >= seg.len();
        } else {
            // Floating: find the next occurrence at or after `pos`.
            match path[pos..].find(seg) {
                Some(off) => pos += off + seg.len(),
                None => return false,
            }
        }
    }
    // Pattern ended with `*` (last segment empty): the rest is absorbed.
    true
}

/// Compute the forbidden patterns scan for a repository at a given ref.
///
/// If `ref_name` is None, defaults to HEAD. Files whose repo-relative path
/// matches an entry of `scan_exclude` are skipped (see
/// [`path_matches_exclude`]); an empty list scans the whole tree. Real-secret
/// detection is unchanged for every file that is NOT excluded.
pub fn scan_forbidden(
    repo: &crate::resolve::ResolvedRepo,
    ref_name: Option<&str>,
    scan_exclude: &[String],
) -> Result<ScanResult> {
    info!(
        target: "nxm::scan::forbidden",
        ref_name = ?ref_name,
        excludes = scan_exclude.len(),
        "ENTER scan_forbidden"
    );
    let commit = if let Some(ref_name) = ref_name {
        let obj = repo.inner.revparse_single(ref_name)?;
        obj.peel_to_commit()?
    } else {
        repo.inner.head()?.peel_to_commit()?
    };
    let tree = commit.tree()?;

    let mut hits = Vec::new();
    let mut stack: Vec<(Tree, String)> = Vec::new();
    stack.push((tree, String::new()));

    while let Some((tree, prefix)) = stack.pop() {
        for entry in tree.iter() {
            let file_name = entry.name().unwrap_or("<invalid utf8>");
            let full_path = if prefix.is_empty() {
                file_name.to_string()
            } else {
                format!("{}/{}", prefix, file_name)
            };

            match entry.kind() {
                Some(ObjectType::Blob) => {
                    if is_excluded(&full_path, scan_exclude) {
                        debug!(target: "nxm::scan::forbidden", file = %full_path, "skip excluded");
                        continue;
                    }
                    let blob_obj = entry.to_object(&repo.inner)?;
                    if let Some(blob) = blob_obj.as_blob() {
                        let content =
                            std::str::from_utf8(blob.content()).unwrap_or("<invalid utf-8>");
                        for (line_no, line) in content.lines().enumerate() {
                            let line_no = line_no as u32 + 1;
                            for pattern in FORBIDDEN_PATTERNS {
                                if line.contains(pattern) {
                                    hits.push(ScanHit {
                                        file: full_path.clone(),
                                        line: line_no,
                                        content: line.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
                Some(ObjectType::Tree) => {
                    let tree_obj = entry.to_object(&repo.inner)?;
                    if let Some(subtree) = tree_obj.as_tree() {
                        stack.push((subtree.clone(), full_path));
                    }
                }
                _ => {}
            }
        }
    }

    let clean = hits.is_empty();
    info!(
        target: "nxm::scan::forbidden",
        clean,
        hits = hits.len(),
        "EXIT scan_forbidden"
    );

    Ok(ScanResult { clean, hits })
}
