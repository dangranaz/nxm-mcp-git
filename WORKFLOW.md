# NXM Git Workflow

The branching pipeline every NXM repository follows. It exists so the history
stays **readable**: when something goes wrong you can see exactly what entered,
from where, and when. `nxm-git` is the tool we use to drive and guard it.

## Principles

- **`main` is never touched directly.** It only ever receives merges from
  `develop`. It is a protected branch (see policy below).
- **History is explicit.** Feature→develop and develop→main merges are always
  `--no-ff`: each produces a merge commit, so every feature and every promotion
  is a visible, revertable node in the graph. We trade a flatter log for
  traceability on purpose.
- **Guards are friction, not a cage.** `nxm-git` policy prevents the distracted
  mistake (direct commit to `main`, unflagged force-push, forbidden content).
  An intentional operation still passes with an explicit flag.

## The branch hierarchy

```
main            ← only merges from develop (--no-ff), after a trial period
  └── develop   ← integration; features land here first and are tried out
        └── feature/<name>      ← one feature, branched from develop
              └── work/<nn>-<slug>  ← one micro-task (often a delegated brief)
```

## The pipeline, step by step

1. **`main` is frozen.** Never commit to it, never branch work from it.
2. **Start from `develop`.** Always branch from an up-to-date `develop`.
3. **Create a feature branch:** `feature/<name>` from `develop`.
4. **Create a work branch** off the feature branch: `work/<nn>-<slug>` — this is
   where a single micro-task (e.g. a delegated brief) is executed.
5. **Validate the work branch**, then **merge it into its feature branch and
   close it** (delete `work/*`). A work branch is validated by: review (external
   reviewer / `code-review`) + independent re-run of build/test/lint.
6. **When the whole feature is safe**, merge `feature/<name>` into `develop`
   with `--no-ff`. Delete the feature branch after merge.
7. **After a trial period on `develop`**, merge `develop` into `main` with
   `--no-ff`. This is the promotion of everything that survived the trial.
8. **A new feature born during develop's trial period** is worked in parallel
   safely on its own `feature/*`. When that new feature is complete, in order:
   **(a) first align `main` with `develop`** (promote what passed the trial —
   step 7), **then (b)** merge the new `feature/*` into `develop` (step 6). This
   keeps each trial window starting from a clean, promoted base.

## Merge style

| Transition | Style | Why |
|---|---|---|
| `work/*` → `feature/*` | `--no-ff` (or ff for a trivial single commit) | Keep the micro-task boundary visible. |
| `feature/*` → `develop` | **`--no-ff`** | One merge commit per feature. |
| `develop` → `main` | **`--no-ff`** | One merge commit per promotion/release. |

## Releases & tags

- Versioning is **SemVer**, per the ecosystem rules (`nxm-docs/RULES.md`:
  internal deps via pinned git tags).
- A feature (new, backward-compatible capability) is a **minor** bump
  (`0.1.0 → 0.2.0`). A fix is a **patch** bump. Breaking changes bump **major**
  (post-1.0) or minor (pre-1.0), flagged explicitly.
- Tags are **per-crate**: `<crate>-v<version>` (e.g. `nxm-session-mcp-v0.2.0`),
  created on the promotion commit on `main`.
- **Open point:** multi-crate workspaces today share one `workspace.package.version`,
  so a per-crate bump raises every member. Per-crate versions are a future
  refactor; until then a bump is workspace-wide and the tag names the crate that
  motivated it.

## How `nxm-git` enforces this

The *process* above is convention; `nxm-git` mechanically guards the dangerous
edges via each repo's `.nxm-git-policy.toml` (applied by the core lib, inherited
by both the agent and the TUI):

- `protected_branches = ["main"]` → `git_commit`/`git_push`/`git_pr_create`
  against `main` refuse unless `confirm=true`.
- `allow_force = false` → no force-push without flipping policy *and* passing
  `allow_force` + `confirm` per call.
- `forbidden_patterns` → `git_push` refuses content that must never ship.

See `.nxm-git-policy.toml.example` in this repo for a template that encodes the
pipeline's protected branches.

## Parallel workstreams in isolated worktrees (convention)

> **Status: enforced by `nxm-git`.** This convention is applied mechanically
> through the policy fields in `.nxm-git-policy.toml`:
> `integration_branches` (default empty), `develop_direct_commit`
> (`"confirm"`/`"deny"`, default `"confirm"`), `develop_branch` (default
> `"develop"`), and `work_branch_prefix` (default `"work/"`). See
> `.nxm-git-policy.toml.example` for the template.

When several workstreams run in parallel on the same repository (e.g. one agent
on a memory subsystem, another on sampling), sharing a single working tree is a
hazard: branch switches and auto-stashes under one workstream silently move or
park another's uncommitted work. The convention that prevents this:

- **One stable worktree per workstream**, for its whole lifetime:
  `.worktrees/<workstream>` (e.g. `.worktrees/filone3-sampling`). A workstream
  lives there until its release; it does **not** operate on the shared main
  checkout.
- **One integration branch per workstream**: `integration/<workstream>`,
  branched from `develop`. This is the workstream's private landing zone — the
  analogue of `feature/<name>` in the hierarchy above, but held in its own
  worktree so no other workstream can disturb it.

> **Tooling.** The two steps above are materialised by two `nxm-git` tools, so
> the hierarchy is created mechanically from the right base (never the current
> checkout): `git_create_integration_branch { workstream }` makes
> `integration/<workstream>` from `develop`, then `git_worktree_add
> { workstream }` adds its stable worktree under `.worktrees/`. The worktree
> tool refuses to run before its integration branch exists, so the order cannot
> be inverted. A `work/<nn>-<slug>` is then created from the integration branch
> via `git_create_branch { name, base: "integration/<workstream>" }`.

- **Approved micro-tasks land on the integration branch, not on `develop`.**
  Each `work/<nn>-<slug>` is reviewed and, once APPROVED, merged into
  `integration/<workstream>` (the feature boundary rule — `--no-ff`, or ff for a
  trivial single commit — still applies).
- **Sync to `develop` at milestones, not per task.** `integration/<workstream>`
  is merged into `develop` (`--no-ff`, step 6) at a milestone boundary (e.g. a
  completed sub-track), **not** on every approved task. This keeps `develop`
  stable for the other workstreams and keeps the workstream off `develop`'s
  constant churn. Rationale: a busy `develop` (many parallel merges) would force
  a rebase on every task if the workstream synced per-task; milestone sync
  rebases once per milestone instead.
- **Pull `develop` into the integration branch when you must**, not reflexively:
  realign `integration/<workstream>` onto an advanced `develop` only when a
  conflict or a needed upstream change forces it, so divergence stays bounded
  without churn.

Updated hierarchy with workstream isolation:

```
main                               ← only merges from develop (--no-ff)
  └── develop                      ← integration of ALL workstreams
        └── integration/<workstream>   ← one workstream, its own stable worktree
              └── work/<nn>-<slug>     ← one reviewed micro-task
```

| Transition | Style | When |
|---|---|---|
| `work/*` → `integration/<workstream>` | `--no-ff` (ff if trivial) | per approved task |
| `integration/<workstream>` → `develop` | **`--no-ff`** | per **milestone** |
| `develop` → `integration/<workstream>` | ff / rebase | only when forced (conflict/upstream need) |

The `develop` → `main` promotion (step 7) is unchanged.
