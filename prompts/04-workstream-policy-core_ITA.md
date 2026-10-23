# Delega — 04 Enforcement workstream: campi policy + logica di guardia (core puro in `policy.rs`)

> **Stato:** PRONTO · **Creato:** 2026-10-18
> Companion: `04-workstream-policy-core_ENG.md` (tenere in sync).
> Origine design: `sessions/ses_20261018_nxmgit_workstream_worktree_enforcement.md`.
> Pipeline: `nxm-git/WORKFLOW.md`. Core policy: `nxm_git/src/policy.rs`.

## Obiettivo (un micro-task, poi STOP)
Estendere **solo** `nxm_git/src/policy.rs` con i nuovi campi policy e la **logica di
guardia pura** (nessun git, nessun side-effect) che applica la convenzione dei filoni
paralleli. Non toccare `write.rs` (è il micro-task 05). Un solo risultato: `Policy` più
ricca + `check_policy` che applica le nuove regole, tutto coperto da test di unità.

## Skill
**`tdd`** — scrivi prima i test (commit diretto su `develop` senza confirm → negato;
merge da `integration/*` → passa; `work/*` creato da `main` → negato), poi implementa
campi + logica finché passano. `check_policy` è puro: testabile senza git.

## Contesto (già deciso — non riaprire)
- Crate: `~/Projects/private/nxm-git` (workspace Cargo). Core lib: `nxm_git`.
- `Policy` oggi (`#[serde(deny_unknown_fields, default)]`) ha: `protected_branches`,
  `allow_force`, `destructive_confirm`, `forbidden_patterns`, `scan_exclude`,
  `require_pre_push`. `check_policy(op, &Policy, &PolicyFlags) -> Result<(), PolicyDenied>`
  è **puro**: non tocca git. Oggi `CreateBranch`/`Switch` sono sempre permessi.
- `PolicyFlags` ha: `confirm`, `allow_force`, `target_branch: Option<String>`.
- **Decisione di design (contratto con il task 05, NON riaprire):** per sapere se un
  commit/merge su `develop` "proviene da un `integration/*`" o da quale base nasce un
  `work/*`, `check_policy` NON interroga git. Aggiungi a `PolicyFlags` un campo
  `source_branch: Option<String>` che il **chiamante** (write.rs, task 05) risolverà via
  git e passerà qui. `check_policy` resta puro.
- Default retro-compatibili: con i nuovi campi **assenti** dal toml, il comportamento
  deve essere **identico a oggi** (nessuna nuova negazione).

## Nuovi campi `Policy` (additivi, con default)
1. `integration_branches: Vec<String>` — glob dei branch landing-zone di filone
   (es. `["integration/*"]`). **Default: vuoto** (`Vec::new()`), così un repo senza
   questo campo non attiva alcuna nuova guardia → retro-compatibile.
2. `develop_direct_commit: DevelopDirectCommit` — enum con `Deny | Confirm`, **default
   `Confirm`**. Serde: `#[serde(rename_all = "lowercase")]` → accetta `"deny"`/`"confirm"`.
   Semantica applicata SOLO quando il branch target è `develop` (vedi guardia sotto).
3. `develop_branch: String` — nome del branch confluente, **default `"develop"`** (non
   hard-codare la stringa nella logica; leggila dal campo, così è configurabile).
4. `work_branch_prefix: String` — prefisso dei branch di filone, **default `"work/"`**.

> Il matching glob di `integration_branches` dev'essere coerente con quello già usato per
> `scan_exclude` (vedi `crate::scan::path_matches_exclude` / helper glob esistente). Se
> esiste già un helper glob riusabile, **riusalo**; non introdurre una nuova dipendenza.

## Nuove guardie in `check_policy` (pure)
- **G1 — commit/merge diretto su develop.** Se `op == Commit` e `target_branch ==
  policy.develop_branch`: se `source_branch` è `Some(b)` e `b` **matcha** un
  `integration_branches` glob → **permesso** (è una sync di milestone legittima);
  altrimenti applica `develop_direct_commit`: `Confirm` → negato salvo `flags.confirm`;
  `Deny` → sempre negato (anche con confirm). Nuovo `DenialReason` dedicato
  (es. `DevelopDirectCommit`).
- **G2 — base dei work branch.** Se `op == CreateBranch` e il nome nuovo inizia con
  `work_branch_prefix`: deve nascere da un `integration/*` (o da `develop` **con**
  `confirm`), non da `main`. La "base" è `source_branch` (passata dal caller). Se
  `source_branch` è `None` → negazione fail-safe (nuovo `DenialReason`, es.
  `UnknownBase`), coerente con la filosofia `UnknownBranch` già presente. Se è un
  `integration/*` → permesso; se è `develop` → permesso solo con `confirm`; altrimenti
  (es. `main`) → negato.
- **Gate di retro-compatibilità (critico):** G1/G2 si attivano SOLO quando
  `integration_branches` è **non vuoto** (il repo ha dichiarato di usare la convenzione).
  Con lista vuota → comportamento identico a oggi. Codifica questo gate esplicito e **testalo**.

