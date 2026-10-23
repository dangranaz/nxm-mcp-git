# Delegation — 05b Follow-up: worktree-safe `merge_source_branch` (`git rev-parse --git-path`)

> **Status:** READY · **Created:** 2026-10-18
> Companion: `05b-merge-source-worktree-safe_ITA.md` (keep in sync).
> Origin: micro-task 05 review (NEEDS_CHANGES, 2026-10-18).
> **Depends on:** the task 05 branch (`work/05-workstream-wire-write`, NOT yet merged). This
> follow-up applies to THAT branch, it does not start from scratch.
> Pipeline: `nxm-git/WORKFLOW.md`. Write: `nxm_git/src/write.rs`.

## Objective (one micro-task, then STOP)
Fix the single blocking defect found in the task 05 review: `merge_source_branch` in
`nxm_git/src/write.rs` builds a hard-coded path `repo.path.join(".git/MERGE_HEAD")` that
**does not work in secondary git worktrees** — exactly the scenario this workstream must
support. Make it worktree-safe and add the missing test. Nothing else.

## Skill
**`tdd`** — write the failing test FIRST (a merge prepared in a **secondary worktree**,
source not resolved by the current code), then fix until it passes.

## Context (already decided — do not reopen)
- **The bug (empirically verified in review):** in a secondary git worktree `.git` is a
  *file* (`gitdir: ...`), and `MERGE_HEAD` lives in `.git/worktrees/<name>/MERGE_HEAD`, NOT
  in `<worktree>/.git/MERGE_HEAD`. So `repo.path.join(".git/MERGE_HEAD").is_file()` is always
  `false` in worktrees → `merge_source_branch` returns `None` → a legitimate
  `integration/* -> develop` merge performed FROM a worktree is wrongly blocked by
  `develop_direct_commit`.
- `merge_source_branch` is the **only** hard-coded `.git` in the source: everything else uses
  `git2::Repository` (`ResolvedRepo::inner`), which resolves the worktree gitdir correctly.
  This fix realigns with the repo convention.
- **Decided fix:** replace the hard-coded path with the path resolved by git itself:
  `git rev-parse --git-path MERGE_HEAD` (via the existing `run_git` argv-array pattern)
  returns the correct path BOTH in the main repo (`.git/MERGE_HEAD`) AND in a worktree
  (`.git/worktrees/<name>/MERGE_HEAD`). Then check that path's existence.
  - Equivalent alternative: git2's `repo.inner.path()` gives the real gitdir; that is also
    fine, as long as the result is worktree-safe. Pick ONE way (preferred:
    `rev-parse --git-path`, consistent with the existing `name-rev` usage) and be consistent.
- Downstream semantics stay identical: merge in progress → resolve source via
  `git name-rev --name-only MERGE_HEAD`; no merge or unresolved name → `None` (fail-safe).

## Intervention points (ONLY `write.rs` + `tests/workstream.rs`)
- `merge_source_branch`: replace the path construction. Keep: `run_git` argv-array, no shell
  string, no `unwrap`/`expect` in production, fail-safe on `None`.
- Do NOT change `commit`/`create_branch`/`gate`/the task-04 pure logic.

## Rules to respect
- `write.rs` style (`run_git`, `tracing` logging). Ecosystem `nxm-docs/RULES.md`: no
  `unwrap`/`expect` in production, structured errors, English, ~300 lines/file.
- `CHANGELOG.md`: if the task 05 entry already exists under `[Unreleased]`, do **not**
  duplicate it; at most update the line to reflect the worktree-safe fix (one line). Do not
  invent a new release.

## Requirements (each with its test — invariant rule 7)
- R1 Worktree-safe (the missing test): + **test** `merge_from_integration_in_worktree_ok` —
  create a temp repo, then a **secondary worktree** (`git worktree add`), prepare there an
  in-progress merge of an `integration/*` into `develop` (`git merge --no-ff --no-commit`),
  open the `ResolvedRepo` on the WORKTREE PATH, and verify that `commit(...)` resolves the
  source to `integration/*` and **passes** G1 without confirm. This test MUST fail with the
  current code (hard-coded path) and pass after the fix.
- R2 Main-repo non-regression: + the 7 existing `tests/workstream.rs` tests stay green (in
  particular `merge_from_integration_into_develop_ok` on the main repo).
- R3 No merge in progress → `None`: behavior unchanged (covered by existing tests on develop
  without a merge; if there is no direct test for `merge_source_branch=None`, add a light one).

## Work branch
- Continue on the task 05 branch: `work/05-workstream-wire-write` (the fix is part of the
  same micro-task, failed in review). Do NOT open a new parallel work branch.
- Do NOT commit on `develop`/`main`. The `work/05 -> feature/workstream-enforcement` merge is
  done by the reviewer/supervisor AFTER this follow-up is approved.

## Available tools (use if needed)
- `nxm-mcp-exec` — `run_gate {repo_path}`; "done" only on FULL `verdict:"GO"` (fmt included).
- `nxm-memory` — context on the bug (episodic task 05 review, 2026-10-18).
- `nxm-decision` — if a fork appears (rev-parse vs git2 path), categorize it.

## Prohibitions
- Do NOT reopen the task-04 pure logic or change `commit`/`create_branch`/`gate` beyond the
  path resolution.
- Do NOT use a hard-coded `.git` path: use a git-resolved (worktree-safe) path.
- Do NOT run git via shell string: only `run_git` argv-array.
- Do NOT add dependencies. Do NOT self-approve: external review is required.

## STOP and hand off to review
- Complete ONLY this fix, then STOP.
- Show `git diff nxm_git/src/write.rs nxm_git/tests/workstream.rs` and the REAL output of
  `run_gate` (full GO). Explicitly show that the new test R1 FAILS on the pre-fix code and
  PASSES after (describe it or show both runs).
- **Hand off to an external reviewer** — reference skill `code-review`. Merge only after review.

## Expected verification
```bash
# run_gate { repo_path: "~/Projects/private/nxm-git" } -> FULL GO verdict
cargo test --workspace            # 7 existing + 1 new (worktree) green
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt -- --check              # exit 0
```
Expected empirical proof of the fix: `git rev-parse --git-path MERGE_HEAD` in a worktree
returns `.git/worktrees/<name>/MERGE_HEAD`, not `.git/MERGE_HEAD`.
