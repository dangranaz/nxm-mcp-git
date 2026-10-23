# Delega — 02 docs + versioning + registrazione MCP

> **Stato:** PRONTO · **Creato:** 2026-10-03 · **Dipende da:** 01-git-pr-create (completato e rivisto).
> Companion: `02-docs-and-register_ENG.md` (tenere in sync).
> Origine design: `nxm-docs/research/2026-10-01_nxm-git-TICKETS.md` (T8) + DESIGN.

## Obiettivo (un micro-task, poi STOP)
Rendi nxm-git rilasciabile e usabile: README (doppio modello + config + 12 tool),
versione 0.1.0, script up/down documentati, e registrazione del server come MCP sui
client dell'utente + dashboard nxm-mcp. Poi STOP.

## Skill
**`implement`** — lavoro di integrazione/configurazione e documentazione su base già
costruita, non test-first. Procedi per passi verificabili.

## Contesto (già deciso — non riaprire)
- Repo: `~/Projects/private/nxm-git`. M1+M2 approvati e committati; T7 (git_pr_create)
  completato e rivisto PRIMA di questo task → 12 tool totali.
- Server: rmcp 0.14, HTTP stateless, porta **:7281**, route **/mcp**.
- Modello di riferimento per README/script/registrazione: `~/Projects/private/nxm-decision`
  (script `scripts/decision-up.sh`/`decision-down.sh`, già su :7280). nxm-git ha già
  `scripts/git-up.sh`/`git-down.sh` da T0 — verifica e documenta.

## Rules da rispettare
- Ecosistema: `nxm-docs/RULES.md` — `CHANGELOG.md` aggiornato ad ogni cambiamento
  significativo; `CONTEXT.md` a fine sessione; `///` doc con esempio sulle API pubbliche;
  Conventional Commits (`docs:`, `chore:`).
- Config/porte: NON confliggere con :7280 (nxm-decision). nxm-git resta su :7281.
- NON pubblicare su repo pubblici: nxm-git è privato (vedi `nxm-sync/config.toml`,
  nxm-git NON è ancora un prodotto lì — se va aggiunto, visibility=private).

## Requisiti
- `README.md`: cosa è nxm-git (typed git per agenti + futura TUI), doppio modello,
  config `~/.nxm/git-mcp.toml` (allowed_roots) e `.nxm-git-policy.toml` per-repo, elenco
  dei 12 tool con una riga ciascuno, come avviare (git-up/git-down), porta :7281.
- Versione **0.1.0** nel workspace `Cargo.toml`.
- `CHANGELOG.md` con la 0.1.0 (M1+M2+T7).
- Script `git-up.sh`/`git-down.sh` verificati funzionanti e documentati nel README.
- Registrazione come MCP sui client dell'utente (HTTP :7281, route /mcp) + voce nel
  dashboard `nxm-mcp`. Cerca con `nxm-memory` come sono registrati gli altri server
  (nxm-decision, nxm-memory) nei config dei client, e replica il pattern.

## Branch di lavoro
- Crea e lavora su un branch dedicato: `work/02-docs-and-register`.
- NON committare su main. Il lavoro va **validato prima** del merge.

## Tool disponibili (usa se necessario)
- `nxm-memory` — cerca come sono registrati gli altri MCP sui client e nel dashboard.
- `nxm-decision` — categorizza eventuali bivi (es. dove mettere la voce dashboard).
- `nxm-session` — leggi/aggiorna lo stato di sessione se pertinente.

## Divieti
- NON modificare la logica dei tool (M1/M2/T7 sono chiusi): solo docs, versione, script,
  registrazione.
- NON pubblicare nxm-git su repo pubblici né bypassare il guard FORBIDDEN di nxm-sync.
- NON cambiare la porta :7281 né la route /mcp.
- NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa SOLO questo task, poi FERMATI.
- Mostra il diff, il README renderizzato, e la prova che la registrazione funziona
  (`nxm-mcp status` mostra nxm-git; un client elenca i 12 tool; handshake HTTP ok).
- **Consegna a un revisore esterno** (riferimento skill `code-review`). Validazione del
  branch **solo dopo** review approvata.
- Se c'è ambiguità, FERMATI e chiedi.

## Verifica attesa
```bash
cargo build --release --workspace
./scripts/git-up.sh && sleep 1
# handshake + tools/list su http://127.0.0.1:7281/mcp → 12 tool
./scripts/git-down.sh
# nxm-mcp status mostra nxm-git
```
