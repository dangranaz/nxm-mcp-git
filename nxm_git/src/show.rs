use anyhow::Result;
use git2::ObjectType;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The result of `git show <ref>:<path>` - the contents of a file at a given ref.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileShowResult {
    /// Path of the file (relative to repo root).
    pub path: String,
    /// Contents of the file.
    pub content: String,
}

/// The result of `git show <ref>` - commit information and diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitShowResult {
    /// Commit information.
    pub commit: super::log::CommitEntry,
    /// TODO: Add diff information
    pub diff: String,
}

/// The result of `git show`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ShowResult {
    /// Showing a file at a specific path.
    File(FileShowResult),
    /// Showing a commit object.
    Commit(CommitShowResult),
}

/// Show the contents of a file at a given ref, or show a commit object.
///
/// `ref_name` is required (e.g., "HEAD", "v1.0", "feature/branch").
/// `path` is optional; if provided, shows the file at that path in the given ref.
/// If `path` is not provided, shows the commit object (like `git show <ref>`).
pub fn show(
    repo: &crate::resolve::ResolvedRepo,
    ref_name: &str,
    path: Option<&str>,
) -> Result<ShowResult> {
    if let Some(path) = path {
        let obj = repo.inner.revparse_single(ref_name)?;
        let mut obj = obj;
        loop {
            match obj.kind() {
                Some(ObjectType::Blob) => break,
                Some(ObjectType::Tag) => {
                    obj = obj.peel(ObjectType::Any)?;
                }
                Some(ObjectType::Commit) => {
                    let commit = obj.peel_to_commit()?;
                    let tree = commit.tree()?;
                    let path_ref = Path::new(path);
                    if let Ok(entry) = tree.get_path(path_ref) {
                        let blob = entry.to_object(&repo.inner)?;
                        obj = blob;
                        break;
                    } else {
                        return Err(anyhow::anyhow!("path not found in commit"));
                    }
                }
                Some(ObjectType::Tree) => {
                    return Err(anyhow::anyhow!("object is a tree, not a blob"));
                }
                Some(ObjectType::Any) => {
                    return Err(anyhow::anyhow!("unsupported object type"));
                }
                None => {
                    return Err(anyhow::anyhow!("unable to peel object"));
                }
            }
        }
        let blob = obj.as_blob().ok_or_else(|| anyhow::anyhow!("not a blob"))?;
        let content = std::str::from_utf8(blob.content())
            .unwrap_or("<invalid utf-8>")
            .to_string();
        Ok(ShowResult::File(FileShowResult {
            path: path.to_string(),
            content,
        }))
    } else {
        let obj = repo.inner.revparse_single(ref_name)?;
        let commit = obj.peel_to_commit()?;
        let author = commit.author();
        let super::log::CommitEntry {
            sha,
            author: _author,
            author_email,
            date,
            subject,
        } = super::log::CommitEntry {
            sha: commit.id().to_string(),
            author: author.name().unwrap_or("<unknown>").to_string(),
            author_email: author.email().unwrap_or("<unknown>").to_string(),
            date: author.when().seconds(),
            subject: commit.summary().unwrap_or("<no subject>").to_string(),
        };
        let diff = String::new();
        Ok(ShowResult::Commit(CommitShowResult {
            commit: super::log::CommitEntry {
                sha,
                author: _author,
                author_email,
                date,
                subject,
            },
            diff,
        }))
    }
}
