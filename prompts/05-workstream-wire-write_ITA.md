# Delega — 05 Enforcement workstream: cablaggio guardie in `write.rs` (risoluzione branch di base via git)

> **Stato:** PRONTO · **Creato:** 2026-10-18 · **Aggiornato:** 2026-10-18 (post-fmt)
> Companion: `05-workstream-wire-write_ENG.md` (tenere in sync).
> Origine design: `sessions/ses_20261018_nxmgit_workstream_worktree_enforcement.md`.
> **Dipende da:** micro-task 04, **già mergiato** in `feature/workstream-enforcement`
> (campi policy + `PolicyFlags.source_branch` + guardie pure). Il branch è anche **già
> allineato a `develop` formattato** (chore rustfmt 07): lo standard di formato del repo è
> ora `rustfmt 1.9.0` e `cargo fmt -- --check` è un gate REALE (verde sul branch oggi).
> Parti da `feature/workstream-enforcement`.
> Pipeline: `nxm-git/WORKFLOW.md`. Write: `nxm_git/src/write.rs`, resolve: `nxm_git/src/resolve.rs`.

## Obiettivo (un micro-task, poi STOP)
Collegare le guardie pure introdotte nel task 04 ai write reali in `nxm_git/src/write.rs`:
**risolvere via git** il `source_branch` (base/merge-source) e passarlo in `PolicyFlags`,
così che `check_policy` possa applicare G1 (commit diretto su develop) e G2 (base dei
`work/*`). Un solo risultato: i due write interessati (`commit`, `create_branch`) calcolano
e passano `source_branch`; nient'altro cambia.

## Skill
**`tdd`** — scrivi prima i test di integrazione su repo temporaneo (fixture esistenti in
`nxm_git/tests/`): commit diretto su `develop` bloccato/permesso secondo policy; `work/*`
creato dalla base sbagliata bloccato. Poi implementa la risoluzione git finché passano.

## Contesto (già deciso — non riaprire)
- Dal task 04 esistono: `Policy.integration_branches`, `Policy.develop_direct_commit`
  (`Deny|Confirm`), `Policy.develop_branch`, `Policy.work_branch_prefix`, e
  `PolicyFlags.source_branch: Option<String>`. `check_policy` è già puro e completo: qui
  si tratta SOLO di **popolare** `source_branch` correttamente dal lato caller.
- In `write.rs` oggi: `gate(repo, op, target_branch, confirm, allow_force)` costruisce i
  `PolicyFlags` e chiama `check_policy`. `current_branch(repo)` legge HEAD via symref.
  `run_git(repo, &[..])` esegue git come argv-array (MAI shell string) — **riusa questo
  pattern** per ogni interrogazione git, niente nuove dipendenze.
- **Semantica di `source_branch` per operazione (contratto col task 04):**
  - `commit`: `source_branch` = il branch da cui proviene il contenuto che sta per
    atterrare su `target_branch`. Per un commit "normale" sul branch corrente NON è un
    merge: `source_branch = None` (quindi su `develop` scatta `develop_direct_commit`).
    Se il commit in corso è un **merge commit** (HEAD ha, o sta per avere, >1 parent — es.
    `MERGE_HEAD` presente), risolvi il branch sorgente del merge e passalo come
    `source_branch` (così un merge `integration/* -> develop` è permesso). Rileva lo stato
    di merge in modo robusto (presenza di `MERGE_HEAD`), senza assumere.
  - `create_branch`: `source_branch` = il branch su cui si trova HEAD **al momento della
    creazione** (la base da cui nascerà il nuovo branch), cioè `current_branch(repo)`.
- **Gate di retro-compatibilità** già nella logica pura (task 04): con
  `integration_branches` vuoto, nessuna nuova negazione. Il cablaggio NON deve alterare
  questo: se la policy non attiva le guardie, `source_branch` può anche essere calcolato ma
  non cambia l'esito.

## Punti d'intervento (SOLO `write.rs`, + fixture in `nxm_git/tests/`)
- Estendi `gate` (o aggiungi un parametro/variante) perché accetti e inoltri un
  `source_branch: Option<String>` ai `PolicyFlags`. Mantieni invariata l'API per i write
  che non ne hanno bisogno (`switch`, `push`, `force_push`): passa `None`.
- In `commit`: calcola `source_branch` secondo la semantica sopra (None se non-merge;
  branch sorgente se merge in corso) e passalo al gate.
- In `create_branch`: calcola `source_branch = current_branch(repo)` e passalo al gate.
- NON cambiare `push`/`force_push`/`switch` se non per inoltrare `None` al gate esteso.

## Rules da rispettare
- Repo corrente: nessun `RULES.md` locale → stile di `write.rs` (`run_git` argv-array,
  logging `tracing::info!(target: "nxm::git::write", ...)`).
