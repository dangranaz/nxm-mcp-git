# Delegation — 05 Workstream enforcement: wire guards into `write.rs` (resolve base branch via git)

> **Status:** READY · **Created:** 2026-10-18 · **Updated:** 2026-10-18 (post-fmt)
> Companion: `05-workstream-wire-write_ITA.md` (keep in sync).
> Design origin: `sessions/ses_20261018_nxmgit_workstream_worktree_enforcement.md`.
> **Depends on:** micro-task 04, **already merged** into `feature/workstream-enforcement`
> (policy fields + `PolicyFlags.source_branch` + pure guards). The branch is also **already
> aligned with the formatted `develop`** (rustfmt chore 07): the repo format standard is now
> `rustfmt 1.9.0` and `cargo fmt -- --check` is a REAL gate (green on the branch today).
> Start from `feature/workstream-enforcement`.
> Pipeline: `nxm-git/WORKFLOW.md`. Write: `nxm_git/src/write.rs`, resolve: `nxm_git/src/resolve.rs`.

## Objective (one micro-task, then STOP)
Connect the pure guards introduced in task 04 to the real writes in `nxm_git/src/write.rs`:
**resolve via git** the `source_branch` (base/merge-source) and pass it into `PolicyFlags`,
so `check_policy` can apply G1 (direct commit on develop) and G2 (base of `work/*`). A
single outcome: the two affected writes (`commit`, `create_branch`) compute and pass
`source_branch`; nothing else changes.

## Skill
**`tdd`** — write integration tests first on a temp repo (existing fixtures in
`nxm_git/tests/`): direct commit on `develop` blocked/allowed per policy; `work/*` created
off the wrong base blocked. Then implement the git resolution until green.

## Context (already decided — do not reopen)
- From task 04 these exist: `Policy.integration_branches`, `Policy.develop_direct_commit`
  (`Deny|Confirm`), `Policy.develop_branch`, `Policy.work_branch_prefix`, and
  `PolicyFlags.source_branch: Option<String>`. `check_policy` is already pure and complete:
  here it is ONLY about **populating** `source_branch` correctly on the caller side.
- In `write.rs` today: `gate(repo, op, target_branch, confirm, allow_force)` builds the
  `PolicyFlags` and calls `check_policy`. `current_branch(repo)` reads HEAD via symref.
  `run_git(repo, &[..])` runs git as an argv-array (NEVER a shell string) — **reuse this
  pattern** for every git query, no new dependencies.
- **`source_branch` semantics per operation (contract with task 04):**
  - `commit`: `source_branch` = the branch the content about to land on `target_branch`
    comes from. For a "normal" commit on the current branch this is NOT a merge:
    `source_branch = None` (so on `develop` the `develop_direct_commit` rule fires). If the
    commit in progress is a **merge commit** (HEAD has, or is about to have, >1 parent —
    e.g. `MERGE_HEAD` present), resolve the merge source branch and pass it as
    `source_branch` (so a merge `integration/* -> develop` is allowed). Detect the merge
    state robustly (presence of `MERGE_HEAD`), without assuming.
  - `create_branch`: `source_branch` = the branch HEAD is on **at creation time** (the base
    the new branch will spring from), i.e. `current_branch(repo)`.
- **Backward-compat gate** is already in the pure logic (task 04): with empty
  `integration_branches`, no new denial. The wiring must NOT alter this: if the policy does
  not activate the guards, `source_branch` may still be computed but does not change the outcome.

## Intervention points (ONLY `write.rs`, + fixtures in `nxm_git/tests/`)
- Extend `gate` (or add a parameter/variant) so it accepts and forwards a
  `source_branch: Option<String>` into `PolicyFlags`. Keep the API unchanged for writes that
  do not need it (`switch`, `push`, `force_push`): pass `None`.
- In `commit`: compute `source_branch` per the semantics above (None if non-merge; source
  branch if a merge is in progress) and pass it to the gate.
- In `create_branch`: compute `source_branch = current_branch(repo)` and pass it to the gate.
- Do NOT change `push`/`force_push`/`switch` except to forward `None` to the extended gate.

## Rules to respect
- Current repo: no local `RULES.md` → follow `write.rs` style (`run_git` argv-array,
  logging `tracing::info!(target: "nxm::git::write", ...)`).
