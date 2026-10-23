# Delega — 01 git_pr_create (wrap gh pr create)

> **Stato:** PRONTO · **Creato:** 2026-10-03
> Companion: `01-git-pr-create_ENG.md` (tenere in sync).
> Origine design: `nxm-docs/research/2026-10-01_nxm-git-TICKETS.md` (T7) + DESIGN.

## Obiettivo (un micro-task, poi STOP)
Implementa il tool `git_pr_create` nel server nxm-git: un wrap di `gh pr create`
(title, body, base) che apre una PR e ritorna l'URL. MAI auto-merge. Poi STOP.

## Skill
**`tdd`** — codice nuovo con comportamento verificabile (ritorna URL, non merge mai).
Scrivi prima il test (red), poi l'implementazione (green), come nelle fasi precedenti.

## Contesto (già deciso — non riaprire)
- Repo: `~/Projects/private/nxm-git`. M1 (read) e M2 (policy + write) già approvati e
  committati (HEAD = commit M2 `e23b181`).
- Doppio modello: core lib `nxm_git` + server `nxm_git_mcp`. rmcp 0.14, HTTP stateless.
- Write via shell-out con **array di argomenti** (`Command::new("gh").args([...])`),
  MAI interpolazione di stringhe. Vedi `nxm_git/src/write.rs` (run_git) come modello.
- Il tool si aggiunge ai 11 esistenti → diventeranno 12 tool in `tools/list`.

## Rules da rispettare
- Repo corrente: `nxm-git/RULES.md` NON esiste; valgono le rules ecosistema.
- Ecosistema: `nxm-docs/RULES.md` — in particolare: NO `unwrap()`/`expect()` in
  produzione (solo nei test); `thiserror` per errori di libreria; `anyhow` + context
  a livello applicativo; logging `tracing::info!`/`debug!` con target per funzionalità;
  `///` doc con esempio su ogni API pubblica; **max 300 righe per file**; Conventional
  Commits.
- Shell-out: solo arg-array, nessun `format!` dentro il comando (come write.rs).

## Requisiti
- `pr_create(repo, title, body, base) -> Result<PrResult, ...>` nel core, con
  `PrResult { url: String }`.
- Esecuzione via `gh pr create --title <t> --body <b> --base <base>` come arg-array.
  I valori dinamici sono elementi argv separati (anti-injection), rifiuta argomenti
  che iniziano con `-` dove sono ref/branch (riusa il pattern `ensure_ref_arg` di write.rs).
- MAI `--merge`/auto-merge: il tool apre solo la PR.
- Tool MCP `git_pr_create` nel server, errore strutturato per il client (coerente con
  `WriteError::to_value`).
- Logging `tracing` dell'operazione.

## Branch di lavoro
- Crea e lavora su un branch dedicato: `work/01-git-pr-create`.
- NON committare su main. Il lavoro va **validato prima** del merge.

## Tool disponibili (usa se necessario)
- `nxm-memory` — cerca rules/contesto/precedenti (es. come nxm-sync gestisce `gh`
  e lo switch account draco690456/dangranaz).
- `nxm-decision` — categorizza eventuali bivi di implementazione.
- `nxm-session` — leggi/aggiorna lo stato di sessione se pertinente.

## Divieti
- NON uscire dallo scope: solo `git_pr_create`. NON toccare policy.rs/write.rs/i read-tool.
- NON implementare auto-merge né alcuna operazione che unisca la PR.
- NON eseguire `gh pr create` reale verso repo veri nei test: usa mock/`#[ignore]` con
  commento, oppure testa solo la costruzione dell'argv e il rifiuto input invalido.
- NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa SOLO `git_pr_create`, poi FERMATI.
- Mostra il diff e come hai verificato (`cargo clippy --workspace --all-targets` a 0
  warning, `cargo test --workspace` verde, `tools/list` = 12 tool).
- **Consegna a un revisore esterno** (riferimento skill `code-review`). Il branch si
  valida/mergia **solo dopo** review approvata.
- Se c'è ambiguità, FERMATI e chiedi.

## Verifica attesa
```bash
cargo clippy --workspace --all-targets   # 0 warning
cargo test --workspace                   # tutto verde, incl. nuovo test pr_create
# tools/list via HTTP deve elencare 12 tool (11 + git_pr_create)
```
