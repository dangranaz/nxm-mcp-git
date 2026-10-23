# Delega — 07 Chore: normalizzazione rustfmt repo-wide (`cargo fmt`)

> **Stato:** PRONTO · **Creato:** 2026-10-18
> Companion: `07-rustfmt-normalize_ENG.md` (tenere in sync).
> Origine: review del micro-task 04 (2026-10-18) — scoperto drift rustfmt pre-esistente
> repo-wide, separato dal workstream enforcement. Chore indipendente.
> Pipeline: `nxm-git/WORKFLOW.md`.

## Obiettivo (un micro-task, poi STOP)
Rendere l'intero repo `nxm-git` conforme a `cargo fmt --check` con **una sola** passata di
`cargo fmt` repo-wide. Un solo risultato: dopo il task, `cargo fmt -- --check` esce 0
(nessun file offending). Nessun cambiamento di logica: SOLO riformattazione.

## Skill
**`implement`** — chore meccanico e verificabile: lanciare `cargo fmt`, verificare che il
diff sia puramente di formattazione, confermare che i gate restano verdi.

## Contesto (già deciso — non riaprire)
- Il repo ha un drift rustfmt **pre-esistente** su ~25 file (sorgenti + test di entrambi i
  crate: `nxm_git/src/*`, `nxm_git/tests/*`, `nxm_git_mcp/src/*`, `nxm_git_mcp/tests/*`),
  causato da un mismatch di versione rustfmt (il repo fu formattato con una versione che
  ordinava gli import diversamente da `rustfmt 1.9.0-stable` di oggi: es. `{ScanResult,
  scan_forbidden}` vs `{scan_forbidden, ScanResult}`). NON è una regressione di una
  feature: è debito pregresso.
- Strada scelta dall'owner: chore **adesso**, branch dedicato **da `develop`**, separato
  dal workstream enforcement (`feature/workstream-enforcement`).
- `require_pre_push` del repo è `cargo test --workspace` (verde oggi); questo chore NON deve
  cambiare quel comportamento.

## Scope ESATTO
- Lanciare `cargo fmt` (repo-wide, workspace) una volta e committare il risultato.
- NESSUNA modifica di logica, nessun rename, nessun refactor: se `cargo fmt` propone
  qualcosa che NON è pura formattazione, FERMATI e chiedi (non dovrebbe succedere).
- NON toccare `Cargo.toml`/`Cargo.lock`, config, CHANGELOG (è un chore di formato; se una
  rule del repo imponesse comunque una voce CHANGELOG, aggiungi UNA riga chore sotto
  `[Unreleased]` e segnalalo — ma di default per un chore puro di formato si può omettere).

## Fuori scope (raccomandazione separata, NON implementare qui)
- **Pin della versione rustfmt/toolchain** (`rust-toolchain.toml`): eviterebbe il ri-drift
  futuro, MA impatta l'intera toolchain (build+clippy) per tutti ed è una decisione di
  policy del repo che spetta all'owner. **NON** aggiungere `rust-toolchain.toml` in questo
  task. Se vuoi, cita nella consegna che è la naturale prosecuzione (ticket a parte).

## Requisiti (verificabili — regola invariante 7 adattata al chore)
- R1 Conformità fmt: **comando di accettazione** `cargo fmt -- --check` → **exit 0**,
  nessun file in output. Incolla l'output reale.
- R2 Nessuna regressione funzionale: `cargo test --workspace` **verde** (stesso numero di
  test di prima, 0 falliti); `cargo clippy --workspace --all-targets -- -D warnings` pulito.
  Incolla gli output reali (o `run_gate` con verdict GO).
- R3 Diff puramente di formato: **verifica** che `git diff --stat` del commit mostri SOLO
  file `.rs` e che nessuna riga cambi la semantica (spot-check: il diff deve essere
  riordino import / wrapping / spaziatura, non logica). Dichiaralo nella consegna.

## Branch di lavoro
- Parti da `develop` (aggiornato). Crea `feature/rustfmt-normalize`, poi
  `work/07-rustfmt-normalize`.
- NON committare su `main` né su `develop` direttamente. Merge no-ff `work→feature`. Il
  merge `feature→develop` lo fa il revisore/supervisore dopo approvazione.
- ATTENZIONE (rischio noto, non un errore tuo): esiste in parallelo
  `feature/workstream-enforcement` che tocca `policy.rs`/`write.rs`/test. Un reformat
  repo-wide su `develop` creerà conflitti al merge futuro di quel branch: è previsto e
  accettato dall'owner (strada 1). Tu NON devi coordinarti con quel branch: formatta
  `develop` e basta.

## Tool disponibili (usa se necessario)
- `nxm-mcp-exec` — `run_gate {repo_path}` per R2 (GO su check+clippy+test); nota che DOPO
  questo task anche `cargo_fmt_check` deve dare `ok:true` → `run_gate` dovrebbe dare GO pieno.
- `nxm-memory` — contesto sul drift (episodic 2026-10-18, review task 04).

## Divieti
- NON cambiare logica: solo `cargo fmt`. Se il formatter tocca qualcosa di sospetto, FERMATI.
- NON aggiungere `rust-toolchain.toml` / pin (fuori scope, decisione owner).
- NON formattare "a mano" selettivamente: usa `cargo fmt` così il risultato è deterministico.
- NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa SOLO questo chore, poi FERMATI.
- Mostra `git diff --stat` del commit e l'output REALE di `cargo fmt -- --check` (exit 0),
  `cargo test --workspace`, `cargo clippy ... -D warnings` (o `run_gate` → GO).
- **Consegna a un revisore esterno** — riferimento skill `code-review`. Merge solo dopo review.
- Conferma esplicitamente che il diff è puramente di formato (R3).

## Verifica attesa
```bash
cargo fmt -- --check        # exit 0, nessun file
cargo test --workspace      # verde, 0 falliti
cargo clippy --workspace --all-targets -- -D warnings   # pulito
# oppure: run_gate { repo_path: "~/Projects/private/nxm-git" } -> verdict GO (tutti e 4 gli step)
```
`run_gate` ora deve dare **GO pieno** (incluso `cargo_fmt_check: ok`), a differenza di prima.
