# Delegation — 01 Enforce `require_pre_push` (hard gate) in nxm-git

> **Status:** READY · **Created:** 2026-10-04
> Companion: `01-enforce-require-pre-push_ITA.md` (keep in sync).
> Pipeline: `nxm-git/WORKFLOW.md`. Policy: `nxm_git/src/policy.rs`, write: `nxm_git/src/write.rs`.

## Goal (one micro-task, then STOP)
Make `Policy.require_pre_push` **effective**: today it is declared but never run.
In `nxm_git/src/write.rs`, inside `push`, run the `require_pre_push` commands as a **hard gate**
BEFORE the push: if a command fails, the push does NOT run and a structured error is returned.
Nothing else.

## Skill
**`tdd`** — write the tests first (pre-push passes → push proceeds; pre-push fails → push blocked,
git never runs), then implement until they pass.

## Context (already decided — do not reopen)
- Crate: `~/Projects/private/nxm-git` (Cargo workspace). Core lib: `nxm_git`.
- Today `write::push` does, in order: `gate(policy)` → `scan_forbidden` → `run_git push`.
  The pre-push commands must run **after scan_forbidden and before `run_git push`**.
- `Policy::load(&repo.path)` is already called inside `gate`; here you need the policy in `push`
  to read `require_pre_push` (re-load it, or extend the logic). Keep `gate` as is (shared by
  commit/branch/switch) and add the pre-push step ONLY in `push`.
- `require_pre_push: Vec<String>` are commands like `"cargo test --workspace"`: split on
  whitespace into argv and run with `std::process::Command` in `repo.path` (same argv-array style
  as `run_git`, NEVER an interpolated shell string). Empty = no command (no-op).
- Hard gate: a command with non-zero exit status blocks the push.

## Rules to respect
- Current repo: no local `RULES.md` → existing files' style (`write.rs`).
- Ecosystem: `nxm-docs/RULES.md` — no `unwrap()`/`expect()` in production (ok in tests);
  `tracing::info!(target: "nxm::git::write", ...)` logging like the existing one; English for code/docs.
- Structured errors via `thiserror` (the `WriteError` style): no loose free text.

## Requirements
1. New `WriteError` variant, e.g. `PrePushFailed { command: String, stderr: String }` (git was not
   run). Add its case to `WriteError::to_value()` with `"kind": "pre_push_failed"`, `command`,
   `stderr` (consistent with the other payloads).
2. In `push`, after `scan_forbidden` and before `run_git push`: for each command in
   `policy.require_pre_push`, run it in `repo.path`; on the first failure return
   `WriteError::PrePushFailed` (git push does NOT run). Log entry/exit of the pre-push step.
3. An empty command or empty list = no gate (unchanged behaviour for repos without pre-push).
4. Argv-array execution (whitespace split), never a shell string. Command-not-found / spawn error
   → map to a clear error (reuse `PrePushFailed` or `Other`, with a reason).

## Tests (inline, style of write.rs tests / tests/write.rs)
- `push_runs_pre_push_and_proceeds_when_ok`: policy with `require_pre_push=["true"]` (a passing
  command) → the push proceeds (or, if a real push is awkward in tests, assert the pre-push gate
  does not block; follow the existing test pattern on a temp repo with a dummy remote).
- `push_blocked_when_pre_push_fails`: policy with `require_pre_push=["false"]` → `WriteError::PrePushFailed`,
  and assert the push did NOT happen (nothing pushed).
- `empty_pre_push_is_noop`: no command → behaviour as today.

## Work branch
- Branch from `develop` (up to date). Create `feature/enforce-pre-push`, then `work/01-enforce-pre-push`.
- Do NOT commit on `main` or `develop` directly. Work on `work/*`, merge no-ff work→feature.
  The feature→develop merge is done by the reviewer/supervisor after approval.

## Available tools (use if needed)
- `nxm-memory` — find similar patterns (run_git, gate) in the repo.
- `nxm-decision` — categorize forks (e.g. reuse Other vs new variant).
- `nxm-session` — read the relevant session state if needed.

## Prohibitions
- Do NOT change the semantics of `check_policy`, `gate`, or the other writes (commit/branch/switch/forcepush).
- Do NOT run commands via a shell (`sh -c`): argv-array only.
- Do NOT add dependencies. Do NOT leave the scope (only write.rs + its tests).
- Do NOT self-approve: external review is required.

## STOP and hand off to review
- Complete ONLY this micro-task, then STOP.
- Show `git diff nxm_git/src/write.rs` (+ any tests) and the REAL verification output.
- **Hand off to an external reviewer** (another agent/person) — reference skill `code-review`.
  The branch is validated/merged **only after** approved review.
- If anything is ambiguous (e.g. how existing tests simulate the push), STOP and ask.

## Expected verification
```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
Pre-existing tests stay green; the 3 new tests green; paste the real output.
