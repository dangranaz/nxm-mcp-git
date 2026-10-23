# Delegation — 01 git_pr_create (wrap gh pr create)

> **Status:** READY · **Created:** 2026-10-03
> Companion: `01-git-pr-create_ITA.md` (keep in sync).
> Design origin: `nxm-docs/research/2026-10-01_nxm-git-TICKETS.md` (T7) + DESIGN.

## Objective (one micro-task, then STOP)
Implement the `git_pr_create` tool in the nxm-git server: a wrapper over
`gh pr create` (title, body, base) that opens a PR and returns its URL. NEVER
auto-merge. Then STOP.

## Skill
**`tdd`** — new code with verifiable behaviour (returns a URL, never merges).
Write the test first (red), then the implementation (green), as in prior phases.

## Context (already decided — do not reopen)
- Repo: `~/Projects/private/nxm-git`. M1 (read) and M2 (policy + write) already
  approved and committed (HEAD = M2 commit `e23b181`).
- Dual model: core lib `nxm_git` + server `nxm_git_mcp`. rmcp 0.14, stateless HTTP.
- Writes via shell-out with **argument arrays** (`Command::new("gh").args([...])`),
  NEVER string interpolation. See `nxm_git/src/write.rs` (run_git) as the model.
- This tool adds to the existing 11 → `tools/list` will report 12 tools.

## Rules to follow
- Current repo: `nxm-git/RULES.md` does not exist; ecosystem rules apply.
- Ecosystem: `nxm-docs/RULES.md` — in particular: NO `unwrap()`/`expect()` in
  production (tests only); `thiserror` for library errors; `anyhow` + context at the
  application level; `tracing::info!`/`debug!` logging with per-feature targets;
  `///` docs with an example on every public API; **max 300 lines per file**;
  Conventional Commits.
- Shell-out: argument arrays only, no `format!` inside the command (like write.rs).

## Requirements
- `pr_create(repo, title, body, base) -> Result<PrResult, ...>` in the core, with
  `PrResult { url: String }`.
- Execution via `gh pr create --title <t> --body <b> --base <base>` as an arg-array.
  Dynamic values are separate argv elements (anti-injection); reject arguments that
  start with `-` where they are refs/branches (reuse the `ensure_ref_arg` pattern
  from write.rs).
- NEVER `--merge`/auto-merge: the tool only opens the PR.
- MCP tool `git_pr_create` in the server, structured error for the client (consistent
  with `WriteError::to_value`).
- `tracing` logging of the operation.

## Working branch
- Create and work on a dedicated branch: `work/01-git-pr-create`.
- Do NOT commit to main. The work must be **validated before** merge.

## Available tools (use if needed)
- `nxm-memory` — search rules/context/precedents (e.g. how nxm-sync handles `gh`
  and the draco690456/dangranaz account switch).
- `nxm-decision` — categorise any implementation forks.
- `nxm-session` — read/update session state if relevant.

## Prohibitions
- Do NOT leave scope: only `git_pr_create`. Do NOT touch policy.rs/write.rs/read-tools.
- Do NOT implement auto-merge or any operation that merges the PR.
- Do NOT run a real `gh pr create` against real repos in tests: use mocks/`#[ignore]`
  with a comment, or test only argv construction and invalid-input rejection.
- Do NOT self-approve: external review is required.

## STOP and hand off to review
- Complete ONLY `git_pr_create`, then STOP.
- Show the diff and how you verified (`cargo clippy --workspace --all-targets` 0
  warnings, `cargo test --workspace` green, `tools/list` = 12 tools).
- **Hand off to an external reviewer** (reference skill `code-review`). The branch is
  validated/merged **only after** approved review.
- If anything is ambiguous, STOP and ask.

## Expected verification
```bash
cargo clippy --workspace --all-targets   # 0 warnings
cargo test --workspace                   # all green, incl. new pr_create test
# tools/list over HTTP must list 12 tools (11 + git_pr_create)
```
