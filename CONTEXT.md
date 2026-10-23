# CONTEXT — nxm-git

> Ultimo aggiornamento: 2026-10-03 (task 02 — docs + versioning + registrazione MCP)

## Cos'è questo progetto

Workspace Rust che implementa un **backend Git tipizzato** per agenti: le operazioni
Git che un agente deve fare diventano **tool MCP con schema e output JSON
strutturato**, invece di shell-out a `git` con parsing di testo. Fa parte
dell'ecosistema **Nexum**, repo privato `draco690456/nxm-git`.

Il valore dichiarato è **ergonomia + portabilità**, non sicurezza garantita:
complementare (non sostitutivo) dell'hook `git-guardrails-claude-code`.

## Doppio modello

Due crate, un core (stessa forma di nxm-decision / nxm-memory):

- **`nxm-git`** — libreria core. Git tipizzato via `git2` (lato read, dati nativi,
  niente parsing) e shell-out con array di argomenti stricti (lato write, parità con
  git/gh reali). Nessuna conoscenza dell'MCP.
- **`nxm-git-mcp`** — server MCP sottile sopra il core. Espone i 12 tool, il
  transport e la config. Non aggiunge logica Git.

Due consumatori, un core: l'**agente** via MCP (processo HTTP condiviso), la
**TUI** (futura) in-process, che eredita le stesse guardie per gratis.

## Transport

MCP Streamable HTTP **stateless** (`stateful_mode=false`) su `127.0.0.1:7281`,
route `/mcp`. Condiviso da tutti i client. Anche stdio quando non viene dato un
indirizzo: `--http <ADDR>` vince su `NXM_GIT_MCP_HTTP_ADDR`.

## Config

- **`~/.nxm/git-mcp.toml`** — perimetro di sicurezza: `allowed_roots`. Il server
  rifiuta qualsiasi repo fuori dalle root. File assente = allow-list vuota = tutto
  rifiutato.
- **`.nxm-git-policy.toml`** (per repo, committato) — `protected_branches`,
  `allow_force`, `destructive_confirm`, `forbidden_patterns`, `require_pre_push`.
  Applicata dal core, quindi ereditata da entrambi i consumatori. `deny_unknown_fields`:
  un refuso è un errore di caricamento, una branch non resta mai scoperta per errore.

## I 12 tool

Read (git2, JSON strutturato): `git_status`, `git_diff`, `git_log`,
`git_branches`, `git_show`, `git_blame`, `git_scan_forbidden`.

Write (shell-out, policy-gated dal core): `git_commit`, `git_create_branch`,
`git_switch`, `git_push`, `git_pr_create`.

## Stato 0.1.0

- Tutti i 12 tool sono implementati e coperti da test su dati reali (59 test).
- `git_log`/`git_blame`/`git_diff` verificati contro `git` reale (log history,
  blame incl. righe non committate, diff per-linea cross-checked con `--numstat`).

## Operazioni

```bash
cargo build --release --workspace
./scripts/git-up.sh      # avvia :7281/mcp (idempotente)
./scripts/git-down.sh    # ferma
nxm-mcp status           # dashboard dei server locali
```

## Stato

Versione **0.1.0**. M1 (read) + M2 (policy + write) + T7 (`git_pr_create`)
committati su `main`; task 02 (README, CHANGELOG, script, registrazione MCP)
su `work/02-docs-and-register`, in attesa di review esterna.