# Delegation — 01 Add `scan_exclude` to the forbidden scan (nxm-git)

> **Status:** READY · **Created:** 2026-10-05
> Companion: `01-scan-exclude_ITA.md` (keep in sync).
> Files: `nxm_git/src/scan.rs`, `nxm_git/src/policy.rs`. Pipeline: `nxm-git/WORKFLOW.md`.

## Why
`scan_forbidden` applies `FORBIDDEN_PATTERNS` to the WHOLE tree of any repo, so a private repo trips
on legitimate content: `scan.rs` (which DEFINES the patterns), project metadata (`Cargo.toml`,
`README.md`, `CONTEXT.md` carrying the owner `draco690456` and `/Users/` example paths) and test
fixtures (`pr.rs`, `http_tool.rs`). 26 false-positive hits block `git_push`, none a real secret.
The forbidden guard is meant for the PUBLIC boundary; it must not block on a repo's own legitimate
content. Fix: a policy-driven exclusion list, WITHOUT weakening detection of real secrets.

## Goal (one micro-task, then STOP)
Add `scan_exclude` to `Policy` and honour it in `scan_forbidden`: files whose path matches an
exclusion are skipped. Keep real-secret detection intact.

## Skill
**`tdd`** — write tests first (an excluded file yields no hit; a NON-excluded file containing a real
token like `gho_...` is STILL a hit), then implement.

## Context (already decided — do not reopen)
- Crate `~/Projects/private/nxm-git`, workspace Cargo (`cargo ... --workspace`).
- `Policy` has `#[serde(deny_unknown_fields, default)]` → the new field MUST be `#[serde(default)]`
  so existing policy files keep parsing.
- Exclusions are **path-based** (relative repo paths): support simple prefix match and/or a `*` glob
  on the path. Keep it dependency-free (no new crate): a small manual matcher is fine.
- Default `scan_exclude`: cover the known structural false positives generically but NARROWLY — the
  file that defines the patterns (`nxm_git/src/scan.rs`) and test fixtures (`**/tests/**`). Do NOT
  blanket-exclude all `*.md` or `Cargo.toml` (that could hide a real leak in metadata); if a metadata
  false positive remains, prefer leaving it visible over a broad exclusion. Document each default
  exclusion with a comment.
- `scan_exclude` empty = scan everything (today's behaviour).

## Requirements
1. `Policy.scan_exclude: Vec<String>` with `#[serde(default)]` + a sensible `Default` (the narrow
   list above). Document the field.
2. In `scan_forbidden`, skip a blob whose relative path matches any `scan_exclude` entry (prefix or
   `*` glob). The matching helper is pure and unit-tested.
3. A file NOT excluded that contains a forbidden pattern (e.g. `gho_<token>`) still produces a hit.
4. Update `.nxm-git-policy.toml` and `.nxm-git-policy.toml.example` in this repo with a commented
   `scan_exclude`. Update `CHANGELOG.md` ([Unreleased]).

## Tests (inline / tests dir per existing style)
- `scan_skips_excluded_paths`: a file at an excluded path with a pattern → no hit.
- `scan_still_flags_real_secret_in_non_excluded_file`: `gho_...` in a normal source file → hit.
- `empty_scan_exclude_scans_everything`: unchanged behaviour.
- `scan_rs_self_is_excluded_by_default`: the default excludes the pattern-defining file.

## Rules to respect (they PREVAIL over scope)
- `nxm-docs/RULES.md`: max 300 lines/file → **if scan.rs/policy.rs exceed 300 after the change,
  SPLIT by functionality** (this is authorised, not scope creep). Tests in per-module files.
  Update CHANGELOG. No `unwrap`/`expect` in prod. `tracing` logging. English in code.
- If a rule conflicts with scope, the rule wins — proceed with the rule-mandated action.

## Work branch
- Branch from `develop`. Create `feature/scan-exclude`, then `work/01-scan-exclude`.
- Do NOT commit on main/develop. work→feature / feature→develop merges are the reviewer's.

## Prohibitions
- Do NOT remove any existing `FORBIDDEN_PATTERNS` entry (esp. `gho_`): detection stays.
- Do NOT make the scan non-blocking on push. Do NOT add dependencies. No shell.
- Do NOT self-approve: external review required.

## STOP and hand off to review
- Complete only this, then STOP. Show `git diff develop..HEAD` + REAL verification output.
- Hand off to an external reviewer (`code-review`). Merge only after approval.
- If anything is ambiguous (e.g. glob semantics), STOP and ask.

## Expected verification
```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
wc -l nxm_git/src/scan.rs nxm_git/src/policy.rs   # each < 300
```
Then (manual sanity, reviewer): git_scan_forbidden on this repo returns CLEAN or only real hits.
