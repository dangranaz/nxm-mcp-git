# Delegation — 02 docs + versioning + MCP registration

> **Status:** READY · **Created:** 2026-10-03 · **Depends on:** 01-git-pr-create (completed and reviewed).
> Companion: `02-docs-and-register_ITA.md` (keep in sync).
> Design origin: `nxm-docs/research/2026-10-01_nxm-git-TICKETS.md` (T8) + DESIGN.

## Objective (one micro-task, then STOP)
Make nxm-git releasable and usable: README (dual model + config + 12 tools),
version 0.1.0, documented up/down scripts, and registration of the server as an MCP
on the user's clients + the nxm-mcp dashboard. Then STOP.

## Skill
**`implement`** — integration/configuration and documentation work on an already-built
base, not test-first. Proceed in verifiable steps.

## Context (already decided — do not reopen)
- Repo: `~/Projects/private/nxm-git`. M1+M2 approved and committed; T7 (git_pr_create)
  completed and reviewed BEFORE this task → 12 tools total.
- Server: rmcp 0.14, stateless HTTP, port **:7281**, route **/mcp**.
- Reference model for README/scripts/registration: `~/Projects/private/nxm-decision`
  (scripts `scripts/decision-up.sh`/`decision-down.sh`, on :7280). nxm-git already has
  `scripts/git-up.sh`/`git-down.sh` from T0 — verify and document them.

## Rules to follow
- Ecosystem: `nxm-docs/RULES.md` — `CHANGELOG.md` updated on every meaningful change;
  `CONTEXT.md` at end of session; `///` docs with an example on public APIs;
  Conventional Commits (`docs:`, `chore:`).
- Config/ports: do NOT conflict with :7280 (nxm-decision). nxm-git stays on :7281.
- Do NOT publish to public repos: nxm-git is private (see `nxm-sync/config.toml`;
  nxm-git is NOT yet a product there — if it must be added, visibility=private).

## Requirements
- `README.md`: what nxm-git is (typed git for agents + future TUI), dual model,
  config `~/.nxm/git-mcp.toml` (allowed_roots) and per-repo `.nxm-git-policy.toml`,
  list of the 12 tools one line each, how to start (git-up/git-down), port :7281.
- Version **0.1.0** in the workspace `Cargo.toml`.
- `CHANGELOG.md` with 0.1.0 (M1+M2+T7).
- `git-up.sh`/`git-down.sh` verified working and documented in the README.
- Registration as an MCP on the user's clients (HTTP :7281, route /mcp) + an entry in
  the `nxm-mcp` dashboard. Use `nxm-memory` to find how the other servers
  (nxm-decision, nxm-memory) are registered in the client configs, and replicate it.

## Working branch
- Create and work on a dedicated branch: `work/02-docs-and-register`.
- Do NOT commit to main. The work must be **validated before** merge.

## Available tools (use if needed)
- `nxm-memory` — find how other MCPs are registered on clients and in the dashboard.
- `nxm-decision` — categorise any forks (e.g. where the dashboard entry goes).
- `nxm-session` — read/update session state if relevant.

## Prohibitions
- Do NOT change tool logic (M1/M2/T7 are closed): only docs, version, scripts,
  registration.
- Do NOT publish nxm-git to public repos or bypass the nxm-sync FORBIDDEN guard.
- Do NOT change port :7281 or route /mcp.
- Do NOT self-approve: external review is required.

## STOP and hand off to review
- Complete ONLY this task, then STOP.
- Show the diff, the rendered README, and proof that registration works
  (`nxm-mcp status` shows nxm-git; a client lists the 12 tools; HTTP handshake ok).
- **Hand off to an external reviewer** (reference skill `code-review`). Branch
  validation **only after** approved review.
- If anything is ambiguous, STOP and ask.

## Expected verification
```bash
cargo build --release --workspace
./scripts/git-up.sh && sleep 1
# handshake + tools/list on http://127.0.0.1:7281/mcp → 12 tools
./scripts/git-down.sh
# nxm-mcp status shows nxm-git
```
