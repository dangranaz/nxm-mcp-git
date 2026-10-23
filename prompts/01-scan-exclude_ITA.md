# Delega — 01 Aggiungere `scan_exclude` allo scan forbidden (nxm-git)

> **Stato:** PRONTO · **Creato:** 2026-10-05
> Companion: `01-scan-exclude_ENG.md` (tenere in sync).
> File: `nxm_git/src/scan.rs`, `nxm_git/src/policy.rs`. Pipeline: `nxm-git/WORKFLOW.md`.

## Perché
`scan_forbidden` applica `FORBIDDEN_PATTERNS` a TUTTO l'albero di qualsiasi repo, quindi un repo
privato inciampa su contenuto legittimo: `scan.rs` (che DEFINISCE i pattern), metadati di progetto
(`Cargo.toml`, `README.md`, `CONTEXT.md` con l'owner `draco690456` e path d'esempio `/Users/`) e
fixture di test (`pr.rs`, `http_tool.rs`). 26 hit falsi positivi bloccano `git_push`, nessuno è un
segreto reale. Il guard forbidden è pensato per il confine PUBBLICO; non deve bloccare sul contenuto
legittimo del repo stesso. Fix: lista di esclusioni guidata da policy, SENZA indebolire il
rilevamento dei segreti reali.

## Obiettivo (un micro-task, poi STOP)
Aggiungere `scan_exclude` a `Policy` e onorarlo in `scan_forbidden`: i file il cui path matcha
un'esclusione vengono saltati. Il rilevamento dei segreti reali resta intatto.

## Skill
**`tdd`** — scrivi prima i test (file escluso → nessun hit; file NON escluso con un token reale tipo
`gho_...` → ANCORA un hit), poi implementa.

## Contesto (già deciso — non riaprire)
- Crate `~/Projects/private/nxm-git`, workspace Cargo (`cargo ... --workspace`).
- `Policy` ha `#[serde(deny_unknown_fields, default)]` → il nuovo campo DEVE essere `#[serde(default)]`
  così i file policy esistenti continuano a parsare.
- Le esclusioni sono **basate su path** (path relativi al repo): supporta prefix match e/o un glob `*`
  sul path. Niente dipendenze nuove: un piccolo matcher manuale va bene.
- Default `scan_exclude`: copri i falsi positivi strutturali noti in modo generico ma STRETTO — il
  file che definisce i pattern (`nxm_git/src/scan.rs`) e le fixture di test (`**/tests/**`). NON
  escludere in blocco tutti i `*.md` o `Cargo.toml` (nasconderebbe un leak reale nei metadati); se
  resta un falso positivo nei metadati, meglio lasciarlo visibile che un'esclusione larga. Commenta
  ogni esclusione di default.
- `scan_exclude` vuoto = scansiona tutto (comportamento di oggi).

## Requisiti
1. `Policy.scan_exclude: Vec<String>` con `#[serde(default)]` + un `Default` sensato (la lista stretta
   sopra). Documenta il campo.
2. In `scan_forbidden`, salta un blob il cui path relativo matcha una entry di `scan_exclude`
   (prefix o glob `*`). L'helper di match è puro e testato.
3. Un file NON escluso che contiene un pattern proibito (es. `gho_<token>`) produce ancora un hit.
4. Aggiorna `.nxm-git-policy.toml` e `.nxm-git-policy.toml.example` di questo repo con uno
   `scan_exclude` commentato. Aggiorna `CHANGELOG.md` ([Unreleased]).

## Test (inline / dir tests secondo lo stile esistente)
- `scan_skips_excluded_paths`: file a un path escluso con un pattern → nessun hit.
- `scan_still_flags_real_secret_in_non_excluded_file`: `gho_...` in un normale file sorgente → hit.
- `empty_scan_exclude_scans_everything`: comportamento invariato.
- `scan_rs_self_is_excluded_by_default`: il default esclude il file che definisce i pattern.

## Rules da rispettare (PREVALGONO sullo scope)
- `nxm-docs/RULES.md`: max 300 righe/file → **se scan.rs/policy.rs superano le 300 dopo la modifica,
  SPLITTA per funzionalità** (è autorizzato, non è scope creep). Test in file per-modulo. Aggiorna
  CHANGELOG. Niente `unwrap`/`expect` in prod. Logging `tracing`. English nel codice.
- Se una rule confligge con lo scope, vince la rule — procedi con l'azione imposta dalla rule.

## Branch di lavoro
- Parti da `develop`. Crea `feature/scan-exclude`, poi `work/01-scan-exclude`.
- NON committare su main/develop. I merge work→feature / feature→develop li fa il revisore.

## Divieti
- NON rimuovere alcuna entry esistente di `FORBIDDEN_PATTERNS` (specie `gho_`): il rilevamento resta.
- NON rendere lo scan non-bloccante sul push. NON aggiungere dipendenze. Niente shell.
- NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa solo questo, poi FERMATI. Mostra `git diff develop..HEAD` + output REALE di verifica.
- Consegna a revisore esterno (`code-review`). Merge solo dopo approvazione.
- Se c'è ambiguità (es. semantica del glob), FERMATI e chiedi.

## Verifica attesa
```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
wc -l nxm_git/src/scan.rs nxm_git/src/policy.rs   # ciascuno < 300
```
Poi (sanity manuale, revisore): git_scan_forbidden su questo repo ritorna CLEAN o solo hit reali.
