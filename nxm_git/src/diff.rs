use anyhow::Result;
use git2::Tree;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single file diff entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    /// Path of the file (relative to repo root).
    pub path: String,
    /// Number of lines added.
    pub additions: u32,
    /// Number of lines deleted.
    pub deletions: u32,
}

/// The result of `git diff`, summarizing changes by file.
///
/// Equivalent to `git diff --stat` but returns structured data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffResult {
    /// List of files with their addition/deletion counts.
    pub files: Vec<FileDiff>,
    /// Total additions across all files.
    pub total_additions: u32,
    /// Total deletions across all files.
    pub total_deletions: u32,
}

/// Peel an object to a tree recursively.
fn peel_to_tree<'a>(mut obj: git2::Object<'a>) -> Result<Tree<'a>> {
    loop {
        match obj.kind() {
            Some(git2::ObjectType::Tree) => return Ok(obj.peel_to_tree()?),
            Some(git2::ObjectType::Tag) => {
                obj = obj.peel(git2::ObjectType::Any)?;
            }
            Some(git2::ObjectType::Commit) => {
                let commit = obj.peel_to_commit()?;
                return Ok(commit.tree()?);
            }
            Some(git2::ObjectType::Blob) => {
                return Err(anyhow::anyhow!("blob object cannot be peeled to a tree"));
            }
            Some(git2::ObjectType::Any) => {
                return Err(anyhow::anyhow!("unsupported object type for tree peel"));
            }
            None => {
                return Err(anyhow::anyhow!("unable to peel object to tree"));
            }
        }
    }
}

/// Compute the diff of the repository.
///
/// By default, shows unstaged changes (worktree vs index).
/// If `from` and/or `to` are provided, shows diff between those refs.
pub fn diff(
    repo: &crate::resolve::ResolvedRepo,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<DiffResult> {
    let mut opts = git2::DiffOptions::new();
    opts.include_untracked(true);
    opts.include_ignored(false);

    let mut files = Vec::new();
    let mut total_additions = 0;
    let mut total_deletions = 0;

    match (from, to) {
        (None, None) => {
            let mut diff = repo.inner.diff_index_to_workdir(None, Some(&mut opts))?;
            process_diff(
                &mut diff,
                &mut files,
                &mut total_additions,
                &mut total_deletions,
            )?;
        }
        (Some(from_ref), None) => {
            let obj = repo.inner.revparse_single(from_ref)?;
            let tree = peel_to_tree(obj)?;
            let mut diff = repo
                .inner
                .diff_tree_to_workdir(Some(&tree), Some(&mut opts))?;
            process_diff(
                &mut diff,
                &mut files,
                &mut total_additions,
                &mut total_deletions,
            )?;
        }
        (None, Some(to_ref)) => {
            let obj = repo.inner.revparse_single(to_ref)?;
            let tree = peel_to_tree(obj)?;
            let mut diff = repo
                .inner
                .diff_tree_to_workdir(Some(&tree), Some(&mut opts))?;
            process_diff(
                &mut diff,
                &mut files,
                &mut total_additions,
                &mut total_deletions,
            )?;
        }
        (Some(from_ref), Some(to_ref)) => {
            let from_obj = repo.inner.revparse_single(from_ref)?;
            let from_tree = peel_to_tree(from_obj)?;
            let to_obj = repo.inner.revparse_single(to_ref)?;
            let to_tree = peel_to_tree(to_obj)?;
            let mut diff =
                repo.inner
                    .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut opts))?;
            process_diff(
                &mut diff,
                &mut files,
                &mut total_additions,
                &mut total_deletions,
            )?;
        }
    }

    Ok(DiffResult {
        files,
        total_additions,
        total_deletions,
    })
}

/// Collect the path a delta refers to, preferring the post-image path and
/// falling back to the pre-image one (deleted files only have the latter).
fn delta_path(delta: &git2::DiffDelta) -> String {
    delta
        .new_file()
        .path()
        .or_else(|| delta.old_file().path())
        .and_then(|p| p.to_str())
        .unwrap_or("<unknown>")
        .to_string()
}

/// Process a diff to collect file-level statistics with real addition/deletion counts.
///
/// Counts are taken **per line** from the line callback, not from hunk headers:
/// `DiffHunk::new_lines()`/`old_lines()` report the hunk's full span including
/// context, which over-counts a small edit inside a large hunk. Only lines whose
/// origin is `Addition`/`Deletion` are counted, so context lines contribute zero.
fn process_diff(
    diff: &mut git2::Diff,
    files: &mut Vec<FileDiff>,
    total_additions: &mut u32,
    total_deletions: &mut u32,
) -> Result<()> {
    let mut file_counts: HashMap<String, (u32, u32)> = HashMap::new();

    diff.foreach(
        &mut |delta: git2::DiffDelta, _progress: f32| {
            // Seed the entry so a delta whose lines never reach the line
            // callback (binary, mode-only, submodule) still appears in the
            // output with zero counts.
            file_counts.entry(delta_path(&delta)).or_insert((0, 0));
            true
        },
        None,
        None,
        None,
    )?;

    diff.foreach(
        &mut |_delta: git2::DiffDelta, _progress: f32| true,
        None,
        None,
        Some(
            &mut |delta: git2::DiffDelta, _hunk: Option<git2::DiffHunk>, line: git2::DiffLine| {
                // Context lines carry origin ' ' and contribute nothing; only the
                // two change origins are counted, which is what makes these numbers
                // exact rather than hunk-shaped.
                let counts = file_counts.entry(delta_path(&delta)).or_insert((0, 0));
                match line.origin() {
                    '+' => counts.0 += 1,
                    '-' => counts.1 += 1,
                    _ => {}
                }
                true
            },
        ),
    )?;

    let mut total_additions_acc = 0;
    let mut total_deletions_acc = 0;
    for (path, (additions, deletions)) in file_counts {
        files.push(FileDiff {
            path,
            additions,
            deletions,
        });
        total_additions_acc += additions;
        total_deletions_acc += deletions;
    }

    // Stable order: git prints paths sorted; a HashMap would make output
    // non-deterministic between runs, which an agent-facing tool must not be.
    files.sort_by(|a, b| a.path.cmp(&b.path));

    *total_additions = total_additions_acc;
    *total_deletions = total_deletions_acc;

    Ok(())
}
