# Delega — 05b Follow-up: `merge_source_branch` worktree-safe (`git rev-parse --git-path`)

> **Stato:** PRONTO · **Creato:** 2026-10-18
> Companion: `05b-merge-source-worktree-safe_ENG.md` (tenere in sync).
> Origine: review del micro-task 05 (NEEDS_CHANGES, 2026-10-18).
> **Dipende da:** il branch del task 05 (`work/05-workstream-wire-write`, NON ancora
> mergiato). Questo follow-up si applica a QUEL branch, non riparte da zero.
> Pipeline: `nxm-git/WORKFLOW.md`. Write: `nxm_git/src/write.rs`.

## Obiettivo (un micro-task, poi STOP)
Correggere l'unico difetto bloccante emerso in review del task 05: `merge_source_branch`
in `nxm_git/src/write.rs` costruisce un path hard-coded `repo.path.join(".git/MERGE_HEAD")`
che **non funziona nei worktree git secondari** — proprio lo scenario che questo workstream
deve supportare. Renderlo worktree-safe e aggiungere il test mancante. Nient'altro.

## Skill
**`tdd`** — scrivi PRIMA il test che fallisce (merge preparato in un **worktree secondario**,
source non risolto con il codice attuale), poi correggi finché passa.

## Contesto (già deciso — non riaprire)
- **Il bug (verificato empiricamente in review):** in un worktree git secondario `.git` è un
  *file* (`gitdir: ...`), e `MERGE_HEAD` vive in `.git/worktrees/<nome>/MERGE_HEAD`, NON in
  `<worktree>/.git/MERGE_HEAD`. Quindi `repo.path.join(".git/MERGE_HEAD").is_file()` è sempre
  `false` nei worktree → `merge_source_branch` ritorna `None` → un merge legittimo
  `integration/* -> develop` eseguito DA un worktree viene erroneamente bloccato da
  `develop_direct_commit`.
- `merge_source_branch` è **l'unico** `.git` hard-coded nel sorgente: tutto il resto usa
  `git2::Repository` (`ResolvedRepo::inner`), che risolve il gitdir del worktree
  correttamente. Questo fix riallinea alla convenzione del repo.
- **Fix deciso:** sostituire il path hard-coded con il path risolto da git stesso:
  `git rev-parse --git-path MERGE_HEAD` (via il pattern `run_git` argv-array già presente)
  restituisce il path corretto SIA nel repo principale (`.git/MERGE_HEAD`) SIA nel worktree
  (`.git/worktrees/<nome>/MERGE_HEAD`). Poi controlla l'esistenza di QUEL path.
  - In alternativa equivalente, `repo.inner.path()` di git2 dà il gitdir reale; va bene
    anche quello, purché il risultato sia worktree-safe. Scegli UNA via (preferita:
    `rev-parse --git-path`, coerente con l'uso di `name-rev` già lì) e sii coerente.
- La semantica a valle resta identica: merge in corso → risolvi il source via
  `git name-rev --name-only MERGE_HEAD`; nessun merge o nome non risolto → `None` (fail-safe).

## Punti d'intervento (SOLO `write.rs` + `tests/workstream.rs`)
- `merge_source_branch`: sostituisci la costruzione del path. Mantieni: `run_git` argv-array,
  nessuna shell string, nessun `unwrap`/`expect` in produzione, fail-safe su `None`.
- NON cambiare `commit`/`create_branch`/`gate`/la logica pura del task 04.

## Rules da rispettare
- Stile `write.rs` (`run_git`, logging `tracing`). Ecosistema `nxm-docs/RULES.md`: niente
  `unwrap`/`expect` in produzione, errori strutturati, English, ~300 righe/file.
- `CHANGELOG.md`: se c'è già la voce del task 05 sotto `[Unreleased]`, **non** duplicarla;
  al più aggiorna la riga per riflettere il fix worktree-safe (una riga). Non inventare una
  release nuova.

## Requisiti (ognuno con il suo test — regola invariante 7)
- R1 Worktree-safe (il test che mancava): + **test** `merge_from_integration_in_worktree_ok`
  — crea un repo temp, poi un **worktree secondario** (`git worktree add`), prepara lì un
  merge in corso di un `integration/*` dentro `develop` (`git merge --no-ff --no-commit`),
  apri il `ResolvedRepo` sul PATH DEL WORKTREE, e verifica che `commit(...)` risolva il
  source a `integration/*` e **passi** G1 senza confirm. Questo test DEVE fallire con il
  codice attuale (path hard-coded) e passare dopo il fix.
- R2 Non-regressione repo principale: + i 7 test esistenti di `tests/workstream.rs` restano
  verdi (in particolare `merge_from_integration_into_develop_ok` sul repo principale).
- R3 Nessun merge in corso → `None`: comportamento invariato (coperto dai test esistenti su
  develop senza merge; se non c'è un test diretto su `merge_source_branch=None`, aggiungine
  uno leggero).

## Branch di lavoro
- Continua sul branch del task 05: `work/05-workstream-wire-write` (il fix fa parte dello
  stesso micro-task, bocciato in review). NON aprire un nuovo work branch parallelo.
- NON committare su `develop`/`main`. Il merge `work/05 -> feature/workstream-enforcement`
  lo fa il revisore/supervisore DOPO che questo follow-up è approvato.

## Tool disponibili (usa se necessario)
- `nxm-mcp-exec` — `run_gate {repo_path}`; "fatto" solo su `verdict:"GO"` PIENO (fmt incluso).
- `nxm-memory` — contesto sul bug (episodic review task 05, 2026-10-18).
- `nxm-decision` — se emergesse un bivio (rev-parse vs git2 path), categorizzalo.

## Divieti
- NON riaprire la logica pura del task 04 né cambiare `commit`/`create_branch`/`gate` oltre
  la risoluzione del path.
- NON usare path `.git` hard-coded: usa un path risolto da git (worktree-safe).
- NON eseguire git via shell string: solo `run_git` argv-array.
- NON aggiungere dipendenze. NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa SOLO questo fix, poi FERMATI.
- Mostra `git diff nxm_git/src/write.rs nxm_git/tests/workstream.rs` e l'output REALE di
  `run_gate` (GO pieno). Mostra esplicitamente che il nuovo test R1 FALLISCE sul codice
  pre-fix e PASSA dopo (puoi descriverlo o mostrare i due run).
- **Consegna a un revisore esterno** — riferimento skill `code-review`. Merge solo dopo review.

## Verifica attesa
```bash
# run_gate { repo_path: "~/Projects/private/nxm-git" } -> verdict GO PIENO
cargo test --workspace            # 7 esistenti + 1 nuovo (worktree) verdi
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt -- --check              # exit 0
```
Prova empirica attesa del fix: `git rev-parse --git-path MERGE_HEAD` in un worktree dà
`.git/worktrees/<nome>/MERGE_HEAD`, non `.git/MERGE_HEAD`.
