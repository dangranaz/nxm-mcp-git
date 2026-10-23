# Delegation — 01b Follow-up: rules compliance for enforce require_pre_push

> **Status:** READY · **Created:** 2026-10-04
> Companion: `01b-enforce-pre-push-followup_ITA.md` (keep in sync).
> Follows: `01-enforce-require-pre-push_ENG.md`. Existing branch: `work/01-enforce-pre-push` @ `cf846ca`.

## Why this follow-up
The review APPROVED the spec but found 2 HARD violations of `nxm-docs/RULES.md`, in tension with the
brief's "do not leave the scope" rule. **The scope was too narrow: the rules win.** This brief
EXTENDS the scope to make it compliant. Continue on the same branch `work/01-enforce-pre-push`.

## Skill
**`tdd`** — keep existing tests green while splitting; add the missing test (empty-string).

## Required fixes (all on the same branch work/01-enforce-pre-push)
1. **300-line limit** (`nxm-docs/RULES.md`): today `write.rs`=311, `tests/write.rs`=327. Split by functionality:
   - Extract the pre-push logic (`PrePushFailed` variant, the `require_pre_push` command runner, its
     `to_value`) into a dedicated module `nxm_git/src/prepush.rs`; `write.rs::push` calls it. Declare
     `mod prepush;` next to the other modules. `write.rs` must drop below 300.
   - Move the 3 pre-push tests from `tests/write.rs` into a new `nxm_git/tests/prepush.rs`
     ("one file per module tested", RULES.md). `tests/write.rs` must drop below 300.
   - Do NOT change behaviour: only move + visibility (`pub(crate)` where needed).
2. **CHANGELOG.md** (`nxm-docs/RULES.md`): add an entry (current / Unreleased section) like:
   "require_pre_push is now enforced as a hard gate in git_push (commands run after the forbidden
   scan, before push; a failing command blocks the push with a structured pre_push_failed error)."
3. **Missing test** (Spec soft): add `empty_command_string_is_noop` (or extend `empty_pre_push_is_noop`)
   to cover an EMPTY/whitespace-only command in the list (brief 01 req 3: "empty command or empty list").
4. **EXIT log on failure** (Standards P3): on the pre-push failure branch, emit the EXIT log consistent
   with the existing `tracing` entry/exit style.
5. **Robust test** (Spec soft): if a test uses a temp path with `touch <tempdir>`, avoid breakage when
   TMPDIR contains spaces (use a space-free file name inside the temp dir, or pass the argv array).

## Out of scope (do NOT touch)
- Do NOT change the semantics of `check_policy`/`gate`/other writes.
- The double `Policy::load` and the whitespace-split `Vec<String>` are sanctioned by brief 01:
  LEAVE them (do not refactor the command abstraction in this round).
- No new dependencies. No shell (`sh -c`): argv-array only.

## Rules to respect
- `nxm-docs/RULES.md`: max 300 lines/file; tests in per-module files; CHANGELOG updated; no
  unwrap/expect in prod; `tracing` logging with `nxm::git::...` target; English in code.

## Work branch
- Continue on `work/01-enforce-pre-push` (do NOT start over). Do NOT commit on main/develop.
- After the fixes, the work→feature and feature→develop merges are done by the reviewer/supervisor after re-review.

## STOP and hand off to review
- Complete the 5 fixes, then STOP.
- Show `git diff develop..HEAD` (incl. new files: prepush.rs, tests/prepush.rs, CHANGELOG.md) and the
  REAL verification output. Confirm line count < 300 for write.rs and tests/write.rs.
- Hand off to an external reviewer (`code-review`). Merge only after approval.

## Expected verification
```bash
wc -l nxm_git/src/write.rs nxm_git/tests/write.rs   # both < 300
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
All tests green (incl. the moved pre-push tests + the new empty-string one); clippy clean; paste the real output.
