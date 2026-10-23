# Delega — 01 Enforce `require_pre_push` (hard gate) in nxm-git

> **Stato:** PRONTO · **Creato:** 2026-10-04
> Companion: `01-enforce-require-pre-push_ENG.md` (tenere in sync).
> Pipeline: `nxm-git/WORKFLOW.md`. Policy: `nxm_git/src/policy.rs`, write: `nxm_git/src/write.rs`.

## Obiettivo (un micro-task, poi STOP)
Rendere **effettivo** il campo `Policy.require_pre_push`: oggi è dichiarato ma non eseguito.
In `nxm_git/src/write.rs`, nella funzione `push`, eseguire i comandi di `require_pre_push`
come **hard gate** PRIMA del push: se un comando fallisce, il push NON parte e si ritorna un
errore strutturato. Nient'altro.

## Skill
**`tdd`** — scrivi prima i test (pre-push che passa → push procede; pre-push che fallisce → push
bloccato e git non gira), poi implementa finché passano.

## Contesto (già deciso — non riaprire)
- Crate: `~/Projects/private/nxm-git` (workspace Cargo). Core lib: `nxm_git`.
- Oggi `write::push` fa, in ordine: `gate(policy)` → `scan_forbidden` → `run_git push`.
  I comandi pre-push vanno eseguiti **dopo lo scan_forbidden e prima di `run_git push`**.
- `Policy::load(&repo.path)` è già chiamata dentro `gate`; qui serve ri-caricare la policy in
  `push` (o estendere la logica) per leggere `require_pre_push`. Mantieni `gate` com'è
  (è condiviso da commit/branch/switch) e aggiungi il passo pre-push SOLO in `push`.
- `require_pre_push: Vec<String>` sono comandi tipo `"cargo test --workspace"`: splitta su
  whitespace in argv ed esegui con `std::process::Command` nella `repo.path` (stessa modalità
  argv-array di `run_git`, MAI stringa interpolata in shell). Vuoto = nessun comando (no-op).
- Hard gate: un comando con exit status ≠ 0 blocca il push.

## Rules da rispettare
- Repo corrente: nessun `RULES.md` locale → stile dei file esistenti (`write.rs`).
- Ecosistema: `nxm-docs/RULES.md` — niente `unwrap()`/`expect()` in produzione (ok nei test);
  logging `tracing::info!(target: "nxm::git::write", ...)` sullo stile già presente; English per codice/doc.
- Errori strutturati con `thiserror` (lo stile di `WriteError`): niente free-text sciolto.

## Requisiti
1. Nuovo variant in `WriteError`, es. `PrePushFailed { command: String, stderr: String }`
   (git non è stato eseguito). Aggiungi il relativo caso in `WriteError::to_value()` con
   `"kind": "pre_push_failed"`, `command`, `stderr` (coerente con gli altri payload).
2. In `push`, dopo `scan_forbidden` e prima di `run_git push`: per ogni comando in
   `policy.require_pre_push`, eseguilo in `repo.path`; al primo fallimento ritorna
   `WriteError::PrePushFailed` (git push NON parte). Logga entry/exit del passo pre-push.
3. Un comando vuoto o lista vuota = nessun gate (comportamento invariato per repo senza pre-push).
4. Esecuzione argv-array (split whitespace), mai shell string. Comando non trovato / spawn error
   → mappalo a un errore chiaro (riusa `PrePushFailed` o `Other`, motivando).

## Test (inline, stile dei test di write.rs / tests/write.rs)
- `push_runs_pre_push_and_proceeds_when_ok`: policy con `require_pre_push=["true"]` (comando che
  passa) → il push procede (o, se il push reale è scomodo nei test, verifica che il gate pre-push
  non blocchi; segui il pattern dei test esistenti su un repo temp con remote fittizio).
- `push_blocked_when_pre_push_fails`: policy con `require_pre_push=["false"]` → `WriteError::PrePushFailed`,
  e verifica che il push NON sia avvenuto (nessun commit/ref spinto).
- `empty_pre_push_is_noop`: nessun comando → comportamento come oggi.

## Branch di lavoro
- Parti da `develop` (aggiornato). Crea `feature/enforce-pre-push`, poi `work/01-enforce-pre-push`.
- NON committare su `main` né su `develop` direttamente. Lavora su `work/*`, merge no-ff work→feature.
  Il merge feature→develop lo fa il revisore/supervisore dopo l'approvazione.

## Tool disponibili (usa se necessario)
- `nxm-memory` — cerca pattern simili (run_git, gate) nel repo.
- `nxm-decision` — categorizza i bivi (es. riuso Other vs nuovo variant).
- `nxm-session` — leggi lo stato di sessione se pertinente.

## Divieti
- NON cambiare la semantica di `check_policy`, `gate`, né degli altri write (commit/branch/switch/forcepush).
- NON eseguire i comandi via shell (`sh -c`): solo argv-array.
- NON aggiungere dipendenze. NON uscire dallo scope (solo write.rs + suoi test).
- NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa SOLO questo micro-task, poi FERMATI.
- Mostra `git diff nxm_git/src/write.rs` (+ eventuali test) e l'output REALE della verifica.
- **Consegna a un revisore esterno** (altro agent/persona) — riferimento skill `code-review`.
  Il branch si valida/mergia **solo dopo** review approvata.
- Se c'è ambiguità (es. come i test esistenti simulano il push), FERMATI e chiedi.

## Verifica attesa
```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
I test preesistenti restano verdi; i 3 nuovi test verdi; incolla l'output reale.
