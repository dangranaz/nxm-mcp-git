# Delegation — 06 Workstream enforcement: documentation (`.example` + `WORKFLOW.md`)

> **Status:** READY · **Created:** 2026-10-18 · **Updated:** 2026-10-18 (post-05)
> Companion: `06-workstream-docs_ITA.md` (keep in sync).
> Design origin: `sessions/ses_20261018_nxmgit_workstream_worktree_enforcement.md`.
> **Depends on:** micro-tasks 04 and 05, **both already merged** into
> `feature/workstream-enforcement` (policy fields + guards + worktree-safe write.rs wiring).
> The branch is also aligned with the formatted `develop` (chore 07): the format standard is
> `rustfmt 1.9.0` and `cargo fmt -- --check` is green on the branch. Start from
> `feature/workstream-enforcement`: the field names/semantics are now FINAL in the code.
> Files: `.nxm-git-policy.toml.example`, `WORKFLOW.md`.

## Objective (one micro-task, then STOP)
Align the documentation with the now-active enforcement: document the new policy fields in
`.nxm-git-policy.toml.example` and **remove the "soft convention / not yet enforced"
marker** from the worktree section of `WORKFLOW.md`, linking the policy fields. Docs only:
no code, no behavior tests.

## Skill
**`implement`** — targeted, consistent edits to the two documentation files, verifying that
the names/defaults cited match EXACTLY the code merged from 04/05.

## Context (already decided — do not reopen)
- After 04/05 the policy fields exist: `integration_branches` (default empty),
  `develop_direct_commit` (`"deny"|"confirm"`, default `"confirm"`), `develop_branch`
  (default `"develop"`), `work_branch_prefix` (default `"work/"`). **Read the real values
  from the merged code** (`nxm_git/src/policy.rs`) before writing: the docs must reflect the
  code, not this brief if the code has diverged.
- `WORKFLOW.md` today contains the section *"Parallel workstreams in isolated worktrees"*
  (line ~85) with a marker (lines ~87–89) declaring it a **soft / documentary / not yet
  enforced** convention:
  `> **Status: soft convention (documentary).** This section is process, not yet enforced
  by nxm-git...`. After 04/05 it is enforced → this marker block must be removed and
  replaced with a pointer to the policy fields that enforce it (e.g. "enforced by `nxm-git`
  via the `integration_branches` / `develop_direct_commit` policy fields").
- `.nxm-git-policy.toml.example` is the commented template: the new fields are added
  **commented out** (like the other optional ones), with a brief explanation and the default
  made explicit.

## Requirements (verifiable — invariant rule 7 adapted for docs)
- R1 `.example` documents the 4 new fields, commented, with correct defaults and one
  explanation line each. **Verify:** field names and cited defaults match 1:1 with
  `Policy::default()` in `policy.rs` (direct comparison; cite the source diff).
- R2 Enum consistency: `develop_direct_commit` documents the values `"deny"` and `"confirm"`
  and which is the default. **Verify:** values and default match the `DevelopDirectCommit`
  enum in the code.
- R3 `WORKFLOW.md`: the worktree section no longer contains the "soft / not yet enforced"
  marker. **Verify:** `grep -ni "not yet enforced\\|soft convention" WORKFLOW.md` no longer
  returns the worktree-section line (or it is rewritten as "enforced").
- R4 `WORKFLOW.md` links the policy fields that enforce the convention (field names + pointer
  to `.nxm-git-policy.toml`). **Verify:** the field names cited in WORKFLOW.md exist in
  `policy.rs` (no invented names).
- R5 No factual regression: the docs do not promise unimplemented behavior (e.g. do NOT
  document the worktree→workstream map, which is an unimplemented follow-up).

## Work branch
- Start from `feature/workstream-enforcement` (after 04 and 05). Create `work/06-workstream-docs`.
- Do NOT commit on `main` or `develop`. No-ff merge `work→feature`. The `feature→develop`
  merge is done by the reviewer/supervisor after approval.

## Available tools (use if needed)
- `nxm-memory` — re-read `policy.rs` and `WORKFLOW.md` to align the names.
- `nxm-decision` — categorize forks (where to place the pointer; wording).
- `nxm-session` — read session state if relevant.

## Prohibitions
- Do NOT touch code (`*.rs`), MCP tools, tests. Only `.nxm-git-policy.toml.example` and
  `WORKFLOW.md`.
- Do NOT document unimplemented features (worktree→workstream map).
- Do NOT invent names/defaults: they must match the merged code; if they diverge, STOP and
  flag it (the docs must not "fix" the code).
- Do NOT self-approve: external review is required.

## STOP and hand off to review
- Complete ONLY this micro-task, then STOP.
- Show `git diff` of the two files and the output of the verification `grep`s (R3/R4).
- **Hand off to an external reviewer** — reference skill `code-review`. Merge only after review.
- If the names/defaults in the code diverge from the brief, STOP and ask.

## Expected verification
```bash
grep -ni "not yet enforced\|soft convention" WORKFLOW.md
grep -nE "integration_branches|develop_direct_commit|develop_branch|work_branch_prefix" \
  .nxm-git-policy.toml.example WORKFLOW.md nxm_git/src/policy.rs
# names/defaults in .example and WORKFLOW.md match policy.rs
```
No residual "soft" marker in the worktree section; the 4 fields documented, consistent with the code.
