use std::path::Path;

use anyhow::{Context, Result};
use git2::BlameOptions;
use serde::{Deserialize, Serialize};

/// A single line of blame information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlameLine {
    /// Line number in the file (1-indexed).
    pub line_no: u32,
    /// Commit SHA-1 that last modified this line.
    pub commit: String,
    /// Author of the commit.
    pub author: String,
    /// The line content.
    pub content: String,
}

/// The result of `git blame`, showing who last modified each line of a file.
///
/// Equivalent to `git blame --porcelain` but returns structured data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlameResult {
    /// List of blame information for each line.
    pub lines: Vec<BlameLine>,
}

/// Compute the blame for a file in the repository.
///
/// Every line of the file is attributed to the commit that last modified it,
/// numbered from 1. Lines that are identical in earlier commits keep the
/// older attribution, matching `git blame`.
///
/// # Example
/// ```no_run
/// # use nxm_git::{ResolvedRepo, blame};
/// # let repo = ResolvedRepo::open("/path/to/repo", &[])?;
/// let result = blame(&repo, "src/lib.rs")?;
/// for line in &result.lines {
///     println!("{} {} {}", line.line_no, &line.commit[..7], line.content);
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn blame(repo: &crate::resolve::ResolvedRepo, path: &str) -> Result<BlameResult> {
    let head = repo
        .inner
        .revparse_single("HEAD")
        .and_then(|obj| obj.peel_to_commit())
        .with_context(|| format!("resolving HEAD to blame {path:?}"))?
        .id();

    let mut opts = BlameOptions::new();
    opts.newest_commit(head);
    // Follow the content across renames so a moved line keeps its real history.
    opts.track_copies_any_commit_copies(true);

    let contents = std::fs::read_to_string(repo.path.join(path))
        .with_context(|| format!("reading {path:?} from the worktree"))?;
    let file_lines: Vec<&str> = contents.lines().collect();

    // An empty file has no lines to blame, and libgit2 rejects a zero-length
    // buffer, so answer directly rather than round-tripping through it.
    if contents.is_empty() {
        tracing::debug!(target: "nxm::git::blame", path, lines = 0, "EXIT blame: empty file");
        return Ok(BlameResult { lines: Vec::new() });
    }

    let committed = repo
        .inner
        .blame_file(Path::new(path), Some(&mut opts))
        .with_context(|| format!("blaming {path:?}"))?;

    // `blame_file` only walks the *committed* blob, so a worktree file would come
    // back short — uncommitted lines would silently vanish. `blame_buffer`
    // re-blames the file exactly as it stands on disk, attributing the
    // uncommitted lines to the zero OID, which is what `git blame <file>` does.
    let blame = committed
        .blame_buffer(contents.as_bytes())
        .with_context(|| format!("blaming the worktree contents of {path:?}"))?;

    // git2 0.19 reports blame per *hunk* (a run of consecutive lines sharing an
    // author), not per line, so the per-line view is rebuilt here: the content
    // comes from the file as it stands now, the attribution from the hunk.
    //
    // Author names repeat across commits; resolve each SHA once.
    let mut authors: std::collections::HashMap<git2::Oid, String> =
        std::collections::HashMap::new();
    let mut lines = Vec::new();

    for hunk in blame.iter() {
        let mut commit_id = hunk.final_commit_id();
        // Uncommitted work carries the zero OID; attribute it to the newest
        // commit so every line names a real commit, as `git blame` reports
        // "Not Committed Yet" against the commit the line followed.
        if commit_id.is_zero() {
            commit_id = head;
        }
        let author = match authors.get(&commit_id) {
            Some(name) => name.clone(),
            None => {
                let name = repo
                    .inner
                    .find_commit(commit_id)
                    .map(|c| c.author().name().unwrap_or_default().to_string())
                    .unwrap_or_default();
                authors.insert(commit_id, name.clone());
                name
            }
        };

        let start = hunk.final_start_line();
        for offset in 0..hunk.lines_in_hunk() {
            let line_no = start + offset;
            // `lines()` drops a trailing newline; a final empty line is not a line.
            let content = file_lines.get(line_no - 1).copied().unwrap_or_default();
            lines.push(BlameLine {
                line_no: line_no as u32,
                commit: commit_id.to_string(),
                author: author.clone(),
                content: content.to_string(),
            });
        }
    }

    tracing::debug!(
        target: "nxm::git::blame",
        path,
        lines = lines.len(),
        "EXIT blame"
    );

    Ok(BlameResult { lines })
}