- Ecosistema: `nxm-docs/RULES.md` — niente `unwrap()`/`expect()` in produzione (ok nei
  test); errori strutturati (`WriteError`), mai free-text; English per codice/doc; max ~300
  righe/file (se `write.rs` sfora, **è autorizzato** estrarre un helper — es. funzione di
  risoluzione merge-source in un modulo dedicato; se splitti, segnalalo).
- `CHANGELOG.md`: aggiungi la voce per l'enforcement attivo lato write (rule hard: autorizzato).

## Requisiti (ognuno con il suo test su repo temp — regola invariante 7)
- R1 Commit diretto su develop bloccato: + **test** `commit_direct_on_develop_denied` —
  repo temp con policy `integration_branches=["integration/*"]`,
  `develop_direct_commit="confirm"`; su `develop`, commit normale senza confirm →
  `WriteError::Policy` con reason `DevelopDirectCommit`; git commit NON è avvenuto
  (verifica che HEAD non sia avanzato).
- R2 Commit su develop con confirm: + **test** `commit_direct_on_develop_with_confirm_ok` —
  stessa policy, `confirm=true` → il commit procede.
- R3 Merge da integration permesso: + **test** `merge_from_integration_into_develop_ok` —
  prepara un `MERGE_HEAD` (o esegui un vero merge no-ff di un `integration/x` dentro
  `develop`) e verifica che il write non sia bloccato (source risolto a `integration/x`).
- R4 Base work errata bloccata: + **test** `create_work_branch_from_main_denied` — HEAD su
  `main`, `create_branch("work/09-x")` → `WriteError::Policy` (reason base dedicato); il
  branch NON è stato creato.
- R5 Base work corretta: + **test** `create_work_branch_from_integration_ok` — HEAD su
  `integration/foo`, `create_branch("work/09-x")` → ok, branch creato.
- R6 Retro-compatibilità: + **test** `no_integration_branches_commit_on_develop_ok` — policy
  **senza** `integration_branches` → commit su `develop` senza confirm procede (come oggi).
- R7 Write non interessati invariati: + **test** `switch_and_push_unaffected` — `switch` e il
  gate di `push` si comportano come prima (source `None`, nessuna nuova negazione).

## Branch di lavoro
- Parti da `feature/workstream-enforcement` (contiene già 04 + il reformat fmt, allineato a
  `develop`). Crea `work/05-workstream-wire-write`.
- NON committare su `main` né su `develop`. Lavora su `work/*`, merge no-ff `work→feature`.
  Il merge `feature→develop` lo fa il revisore/supervisore dopo approvazione.
- Il repo è ora fmt-conforme (rustfmt 1.9.0): formatta il tuo codice nuovo con `cargo fmt`
  prima di consegnare, così `cargo fmt -- --check` resta verde (non riformattare file fuori
  scope).

## Tool disponibili (usa se necessario)
- `nxm-mcp-exec` — `run_gate {repo_path}`; "fatto" solo su `verdict:"GO"`. Fallback a `cargo`.
- `nxm-memory` — cerca i pattern dei test esistenti su repo temp (come montano remote/commit).
- `nxm-decision` — categorizza i bivi (come rilevare il merge in corso; dove estrarre l'helper).
- `nxm-session` — leggi lo stato di sessione se pertinente.

## Divieti
- NON riaprire la logica pura del task 04 (`check_policy`, i campi): qui si **popola** solo
  `source_branch`. Se scopri un buco nella logica pura, FERMATI e segnalalo (follow-up su 04),
  non aggirarlo in `write.rs`.
- NON eseguire git via shell string: solo `run_git` argv-array.
- NON toccare i tool MCP, `pr.rs`, `prepush.rs`, `config.rs`.
- NON aggiungere dipendenze. MA le rules prevalgono (split file / CHANGELOG): autorizzate.
- NON implementare la mappa worktree→workstream (è un follow-up opzionale fuori scope).
- NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa SOLO questo micro-task, poi FERMATI.
- Mostra `git diff` (write.rs + eventuale helper + test + CHANGELOG) e l'output REALE di
  `run_gate` (o `cargo test`/`clippy`/`fmt`).
- **Consegna a un revisore esterno** — riferimento skill `code-review`. Merge solo dopo review.
- Se c'è ambiguità (rilevazione merge-in-corso, come i test montano i repo temp), FERMATI e chiedi.

## Verifica attesa
```bash
# run_gate { repo_path: "~/Projects/private/nxm-git" } -> verdict GO PIENO (tutti e 4 gli step)
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt -- --check    # ora gate REALE: deve uscire 0
```
Test preesistenti verdi; i 7 nuovi test di integrazione verdi; `run_gate` deve dare GO
pieno (fmt incluso, a differenza dei task precedenti al chore 07); incolla l'output reale.
