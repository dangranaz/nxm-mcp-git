# Delegation — 04 Workstream enforcement: policy fields + guard logic (pure core in `policy.rs`)

> **Status:** READY · **Created:** 2026-10-18
> Companion: `04-workstream-policy-core_ITA.md` (keep in sync).
> Design origin: `sessions/ses_20261018_nxmgit_workstream_worktree_enforcement.md`.
> Pipeline: `nxm-git/WORKFLOW.md`. Policy core: `nxm_git/src/policy.rs`.

## Objective (one micro-task, then STOP)
Extend **only** `nxm_git/src/policy.rs` with the new policy fields and the **pure guard
logic** (no git, no side effects) that enforces the parallel-workstream convention. Do
NOT touch `write.rs` (that is micro-task 05). A single outcome: a richer `Policy` +
`check_policy` applying the new rules, all covered by unit tests.

## Skill
**`tdd`** — write the tests first (direct commit on `develop` without confirm → denied;
merge from `integration/*` → allowed; `work/*` created off `main` → denied), then
implement fields + logic until green. `check_policy` is pure: testable without git.

## Context (already decided — do not reopen)
- Crate: `~/Projects/private/nxm-git` (Cargo workspace). Core lib: `nxm_git`.
- `Policy` today (`#[serde(deny_unknown_fields, default)]`) has: `protected_branches`,
  `allow_force`, `destructive_confirm`, `forbidden_patterns`, `scan_exclude`,
  `require_pre_push`. `check_policy(op, &Policy, &PolicyFlags) -> Result<(), PolicyDenied>`
  is **pure**: it never touches git. Today `CreateBranch`/`Switch` are always allowed.
- `PolicyFlags` has: `confirm`, `allow_force`, `target_branch: Option<String>`.
- **Design decision (contract with task 05, do NOT reopen):** to know whether a
  commit/merge onto `develop` "comes from an `integration/*`", or which base a `work/*`
  springs from, `check_policy` does NOT query git. Add a `source_branch: Option<String>`
  field to `PolicyFlags` that the **caller** (write.rs, task 05) will resolve via git and
  pass in. `check_policy` stays pure.
- Backward-compatible defaults: with the new fields **absent** from the toml, behavior
  must be **identical to today** (no new denials).

## New `Policy` fields (additive, with defaults)
1. `integration_branches: Vec<String>` — globs of workstream landing-zone branches
   (e.g. `["integration/*"]`). **Default: empty** (`Vec::new()`), so a repo without this
   field activates no new guard → backward compatible.
2. `develop_direct_commit: DevelopDirectCommit` — enum `Deny | Confirm`, **default
   `Confirm`**. Serde: `#[serde(rename_all = "lowercase")]` → accepts `"deny"`/`"confirm"`.
   Applied ONLY when the target branch is `develop` (see guard below).
3. `develop_branch: String` — the confluence branch name, **default `"develop"`** (do not
   hard-code the string in the logic; read it from the field so it is configurable).
4. `work_branch_prefix: String` — the workstream branch prefix, **default `"work/"`**.

> The glob matching of `integration_branches` must be consistent with the one already used
> for `scan_exclude` (see `crate::scan::path_matches_exclude` / the existing glob helper).
> If a reusable glob helper exists, **reuse it**; do not introduce a new dependency.

## New guards in `check_policy` (pure)
- **G1 — direct commit/merge onto develop.** If `op == Commit` and `target_branch ==
  policy.develop_branch`: if `source_branch` is `Some(b)` and `b` **matches** an
  `integration_branches` glob → **allowed** (a legitimate milestone sync); otherwise apply
  `develop_direct_commit`: `Confirm` → denied unless `flags.confirm`; `Deny` → always
  denied (even with confirm). New dedicated `DenialReason` (e.g. `DevelopDirectCommit`).
- **G2 — work-branch base.** If `op == CreateBranch` and the new name starts with
  `work_branch_prefix`: it must spring from an `integration/*` (or from `develop` **with**
  `confirm`), not from `main`. The "base" is `source_branch` (passed by the caller). If
  `source_branch` is `None` → fail-safe denial (new `DenialReason`, e.g. `UnknownBase`),
  consistent with the existing `UnknownBranch` philosophy. If it is an `integration/*` →
  allowed; if `develop` → allowed only with `confirm`; otherwise (e.g. `main`) → denied.
- **Backward-compat gate (critical):** G1/G2 activate ONLY when `integration_branches` is
  **non-empty** (the repo has declared it uses the convention). Empty list → behavior
  identical to today. Encode this explicit gate and **test it**.

## Rules to respect
- Current repo: no local `RULES.md` → follow the style of existing files (`policy.rs`).
- Ecosystem: `nxm-docs/RULES.md` — no `unwrap()`/`expect()` in production (ok in tests);
  structured errors via `thiserror`/enum (style of `DenialReason`/`PolicyDenied`), never
  free text; English for code/docs; max ~300 lines per file (if `policy.rs` overflows, it
  is **authorized** to extract a module — e.g. `policy/workstream.rs` — keeping the public
  API; if you must split, stop first and flag it in the handoff).
