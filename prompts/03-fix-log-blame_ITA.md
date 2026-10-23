# Delega — 03 fix M1: implementa git_log e git_blame (stub) + conteggi diff esatti

> **Stato:** PRONTO · **Creato:** 2026-10-03 · **Priorità:** PRIMA del merge di T8/0.1.0.
> Companion: `03-fix-log-blame_ENG.md` (tenere in sync).
> Origine: scoperta in review T8 — due read-tool di M1 sono stub che ritornano vuoto.

## Obiettivo (un micro-task, poi STOP)
Implementa davvero `git_log` e `git_blame` nel core (oggi ritornano liste vuote),
con test su dati reali. In più, rendi esatti i conteggi +/- di `git_diff`. Poi STOP.

## Skill
**`tdd`** — c'è un comportamento osservabile e sbagliato (output vuoto) da portare
a verde con test su un repo reale. Scrivi prima il test che fallisce (red) mostrando
il vuoto attuale, poi implementa (green).

## Contesto (già deciso — non riaprire)
- Repo: `~/Projects/private/nxm-git`. Backend read via **git2** (deciso in M1).
- `nxm_git/src/log.rs`: `pub fn log(...)` ha `// Dummy implementation for now` e
  ritorna `LogResult { commits: Vec::new() }`. Lo struct `CommitEntry { sha, author,
  author_email, date, subject }` è già definito e wired nel server — NON cambiarlo.
- `nxm_git/src/blame.rs`: `pub fn blame(...)` apre il blame ma ha
  `// TODO: Implement proper iteration` e ritorna `BlameResult { lines: Vec::new() }`.
  Lo struct `BlameLine { line_no, commit, author, content }` è già definito e wired.
- `nxm_git/src/diff.rs`: i conteggi usano `hunk.new_lines()/old_lines()`
  (approssimazione tipo --stat). Lo struct `FileDiff { path, additions, deletions }`
  NON ha un campo hunks — non aggiungerlo. Rendi i conteggi esatti usando il callback
  per-linea (`line.origin()` '+'/'-'), senza cambiare la forma del risultato.
- Esistono già `status.rs` e `scan.rs` come modello di stile git2 + test con TempDir.

## Rules da rispettare
- Ecosistema: `nxm-docs/RULES.md` — NO `unwrap()`/`expect()` in produzione (solo test);
  `anyhow::Result` + `.context(...)`; logging `tracing` con target per-funzionalità;
  `///` doc con esempio sulle API pubbliche; **max 300 righe per file**; Conventional
  Commits (`fix:`/`feat:`); test obbligatori prima del merge.

## Requisiti
- `log(repo, range, limit)`: ritorna i commit reali (sha, author, author_email, date,
  subject). Rispetta `range` (es. "A..B") e `limit`. Ordine: newest-first.
- `blame(repo, path)`: ritorna una `BlameLine` per riga con commit/author/content reali.
- `diff`: `additions`/`deletions` per file e totali esatti (conteggio per-linea), stessa
  forma del risultato.
- **Test su dati reali** per ciascuno (TempDir con commit noti): asserisci valori
  concreti, non solo "non vuoto". Un test tipo `commits.len() == N`, blame che attribuisce
  la riga al commit giusto, diff con +X/-Y verificati.

## Branch di lavoro
- Crea e lavora su un branch dedicato: `work/03-fix-log-blame`.
- NON committare su main. Validazione **prima** del merge.

## Tool disponibili (usa se necessario)
- `nxm-memory` — cerca il pattern git2 già usato in status.rs/scan.rs.
- `nxm-decision` — categorizza eventuali bivi (es. formato date, gestione range).
- `nxm-session` — leggi/aggiorna lo stato di sessione se pertinente.

## Divieti
- NON cambiare la forma degli struct pubblici (CommitEntry, BlameLine, FileDiff,
  LogResult, BlameResult, DiffResult) né le firme dei tool: solo riempire le implementazioni.
- NON toccare policy.rs/write.rs/pr.rs/i tool già funzionanti.
- NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa SOLO log + blame + conteggi diff, poi FERMATI.
- Mostra il diff e l'output di verifica. **Consegna a un revisore esterno** (skill
  `code-review`). Il branch si valida/mergia **solo dopo** review approvata.
- Se c'è ambiguità, FERMATI e chiedi.

## Verifica attesa
```bash
cargo clippy --workspace --all-targets   # 0 warning
cargo test --workspace                   # verde, con NUOVI test su dati reali per log/blame/diff
# una chiamata git_log su un repo con N commit ritorna N commit (non [])
# una chiamata git_blame su un file ritorna una riga per linea con il commit giusto
```
