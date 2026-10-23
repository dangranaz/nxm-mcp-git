# Delegation — 07 Chore: repo-wide rustfmt normalization (`cargo fmt`)

> **Status:** READY · **Created:** 2026-10-18
> Companion: `07-rustfmt-normalize_ITA.md` (keep in sync).
> Origin: micro-task 04 review (2026-10-18) — discovered a pre-existing repo-wide rustfmt
> drift, separate from the enforcement workstream. Independent chore.
> Pipeline: `nxm-git/WORKFLOW.md`.

## Objective (one micro-task, then STOP)
Make the whole `nxm-git` repo compliant with `cargo fmt --check` with **a single**
repo-wide `cargo fmt` pass. One outcome: after the task, `cargo fmt -- --check` exits 0 (no
offending files). No logic change: formatting ONLY.

## Skill
**`implement`** — a mechanical, verifiable chore: run `cargo fmt`, verify the diff is purely
formatting, confirm the gates stay green.

## Context (already decided — do not reopen)
- The repo has a **pre-existing** rustfmt drift across ~25 files (sources + tests of both
  crates: `nxm_git/src/*`, `nxm_git/tests/*`, `nxm_git_mcp/src/*`, `nxm_git_mcp/tests/*`),
  caused by a rustfmt version mismatch (the repo was formatted with a version that ordered
  imports differently from today's `rustfmt 1.9.0-stable`: e.g. `{ScanResult,
  scan_forbidden}` vs `{scan_forbidden, ScanResult}`). It is NOT a feature regression: it is
  pre-existing debt.
- Owner's chosen path: chore **now**, dedicated branch **off `develop`**, separate from the
  enforcement workstream (`feature/workstream-enforcement`).
- The repo's `require_pre_push` is `cargo test --workspace` (green today); this chore must
  NOT change that behavior.

## EXACT scope
- Run `cargo fmt` (repo-wide, workspace) once and commit the result.
- NO logic change, no rename, no refactor: if `cargo fmt` proposes anything that is NOT pure
  formatting, STOP and ask (it should not happen).
- Do NOT touch `Cargo.toml`/`Cargo.lock`, config, CHANGELOG (it is a format chore; if a repo
  rule still mandated a CHANGELOG entry, add ONE chore line under `[Unreleased]` and flag it
  — but by default a pure format chore may omit it).

## Out of scope (separate recommendation, do NOT implement here)
- **Pinning the rustfmt/toolchain version** (`rust-toolchain.toml`): it would prevent future
  re-drift, BUT it impacts the whole toolchain (build+clippy) for everyone and is a repo
  policy decision that belongs to the owner. Do **NOT** add `rust-toolchain.toml` in this
  task. Optionally note in the handoff that it is the natural follow-up (separate ticket).

## Requirements (verifiable — invariant rule 7 adapted for a chore)
- R1 fmt compliance: **acceptance command** `cargo fmt -- --check` → **exit 0**, no files in
  output. Paste the real output.
- R2 No functional regression: `cargo test --workspace` **green** (same test count as
  before, 0 failures); `cargo clippy --workspace --all-targets -- -D warnings` clean. Paste
  the real outputs (or `run_gate` with verdict GO).
- R3 Diff is purely formatting: **verify** that the commit's `git diff --stat` shows ONLY
  `.rs` files and that no line changes semantics (spot-check: the diff must be import
  reorder / wrapping / spacing, not logic). State it in the handoff.

## Work branch
- Start from `develop` (up to date). Create `feature/rustfmt-normalize`, then
  `work/07-rustfmt-normalize`.
- Do NOT commit on `main` or `develop` directly. No-ff merge `work→feature`. The
  `feature→develop` merge is done by the reviewer/supervisor after approval.
- WARNING (known risk, not your mistake): in parallel there is
  `feature/workstream-enforcement` touching `policy.rs`/`write.rs`/tests. A repo-wide
  reformat on `develop` will create conflicts at that branch's future merge: this is
  expected and accepted by the owner (path 1). You do NOT need to coordinate with that
  branch: just format `develop`.

## Available tools (use if needed)
- `nxm-mcp-exec` — `run_gate {repo_path}` for R2 (GO on check+clippy+test); note that AFTER
  this task `cargo_fmt_check` must also return `ok:true` → `run_gate` should give a full GO.
- `nxm-memory` — context on the drift (episodic 2026-10-18, task 04 review).

## Prohibitions
- Do NOT change logic: `cargo fmt` only. If the formatter touches anything suspicious, STOP.
- Do NOT add `rust-toolchain.toml` / a pin (out of scope, owner decision).
- Do NOT hand-format selectively: use `cargo fmt` so the result is deterministic.
- Do NOT self-approve: external review is required.

## STOP and hand off to review
- Complete ONLY this chore, then STOP.
- Show the commit's `git diff --stat` and the REAL output of `cargo fmt -- --check` (exit 0),
  `cargo test --workspace`, `cargo clippy ... -D warnings` (or `run_gate` → GO).
- **Hand off to an external reviewer** — reference skill `code-review`. Merge only after review.
- Explicitly confirm the diff is purely formatting (R3).

## Expected verification
```bash
cargo fmt -- --check        # exit 0, no files
cargo test --workspace      # green, 0 failures
cargo clippy --workspace --all-targets -- -D warnings   # clean
# or: run_gate { repo_path: "~/Projects/private/nxm-git" } -> verdict GO (all 4 steps)
```
`run_gate` must now give a **full GO** (including `cargo_fmt_check: ok`), unlike before.
