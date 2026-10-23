# Delegation — 03 fix M1: implement git_log and git_blame (stubs) + exact diff counts

> **Status:** READY · **Created:** 2026-10-03 · **Priority:** BEFORE merging T8/0.1.0.
> Companion: `03-fix-log-blame_ITA.md` (keep in sync).
> Origin: found during T8 review — two M1 read-tools are stubs returning empty.

## Objective (one micro-task, then STOP)
Actually implement `git_log` and `git_blame` in the core (they currently return
empty lists), with tests on real data. Also make `git_diff` +/- counts exact. Then STOP.

## Skill
**`tdd`** — there is an observable, wrong behaviour (empty output) to drive to green
with tests on a real repo. Write the failing test first (red) showing the current
emptiness, then implement (green).

## Context (already decided — do not reopen)
- Repo: `~/Projects/private/nxm-git`. Read backend via **git2** (decided in M1).
- `nxm_git/src/log.rs`: `pub fn log(...)` has `// Dummy implementation for now` and
  returns `LogResult { commits: Vec::new() }`. The struct `CommitEntry { sha, author,
  author_email, date, subject }` is already defined and wired in the server — do NOT
  change it.
- `nxm_git/src/blame.rs`: `pub fn blame(...)` opens the blame but has
  `// TODO: Implement proper iteration` and returns `BlameResult { lines: Vec::new() }`.
  The struct `BlameLine { line_no, commit, author, content }` is already defined and wired.
- `nxm_git/src/diff.rs`: counts use `hunk.new_lines()/old_lines()` (--stat-like
  approximation). The struct `FileDiff { path, additions, deletions }` has NO hunks
  field — do not add one. Make counts exact using the per-line callback
  (`line.origin()` '+'/'-'), without changing the result shape.
- `status.rs` and `scan.rs` already exist as the git2 style + TempDir test model.

## Rules to follow
- Ecosystem: `nxm-docs/RULES.md` — NO `unwrap()`/`expect()` in production (tests only);
  `anyhow::Result` + `.context(...)`; `tracing` logging with per-feature targets;
  `///` docs with an example on public APIs; **max 300 lines per file**; Conventional
  Commits (`fix:`/`feat:`); tests mandatory before merge.

## Requirements
- `log(repo, range, limit)`: returns real commits (sha, author, author_email, date,
  subject). Honour `range` (e.g. "A..B") and `limit`. Order: newest-first.
- `blame(repo, path)`: returns one `BlameLine` per line with real commit/author/content.
- `diff`: exact per-file and total `additions`/`deletions` (per-line counting), same
  result shape.
- **Tests on real data** for each (TempDir with known commits): assert concrete values,
  not just "non-empty". E.g. `commits.len() == N`, blame attributing the line to the
  right commit, diff with verified +X/-Y.

## Working branch
- Create and work on a dedicated branch: `work/03-fix-log-blame`.
- Do NOT commit to main. Validation **before** merge.

## Available tools (use if needed)
- `nxm-memory` — find the git2 pattern already used in status.rs/scan.rs.
- `nxm-decision` — categorise any forks (e.g. date format, range handling).
- `nxm-session` — read/update session state if relevant.

## Prohibitions
- Do NOT change the shape of public structs (CommitEntry, BlameLine, FileDiff,
  LogResult, BlameResult, DiffResult) or tool signatures: only fill in the implementations.
- Do NOT touch policy.rs/write.rs/pr.rs/the already-working tools.
- Do NOT self-approve: external review is required.

## STOP and hand off to review
- Complete ONLY log + blame + diff counts, then STOP.
- Show the diff and verification output. **Hand off to an external reviewer** (skill
  `code-review`). The branch is validated/merged **only after** approved review.
- If anything is ambiguous, STOP and ask.

## Expected verification
```bash
cargo clippy --workspace --all-targets   # 0 warnings
cargo test --workspace                   # green, with NEW real-data tests for log/blame/diff
# a git_log call on a repo with N commits returns N commits (not [])
# a git_blame call on a file returns one line per row with the correct commit
```
