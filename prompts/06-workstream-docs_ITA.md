# Delega — 06 Enforcement workstream: documentazione (`.example` + `WORKFLOW.md`)

> **Stato:** PRONTO · **Creato:** 2026-10-18 · **Aggiornato:** 2026-10-18 (post-05)
> Companion: `06-workstream-docs_ENG.md` (tenere in sync).
> Origine design: `sessions/ses_20261018_nxmgit_workstream_worktree_enforcement.md`.
> **Dipende da:** micro-task 04 e 05, **entrambi già mergiati** in
> `feature/workstream-enforcement` (campi policy + guardie + cablaggio write.rs
> worktree-safe). Il branch è anche allineato a `develop` formattato (chore 07): lo standard
> di formato è `rustfmt 1.9.0` e `cargo fmt -- --check` è verde sul branch. Parti da
> `feature/workstream-enforcement`: i nomi/semantica dei campi sono ora DEFINITIVI nel codice.
> File: `.nxm-git-policy.toml.example`, `WORKFLOW.md`.

## Obiettivo (un micro-task, poi STOP)
Allineare la documentazione all'enforcement ora attivo: documentare i nuovi campi policy in
`.nxm-git-policy.toml.example` e **rimuovere il marcatore "soft convention / not yet
enforced"** dalla sezione worktree di `WORKFLOW.md`, linkando i campi policy. Solo docs:
nessun codice, nessun test di comportamento.

## Skill
**`implement`** — edit mirati e consistenti ai due file di documentazione, verificando che i
nomi/default citati corrispondano ESATTAMENTE al codice mergiato da 04/05.

## Contesto (già deciso — non riaprire)
- Dopo 04/05 esistono i campi policy: `integration_branches` (default vuoto),
  `develop_direct_commit` (`"deny"|"confirm"`, default `"confirm"`), `develop_branch`
  (default `"develop"`), `work_branch_prefix` (default `"work/"`). **Leggi i valori reali
  dal codice mergiato** (`nxm_git/src/policy.rs`) prima di scrivere: la doc deve riflettere
  il codice, non questo brief se il codice è divergente.
- `WORKFLOW.md` contiene oggi la sezione *"Parallel workstreams in isolated worktrees"*
  (riga ~85) con un marcatore (righe ~87–89) che la dichiara convenzione **soft /
  documentary / non ancora enforced**:
  `> **Status: soft convention (documentary).** This section is process, not yet enforced
  by nxm-git...`. Dopo 04/05 è enforced → questo blocco-marcatore va rimosso e sostituito
  con un rimando ai campi policy che la applicano (es. "enforced by `nxm-git` via the
  `integration_branches` / `develop_direct_commit` policy fields").
- `.nxm-git-policy.toml.example` è il template commentato: i nuovi campi vanno aggiunti
  **commentati** (come gli altri opzionali), con spiegazione breve e default esplicitato.

## Requisiti (verificabili — regola invariante 7 adattata ai docs)
- R1 `.example` documenta i 4 nuovi campi, commentati, con default corretti e una riga di
  spiegazione ciascuno. **Verifica:** i nomi dei campi e i default citati corrispondono 1:1 a
  `Policy::default()` in `policy.rs` (confronto diretto; cita il diff del codice sorgente).
- R2 Coerenza enum: il campo `develop_direct_commit` documenta i valori `"deny"` e
  `"confirm"` e quale è il default. **Verifica:** valori e default combaciano con l'enum
  `DevelopDirectCommit` nel codice.
- R3 `WORKFLOW.md`: la sezione worktree NON contiene più il marcatore "soft / not yet
  enforced". **Verifica:** `grep -ni "not yet enforced\\|soft convention" WORKFLOW.md` non
  restituisce più la riga della sezione worktree (o è stata riscritta come "enforced").
- R4 `WORKFLOW.md` linka i campi policy che applicano la convenzione (nome dei campi +
  rimando a `.nxm-git-policy.toml`). **Verifica:** i nomi dei campi citati in WORKFLOW.md
  esistono in `policy.rs` (nessun nome inventato).
- R5 Nessuna regressione fattuale: la doc non promette comportamenti non implementati (es.
  NON documentare la mappa worktree→workstream, che è un follow-up non implementato).

## Branch di lavoro
- Parti da `feature/workstream-enforcement` (dopo 04 e 05). Crea `work/06-workstream-docs`.
- NON committare su `main` né su `develop`. Merge no-ff `work→feature`. Il merge
  `feature→develop` lo fa il revisore/supervisore dopo approvazione.

## Tool disponibili (usa se necessario)
- `nxm-memory` — rileggi `policy.rs` e `WORKFLOW.md` per allineare i nomi.
- `nxm-decision` — categorizza i bivi (dove collocare il rimando; wording).
- `nxm-session` — leggi lo stato di sessione se pertinente.

## Divieti
- NON toccare codice (`*.rs`), tool MCP, test. Solo `.nxm-git-policy.toml.example` e
  `WORKFLOW.md`.
- NON documentare feature non implementate (mappa worktree→workstream).
- NON inventare nomi/default: devono combaciare col codice mergiato; se divergono, FERMATI e
  segnala (la doc non deve "aggiustare" il codice).
- NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa SOLO questo micro-task, poi FERMATI.
- Mostra `git diff` dei due file e l'output dei `grep` di verifica (R3/R4).
- **Consegna a un revisore esterno** — riferimento skill `code-review`. Merge solo dopo review.
- Se i nomi/default nel codice divergono dal brief, FERMATI e chiedi.

## Verifica attesa
```bash
grep -ni "not yet enforced\|soft convention" WORKFLOW.md
grep -nE "integration_branches|develop_direct_commit|develop_branch|work_branch_prefix" \
  .nxm-git-policy.toml.example WORKFLOW.md nxm_git/src/policy.rs
# i nomi/default in .example e WORKFLOW.md combaciano con policy.rs
```
Nessun marcatore "soft" residuo nella sezione worktree; i 4 campi documentati coerenti col codice.