## Rules da rispettare
- Repo corrente: nessun `RULES.md` locale → stile dei file esistenti (`policy.rs`).
- Ecosistema: `nxm-docs/RULES.md` — niente `unwrap()`/`expect()` in produzione (ok nei
  test); errori strutturati con `thiserror`/enum (stile `DenialReason`/`PolicyDenied`),
  mai free-text; English per codice/doc; max ~300 righe per file (se `policy.rs` sfora,
  **è autorizzato** estrarre un modulo — es. `policy/workstream.rs` — mantenendo l'API
  pubblica; se devi splittare, fermati prima e segnalalo nella consegna).
- `CHANGELOG.md`: aggiungi una voce per i nuovi campi policy (rule hard: è autorizzato).

## Requisiti (ognuno con il suo test — regola invariante 7)
- R1 Parsing additivo + default: + **test** `policy_defaults_are_backcompat` — una `Policy`
  da toml **senza** i nuovi campi deserializza e si comporta come oggi; `Policy::default()`
  ha `integration_branches` vuoto, `develop_direct_commit == Confirm`, `develop_branch ==
  "develop"`, `work_branch_prefix == "work/"`.
- R2 `deny_unknown_fields` preservato: + **test** `unknown_field_still_errors` — un toml con
  una chiave ignota continua a fallire il parsing.
- R3 Enum parsing: + **test** `develop_direct_commit_parses_deny_and_confirm` — `"deny"` e
  `"confirm"` deserializzano nei rispettivi variant; un valore ignoto fallisce.
- R4 G1 confirm: + **test** `direct_commit_on_develop_without_confirm_denied` —
  `integration_branches=["integration/*"]`, `develop_direct_commit=Confirm`, `op=Commit`,
  `target_branch="develop"`, `source_branch=None/non-integration`, `confirm=false` →
  `Err(DevelopDirectCommit)`; con `confirm=true` → `Ok`.
- R5 G1 deny assoluto: + **test** `direct_commit_on_develop_deny_mode_blocks_even_with_confirm`
  — `develop_direct_commit=Deny`, `confirm=true`, source non-integration → `Err`.
- R6 G1 merge da integration: + **test** `merge_from_integration_into_develop_allowed` —
  `source_branch="integration/foo"`, `target_branch="develop"` → `Ok` anche senza confirm.
- R7 G2 base main: + **test** `work_branch_from_main_denied` — `op=CreateBranch`,
  nome `work/07-x`, `source_branch="main"` → `Err` (reason dedicato).
- R8 G2 base integration/develop: + **test** `work_branch_from_integration_allowed` (ok) e
  `work_branch_from_develop_needs_confirm` (develop senza confirm → Err, con confirm → Ok).
- R9 G2 base sconosciuta fail-safe: + **test** `work_branch_without_source_denied` —
  `source_branch=None` → `Err` (fail-safe).
- R10 Retro-compatibilità globale: + **test** `empty_integration_branches_disables_new_guards`
  — con `integration_branches` vuoto, un commit su `develop` senza confirm → `Ok` (come oggi),
  e un `work/*` da `main` → `Ok` (nessuna nuova guardia attiva).

## Branch di lavoro
- Parti da `develop` (aggiornato). Crea `feature/workstream-enforcement`, poi
  `work/04-workstream-policy-core`.
- NON committare su `main` né su `develop` direttamente. Lavora su `work/*`, merge no-ff
  `work→feature`. Il merge `feature→develop` lo fa il revisore/supervisore dopo approvazione.

## Tool disponibili (usa se necessario)
- `nxm-mcp-exec` — valida con `run_gate {repo_path}` (check→clippy→fmt_check→test);
  dichiara "fatto" solo su `verdict:"GO"`. Fallback a `cargo` se il server non è disponibile.
- `nxm-memory` — cerca pattern simili (helper glob, stile `DenialReason`) nel repo.
- `nxm-decision` — categorizza i bivi (es. riuso helper glob vs nuovo; nome dei reason).
- `nxm-session` — leggi lo stato di sessione se pertinente.

## Divieti
- NON toccare `write.rs`, `pr.rs`, `prepush.rs` né i tool MCP: la logica è pura in `policy.rs`.
  Il campo `source_branch` lo AGGIUNGI a `PolicyFlags` qui, ma la **risoluzione** via git è
  del task 05 — qui resta un `Option<String>` passato dai test.
- NON cambiare la semantica delle guardie esistenti (`ProtectedBranch`, force-push): i nuovi
  reason sono additivi.
- NON hard-codare `"develop"`/`"work/"` nella logica: leggili dai campi policy.
- NON aggiungere dipendenze esterne.
- MA le rules prevalgono: se il file sfora 300 righe o il CHANGELOG va aggiornato, fallo
  (autorizzato). In conflitto scope↔rule, FERMATI e chiedi.
- NON auto-approvarti: serve la revisione esterna.

## STOP e consegna alla revisione
- Completa SOLO questo micro-task, poi FERMATI.
- Mostra `git diff` (policy.rs + eventuale modulo estratto + CHANGELOG) e l'output REALE di
  `run_gate` (o `cargo test`/`clippy`).
- **Consegna a un revisore esterno** — riferimento skill `code-review`. Il branch si
  valida/mergia **solo dopo** review approvata.
- Se c'è ambiguità (nomi dei reason, dove splittare il modulo), FERMATI e chiedi.

## Verifica attesa
```bash
# preferito: tool tipati
# run_gate { repo_path: "~/Projects/private/nxm-git" } -> verdict GO
# fallback shell:
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt -- --check
```
I test preesistenti restano verdi; i 10+ nuovi test di `policy.rs` verdi; incolla l'output reale.
