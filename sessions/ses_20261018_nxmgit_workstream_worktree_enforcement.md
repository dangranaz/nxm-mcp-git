---
schema: nxm-session-handoff/v1
id: ses_20261018_nxmgit_workstream_worktree_enforcement
title: "nxm-git — enforce the parallel-workstream worktree convention (integration branches + worktree guards)"
short_description: "Spec per rendere ENFORCED in nxm-git la convenzione worktree-per-filone oggi solo documentale in WORKFLOW.md."
created: "2026-10-18T14:44:00Z"
updated_at: "2026-10-18T14:44:00Z"
revision: 1
workspace: "/Users/devdaniele/Projects/private/nxm-git"
status: unconsumed
trigger: "manual"
---

# Handoff / Spec — nxm-git: enforce the parallel-workstream worktree convention

## Perché questa session esiste (origine)
2026-10-18, durante il Filone 3 di `engine-metal` (Memory Governor + Sampling GPU):
lavorare più filoni in parallelo sullo **stesso working tree** si è rivelato fragile.
Il checkout principale cambiava branch sotto i piedi e le modifiche non committate di
un filone finivano **auto-stashate** quando un altro agente cambiava branch — lavoro
quasi perso (recuperato solo da `stash@{0}`). Decisione dell'owner: ogni filone deve
vivere in un **worktree stabile dedicato** con un **integration branch** proprio, e
sincronizzare su `develop` **a milestone** (non per task).

La convenzione è già stata scritta come **soft/documentary** in
`nxm-git/WORKFLOW.md` → sezione *"Parallel workstreams in isolated worktrees"*.
Questa session è per il passo successivo: **renderla ENFORCED** dal policy engine.

## Goal (cosa implementare)
Estendere `nxm-git` perché **applichi** (non solo documenti) la convenzione:
1. riconoscere gli **integration branch** (`integration/<workstream>`) come landing
   zone di filone;
2. **impedire che un filone committi/mergi direttamente su `develop`** fuori dai
   punti di milestone (develop resta il confluente di tutti i filoni, ma non riceve
   commit diretti dai `work/*` di un filone);
3. opzionale ma utile: **associare un worktree a un workstream** e segnalare quando si
   opera sul working tree sbagliato.

## Stato attuale di nxm-git (as-built, da cui partire)
- Policy engine: `nxm_git/src/policy.rs` (guardie) + `nxm_git/src/config.rs` (lettura
  `.nxm-git-policy.toml`). Campi oggi: `protected_branches`, `allow_force`,
  `destructive_confirm`, `forbidden_patterns`, `scan_exclude`, `require_pre_push`.
- Write tools policy-gated: `git_commit`, `git_create_branch`, `git_switch`,
  `git_push`, `git_pr_create` (vedi `nxm_git/src/write.rs`, `pr.rs`, `prepush.rs`).
- `.nxm-git-policy.toml` + `.nxm-git-policy.toml.example`: schema dichiarativo.
- `WORKFLOW.md`: ora contiene la convenzione soft (hierarchy aggiornata + tabella
  merge-style con `integration/<workstream>`).
- **Non esiste** oggi alcun concetto di worktree/integration-branch nel policy engine
  (verificato: nessun match `worktree|integration` in `src/`).

## Specifiche proposte (da raffinare in implementazione)
### A. Nuovo schema policy (`.nxm-git-policy.toml` + `config.rs`)
- `integration_branches` (lista di glob, es. `["integration/*"]`): branch riconosciuti
  come landing zone di filone.
- `develop_direct_commit = "deny" | "confirm"` (default `confirm`): commit/merge diretti
  su `develop` che NON provengono da un `integration/*` richiedono `confirm=true`
  (milestone esplicita) invece di passare silenziosi. Così la sync a milestone diventa
  un gesto deliberato, non accidentale.
- (opz.) `[workstreams]` map `name -> worktree_path`: associa un filone al suo worktree.

### B. Guardie nel policy engine (`policy.rs`)
- `git_commit`/merge su `develop`: se l'operazione non è un merge da un
  `integration/*` branch → applica `develop_direct_commit` (deny o confirm).
- `git_create_branch`: un `work/<nn>-<slug>` dovrebbe nascere da un `integration/*`
  (o da `develop` solo con confirm), non da `main`.
- (opz.) `git_switch`/`git_commit`: se il repo è un worktree mappato a un workstream e
  il branch target appartiene a un ALTRO workstream → warn/deny (evita il cross-talk che
  ha causato l'auto-stash).

### C. Documentazione
- Aggiornare `.nxm-git-policy.toml.example` con i nuovi campi commentati.
- `WORKFLOW.md`: togliere il marcatore "soft convention / not yet enforced" dalla sezione
  worktree quando l'enforcement è attivo; linkare i campi policy.

## Criteri di accettazione (test)
- `config.rs`: parsing dei nuovi campi + default (test su toml di esempio).
- `policy.rs`: test che un commit diretto su `develop` senza confirm e non da
  `integration/*` è rifiutato; che un merge `integration/* -> develop` con confirm passa;
  che `work/*` da `main` è bloccato.
- (opz.) test della mappa workstream->worktree: operazione su branch di altro workstream
  nel worktree sbagliato → warn/deny.
- `cargo test --workspace` verde (è anche il `require_pre_push` del repo).

## Approcci / vincoli
- Seguire il ciclo nxm-git stesso (WORKFLOW.md): `feature/*` → `work/<nn>` → review →
  develop `--no-ff`. Non implementare direttamente su develop/main.
- Non rompere i tool/policy esistenti: i nuovi campi sono **additivi** con default
  retro-compatibili (assenti ⇒ comportamento di oggi).
- Rules del repo: max 300 righe/file, test in `tests/`, CHANGELOG, niente unwrap/expect.

## File rilevanti (ordine di lettura)
- `nxm-git/WORKFLOW.md` (convenzione soft già scritta — la base da enforce)
- `nxm-git/.nxm-git-policy.toml` + `.nxm-git-policy.toml.example` (schema)
- `nxm-git/nxm_git/src/config.rs` (parsing policy)
- `nxm-git/nxm_git/src/policy.rs` (guardie)
- `nxm-git/nxm_git/src/write.rs`, `pr.rs`, `prepush.rs` (punti d'uso delle guardie)

## Prossimi passi (todo)
1. [ ] Design di dettaglio dei campi policy (A) + default retro-compatibili.
2. [ ] Implementare parsing in `config.rs` + test.
3. [ ] Implementare guardie in `policy.rs` + test.
4. [ ] Aggiornare `.example` e `WORKFLOW.md` (rimuovere il marcatore "soft").
5. [ ] feature → develop `--no-ff`; tag se release.

> NB: intervento "soft" (doc in WORKFLOW.md) già fatto 2026-10-18. Questa session è
> l'intervento "più valido" (enforcement) che l'owner implementerà quando vorrà.
