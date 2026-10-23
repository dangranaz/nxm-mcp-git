# Changelog

All notable changes to nxm-git. Format: Conventional Commits summary per release.

## [Unreleased]

## [0.2.0] — 2026-10-21

Two creation tools for the parallel-workstream convention — the integration
branch and its isolated worktree — plus the fix for the `create_branch`
wrong-base bug, shipped alongside the workstream enforcement guards. 14 typed
MCP tools.

### chore

- **nxm-git now applies the workstream convention to itself (dogfooding).** The
  repo's own `.nxm-git-policy.toml` activates the G1/G2 guards:
  `integration_branches = ["integration/*"]`, `develop_direct_commit = "confirm"`,
  `develop_branch = "develop"`, `work_branch_prefix = "work/"`. From now on a
  direct commit on `develop` needs `confirm` (or a merge from `integration/*`),
  and `work/*` branches must be based on an integration branch. Also refreshed
  the stale `require_pre_push` comment (the gate is already wired).

### feat

- **Two creation tools for the parallel-workstream convention
  (`git_create_integration_branch` + `git_worktree_add`).** They make the
  `WORKFLOW.md` hierarchy mechanical instead of hand-driven shell:
  `git_create_integration_branch { workstream, base?, confirm? }` builds
  `integration/<workstream>` rooted at an explicit base (default: the policy's
  `develop_branch`, never the current HEAD) and validates the name against the
  configured `integration_branches`; `git_worktree_add { workstream, branch?,
  path? }` then adds the workstream's isolated worktree (default
  `.worktrees/<workstream>` on `integration/<workstream>`). The order is forced
  and typed: `worktree_add` refuses if the branch does not exist, if the path
  already exists, or if the path is outside `.worktrees/`. New `worktree` core
  module (`worktree_add`, `WorktreeResult`); git is always an argv array. The
  MCP surface grows from 12 to 14 tools.

- **Fix: `git_create_branch` no longer branches from the current HEAD.** The
  tool gained an explicit `base` parameter; when omitted it now roots the new
  branch at the policy's `develop_branch` instead of whatever the shared
  worktree happened to be checked out on. This closes the concrete bug where a
  branch created while HEAD sat on an unrelated `work/*` was silently rooted at
  the wrong commit. The resolved base is both the git start-point
  (`git branch <name> <base>`) and the `source_branch` the G2 guard judges.
  **Behaviour change (intentional):** the old "branch from HEAD" default is gone
  — to reproduce it, pass `base` explicitly.

- **Workstream enforcement wired into the write tools (task 05 + fix 05b).** `commit` and
  `create_branch` now resolve the git base branch (`source_branch`) and pass it
  to the pure `check_policy` guard from task 04: a plain commit on `develop` is
  governed by `develop_direct_commit`, while an in-progress merge resolves its
  source branch via `git rev-parse --git-path MERGE_HEAD` (worktree-safe) +
  `git name-rev` so a legitimate `integration/* -> develop` sync passes also
  from a secondary worktree; a `work/*` branch is created from the current HEAD
  (fail-safe on `main`, allowed from `integration/*`, `develop` needs
  `confirm`). No new dependencies, no shell-string git, no logic change in
  `check_policy`.

- **Workstream enforcement in the policy core (G1/G2 guards).** `Policy` gains
  four additive, retro-compatible fields — `integration_branches`
  (default empty), `develop_direct_commit` (`Confirm`/`Deny`, default
  `Confirm`), `develop_branch` (default `develop`) and `work_branch_prefix`
  (default `work/`) — plus `PolicyFlags::source_branch`, resolved by the
  caller (task 05) and kept pure here. When `integration_branches` is
  non-empty: a direct commit on `develop_branch` is allowed only from an
  integration branch or per `develop_direct_commit`, and a `work/*` branch
  must be based on an integration branch or on `develop` with `confirm`
  (fail-safe on unknown base). With an empty list, behavior is identical to
  before. New structured `DenialReason` variants
  (`DevelopDirectCommit`, `UnknownBase`, `WorkBranchBaseNotAllowed`). No git
  access, no side effects, no new dependencies.

- **`scan_exclude` lets the forbidden-content scan skip legitimate paths.** New
  `Policy.scan_exclude` (`#[serde(default)]`): repo-relative path prefixes or
  `*`-globs (e.g. `nxm_git/src/scan.rs`, `**/tests/**`) whose files are skipped
  by `scan_forbidden`, so a repo no longer blocks its own push on the file that
  *defines* the patterns or on test fixtures that use example tokens. The match
  helper (`scan::path_matches_exclude`) is pure and unit-tested; an empty list
  scans everything, and detection of a real secret in any non-excluded file is
  unchanged. The default is deliberately narrow — it never excludes broad
  metadata globs (`*.md`, `Cargo.toml`) that could hide a real leak. Honored
  both by `git_push`'s pre-push gate and the `git_scan_forbidden` tool.

