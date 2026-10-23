use anyhow::{Context, Result};
use git2::Sort;
use serde::{Deserialize, Serialize};

/// A single commit entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitEntry {
    /// Commit SHA-1 hash.
    pub sha: String,
    /// Author name.
    pub author: String,
    /// Author email.
    pub author_email: String,
    /// Commit timestamp (seconds since epoch).
    pub date: i64,
    /// Commit message subject (first line).
    pub subject: String,
}

/// The result of `git log`, returning a list of commits.
///
/// Equivalent to `git log --pretty=format:%H,%an,%ae,%at,%s` but returns structured data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogResult {
    /// List of commits, newest first.
    pub commits: Vec<CommitEntry>,
}

/// Compute the log of the repository.
///
/// By default, shows the history of the current branch (HEAD).
/// If `range` is provided, shows commits in that range (e.g., "v1.0..v2.0").
/// If `limit` is provided, limits the number of commits returned.
///
/// Ordering is newest-first by commit timestamp, matching `git log`.
///
/// # Example
/// ```no_run
/// # use nxm_git::{ResolvedRepo, log};
/// # let repo = ResolvedRepo::open("/path/to/repo", &[])?;
/// // The five most recent commits, newest first.
/// let recent = log(&repo, None, Some(5))?;
/// for commit in &recent.commits {
///     println!("{} {}", &commit.sha[..7], commit.subject);
/// }
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn log(
    repo: &crate::resolve::ResolvedRepo,
    range: Option<&str>,
    limit: Option<usize>,
) -> Result<LogResult> {
    let mut walk = repo.inner.revwalk().context("starting revwalk")?;

    // Newest first: commit time descending, as `git log` does by default.
    walk.set_sorting(Sort::TIME)
        .context("sorting revwalk by time")?;

    match range {
        None => {
            // An unborn HEAD (freshly `git init`, nothing committed yet) is a
            // normal state, not an error: there is simply no history to show.
            if repo.inner.head().is_err() {
                tracing::debug!(target: "nxm::git::log", "EXIT log: unborn HEAD, no commits");
                return Ok(LogResult {
                    commits: Vec::new(),
                });
            }
            walk.push_head().context("pushing HEAD onto revwalk")?
        }
        Some(spec) => {
            let spec = spec.trim();
            // `A..B` == reachable from B but not A; `A...B` is symmetric-difference
            // and left unsupported on purpose: an ambiguous merge-base walk is a
            // silent-wrong-answer risk for an agent-facing tool.
            let (from, to) = spec
                .split_once("..")
                .with_context(|| format!("unsupported range {spec:?}: expected 'FROM..TO'"))?;
            if spec.contains("...") {
                anyhow::bail!(
                    "unsupported range {spec:?}: symmetric 'A...B' is not supported, use 'A..TO'"
                );
            }
            let to_oid = resolve_rev(&repo.inner, to)
                .with_context(|| format!("resolving the TO side of range {spec:?}"))?;
            walk.push(to_oid)
                .with_context(|| format!("pushing {to:?} onto revwalk"))?;
            if !from.is_empty() {
                let from_oid = resolve_rev(&repo.inner, from)
                    .with_context(|| format!("resolving the FROM side of range {spec:?}"))?;
                walk.hide(from_oid)
                    .with_context(|| format!("hiding {from:?} from revwalk"))?;
            }
        }
    }

    let mut commits = Vec::new();
    for oid in walk {
        // Applied after sorting/hiding, so the N newest of the *range* survive.
        if limit.is_some_and(|n| commits.len() >= n) {
            break;
        }
        let oid = oid.context("walking a commit id")?;
        let commit = repo
            .inner
            .find_commit(oid)
            .with_context(|| format!("loading commit {oid}"))?;
        let author = commit.author();
        commits.push(CommitEntry {
            sha: commit.id().to_string(),
            author: author.name().unwrap_or_default().to_string(),
            author_email: author.email().unwrap_or_default().to_string(),
            date: author.when().seconds(),
            subject: commit.summary().unwrap_or_default().to_string(),
        });
    }

    tracing::debug!(
        target: "nxm::git::log",
        commits = commits.len(),
        range = ?range,
        limit = ?limit,
        "EXIT log"
    );

    Ok(LogResult { commits })
}

/// Resolve a revision string to an object id, with the commit peeled out.
fn resolve_rev(repo: &git2::Repository, rev: &str) -> Result<git2::Oid> {
    let rev = if rev.is_empty() { "HEAD" } else { rev };
    let obj = repo
        .revparse_single(rev)
        .with_context(|| format!("revparse_single({rev:?})"))?;
    let commit = obj
        .peel_to_commit()
        .with_context(|| format!("peeling {rev:?} to a commit"))?;
    Ok(commit.id())
}