- `CHANGELOG.md`: add an entry for the new policy fields (hard rule: authorized).

## Requirements (each with its test — invariant rule 7)
- R1 Additive parsing + defaults: + **test** `policy_defaults_are_backcompat` — a `Policy`
  from toml **without** the new fields deserializes and behaves as today; `Policy::default()`
  has empty `integration_branches`, `develop_direct_commit == Confirm`, `develop_branch ==
  "develop"`, `work_branch_prefix == "work/"`.
- R2 `deny_unknown_fields` preserved: + **test** `unknown_field_still_errors` — a toml with
  an unknown key still fails to parse.
- R3 Enum parsing: + **test** `develop_direct_commit_parses_deny_and_confirm` — `"deny"` and
  `"confirm"` deserialize to their variants; an unknown value fails.
- R4 G1 confirm: + **test** `direct_commit_on_develop_without_confirm_denied` —
  `integration_branches=["integration/*"]`, `develop_direct_commit=Confirm`, `op=Commit`,
  `target_branch="develop"`, `source_branch=None/non-integration`, `confirm=false` →
  `Err(DevelopDirectCommit)`; with `confirm=true` → `Ok`.
- R5 G1 absolute deny: + **test** `direct_commit_on_develop_deny_mode_blocks_even_with_confirm`
  — `develop_direct_commit=Deny`, `confirm=true`, non-integration source → `Err`.
- R6 G1 merge from integration: + **test** `merge_from_integration_into_develop_allowed` —
  `source_branch="integration/foo"`, `target_branch="develop"` → `Ok` even without confirm.
- R7 G2 base main: + **test** `work_branch_from_main_denied` — `op=CreateBranch`,
  name `work/07-x`, `source_branch="main"` → `Err` (dedicated reason).
- R8 G2 base integration/develop: + **test** `work_branch_from_integration_allowed` (ok) and
  `work_branch_from_develop_needs_confirm` (develop without confirm → Err, with confirm → Ok).
- R9 G2 unknown base fail-safe: + **test** `work_branch_without_source_denied` —
  `source_branch=None` → `Err` (fail-safe).
- R10 Global backward-compat: + **test** `empty_integration_branches_disables_new_guards`
  — with empty `integration_branches`, a commit on `develop` without confirm → `Ok` (as
  today), and a `work/*` off `main` → `Ok` (no new guard active).

## Work branch
- Start from `develop` (up to date). Create `feature/workstream-enforcement`, then
  `work/04-workstream-policy-core`.
- Do NOT commit on `main` or `develop` directly. Work on `work/*`, no-ff merge
  `work→feature`. The `feature→develop` merge is done by the reviewer/supervisor after approval.

## Available tools (use if needed)
- `nxm-mcp-exec` — validate with `run_gate {repo_path}` (check→clippy→fmt_check→test);
  declare "done" only on `verdict:"GO"`. Fall back to `cargo` if the server is unavailable.
- `nxm-memory` — search for similar patterns (glob helper, `DenialReason` style) in the repo.
- `nxm-decision` — categorize forks (reuse glob helper vs new; reason names).
- `nxm-session` — read session state if relevant.

## Prohibitions
- Do NOT touch `write.rs`, `pr.rs`, `prepush.rs` or the MCP tools: the logic is pure in
  `policy.rs`. You ADD the `source_branch` field to `PolicyFlags` here, but the git
  **resolution** belongs to task 05 — here it stays an `Option<String>` passed by tests.
- Do NOT change the semantics of existing guards (`ProtectedBranch`, force-push): the new
  reasons are additive.
- Do NOT hard-code `"develop"`/`"work/"` in the logic: read them from policy fields.
- Do NOT add external dependencies.
- BUT rules win: if the file exceeds 300 lines or the CHANGELOG must be updated, do it
  (authorized). On a scope↔rule conflict, STOP and ask.
- Do NOT self-approve: external review is required.

## STOP and hand off to review
- Complete ONLY this micro-task, then STOP.
- Show `git diff` (policy.rs + any extracted module + CHANGELOG) and the REAL output of
  `run_gate` (or `cargo test`/`clippy`).
- **Hand off to an external reviewer** — reference skill `code-review`. The branch is
  validated/merged **only after** approved review.
- If anything is ambiguous (reason names, where to split the module), STOP and ask.

## Expected verification
```bash
# preferred: typed tools
# run_gate { repo_path: "~/Projects/private/nxm-git" } -> verdict GO
# shell fallback:
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt -- --check
```
Pre-existing tests stay green; the 10+ new `policy.rs` tests pass; paste the real output.