- **`require_pre_push` is now enforced as a hard gate in `git_push`.** The
  commands from `.nxm-git-policy.toml` run after the forbidden-content scan and
  before the push, each as a strict argv array (never a shell string). A failing
  command blocks the push and returns a structured `pre_push_failed` error
  (command + stderr); an empty list or empty command stays a no-op.

## [0.1.0] — 2026-10-03

First release. Read-side, write-side with policy enforcement, and the
`git_pr_create` tool, exposed as 12 typed MCP tools over Streamable HTTP.

### feat

- **M1 — read-side (T0–T4).** Crate `nxm-git`: typed, structured Git via
  `git2`, no output parsing. Modules `status`, `diff`, `log`, `branches`,
  `show`, `blame`, plus `resolve` (path canonicalization + `allowed_roots`
  enforcement) and `config` (`~/.nxm/git-mcp.toml`). Seven read tools:
  `git_status`, `git_diff`, `git_log`, `git_branches`, `git_show`, `git_blame`,
  `git_scan_forbidden` — the last reusing the nxm-sync FORBIDDEN patterns as
  the flagship public-safety query.
- **M2 — policy core and gated write tools (T5–T6).** `.nxm-git-policy.toml`
  parsed by the core lib with `deny_unknown_fields` (a typo is a load error, so
  a branch is never silently left unprotected) and sensible defaults
  (`protected_branches = ["main"]`, no force, `destructive_confirm = true`).
  Pure `check_policy(op, repo_path, flags)` — never touches Git itself. Five
  write tools shell out with strict argument arrays (never string
  interpolation): `git_commit`, `git_create_branch`, `git_switch`, `git_push`
  (refuses protected targets, unflagged force, and forbidden content), and the
  policy-gated branch/PR wrappers. Risky ops take explicit `confirm` /
  `allow_force` flags — friction, not a cage.
- **T7 — `git_pr_create`.** Wraps `gh pr create --title --body --base` and
  returns the PR URL, extracted robustly from `gh` stdout rather than by
  assuming a stream shape. Policy-gated like the other write tools: a
  protected base branch requires `confirm=true`. Opens the PR only — never
  passes `--merge` or any auto-merge flag.
- **MCP server (`nxm-git-mcp`).** rmcp 0.14 with `#[tool]` / `#[tool_router]` /
  `#[tool_handler]`, schemas via schemars. MCP Streamable HTTP **stateless**
  (`stateful_mode=false`) on `127.0.0.1:7281`, route `/mcp`, shared by all
  clients as one process. Also serves stdio when no address is given;
  `--http <ADDR>` wins over `NXM_GIT_MCP_HTTP_ADDR`. Graceful shutdown on
  Ctrl-C / SIGTERM in HTTP mode.

### test

- `nxm_git/tests/pr.rs` — PR URL extraction across `gh` stdout shapes.
- `nxm_git_mcp/tests/http_tool.rs` — real server on an ephemeral port, full
  MCP handshake over HTTP, then `tools/call`.

### docs

- **`README.md`** — what nxm-git is, the dual model (`nxm-git` core lib +
  `nxm-git-mcp` thin server, two consumers: agent over MCP, TUI in-process),
  the 12 tools, `~/.nxm/git-mcp.toml` and per-repo `.nxm-git-policy.toml`,
  `git-up.sh` / `git-down.sh`, port :7281, client registration, and how to
  verify the install.
- **`scripts/git-up.sh` / `git-down.sh`** documented and verified working.
- **`README.md`** registration snippets for OpenCode, Codex, Kiro CLI and Pi,
  plus the `nxm-mcp` dashboard entry.

### chore

- `install.sh` — build-from-source installer: builds the release binary,
  installs it to `~/.local/bin`, seeds `~/.nxm/git-mcp.toml` if missing, and
  registers nxm-git as an MCP server on Kiro CLI, Pi and OpenCode (freebuff does
  not consume MCP servers, so it is skipped). Idempotent; backs up client
  configs before editing.
- Version workspace `0.1.0`.

### fixed

- **`git_log`, `git_blame` and exact `git_diff` counts implemented.** The two
  read tools that shipped as stubs now return real data: `git_log` walks history
  (range + limit, newest-first), `git_blame` attributes every worktree line
  (including uncommitted ones) via `blame_buffer`, and `git_diff` counts
  additions/deletions per line, cross-checked against `git diff --numstat`.
- **`scripts/git-up.sh` had a syntax error and could not run.** A stray extra
  `}` on the binary-existence check (`exit 1; }}`) made the script fail to
  parse; `bash -n` reported "unexpected end of file". Fixed. The script was
  never exercised before now — it is verified working as part of this release.