- Ecosystem: `nxm-docs/RULES.md` — no `unwrap()`/`expect()` in production (ok in tests);
  structured errors (`WriteError`), never free text; English for code/docs; max ~300
  lines/file (if `write.rs` overflows, it is **authorized** to extract a helper — e.g. the
  merge-source resolution in a dedicated module; if you split, flag it).
- `CHANGELOG.md`: add the entry for the active write-side enforcement (hard rule: authorized).

## Requirements (each with its temp-repo test — invariant rule 7)
- R1 Direct commit on develop blocked: + **test** `commit_direct_on_develop_denied` — temp
  repo with policy `integration_branches=["integration/*"]`, `develop_direct_commit="confirm"`;
  on `develop`, a normal commit without confirm → `WriteError::Policy` with reason
  `DevelopDirectCommit`; git commit did NOT happen (verify HEAD did not advance).
- R2 Commit on develop with confirm: + **test** `commit_direct_on_develop_with_confirm_ok` —
  same policy, `confirm=true` → the commit proceeds.
- R3 Merge from integration allowed: + **test** `merge_from_integration_into_develop_ok` —
  set up a `MERGE_HEAD` (or perform a real no-ff merge of an `integration/x` into `develop`)
  and verify the write is not blocked (source resolved to `integration/x`).
- R4 Wrong work base blocked: + **test** `create_work_branch_from_main_denied` — HEAD on
  `main`, `create_branch("work/09-x")` → `WriteError::Policy` (dedicated base reason); the
  branch was NOT created.
- R5 Correct work base: + **test** `create_work_branch_from_integration_ok` — HEAD on
  `integration/foo`, `create_branch("work/09-x")` → ok, branch created.
- R6 Backward compatibility: + **test** `no_integration_branches_commit_on_develop_ok` —
  policy **without** `integration_branches` → commit on `develop` without confirm proceeds
  (as today).
- R7 Unaffected writes unchanged: + **test** `switch_and_push_unaffected` — `switch` and the
  `push` gate behave as before (source `None`, no new denial).

## Work branch
- Start from `feature/workstream-enforcement` (already contains 04 + the fmt reformat,
  aligned with `develop`). Create `work/05-workstream-wire-write`.
- Do NOT commit on `main` or `develop`. Work on `work/*`, no-ff merge `work→feature`. The
  `feature→develop` merge is done by the reviewer/supervisor after approval.
- The repo is now fmt-compliant (rustfmt 1.9.0): format your new code with `cargo fmt`
  before handing off, so `cargo fmt -- --check` stays green (do not reformat out-of-scope
  files).

## Available tools (use if needed)
- `nxm-mcp-exec` — `run_gate {repo_path}`; "done" only on `verdict:"GO"`. Fall back to `cargo`.
- `nxm-memory` — search the existing temp-repo test patterns (how they set up remotes/commits).
- `nxm-decision` — categorize forks (how to detect an in-progress merge; where to extract the helper).
- `nxm-session` — read session state if relevant.

## Prohibitions
- Do NOT reopen the pure logic from task 04 (`check_policy`, the fields): here you only
  **populate** `source_branch`. If you find a hole in the pure logic, STOP and flag it
  (follow-up on 04), do not work around it in `write.rs`.
- Do NOT run git via shell string: only `run_git` argv-array.
- Do NOT touch the MCP tools, `pr.rs`, `prepush.rs`, `config.rs`.
- Do NOT add dependencies. BUT rules win (file split / CHANGELOG): authorized.
- Do NOT implement the worktree→workstream map (it is an optional out-of-scope follow-up).
- Do NOT self-approve: external review is required.

## STOP and hand off to review
- Complete ONLY this micro-task, then STOP.
- Show `git diff` (write.rs + any helper + tests + CHANGELOG) and the REAL output of
  `run_gate` (or `cargo test`/`clippy`/`fmt`).
- **Hand off to an external reviewer** — reference skill `code-review`. Merge only after review.
- If anything is ambiguous (in-progress merge detection, how tests set up temp repos), STOP and ask.

## Expected verification
```bash
# run_gate { repo_path: "~/Projects/private/nxm-git" } -> FULL GO verdict (all 4 steps)
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt -- --check    # now a REAL gate: must exit 0
```
Pre-existing tests green; the 7 new integration tests green; `run_gate` must give a full GO
(fmt included, unlike the tasks before chore 07); paste the real output.
