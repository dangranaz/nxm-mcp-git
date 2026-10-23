---
schema: nxm-session-handoff/v1
id: ses_20261021_nxmgit_create_integration_and_worktree_tools
title: "nxm-git — due tool di CREAZIONE: integration branch da base esplicita + worktree di lavoro da quell'integration branch (fix del bug 'branch dal checkout corrente'); far avanzare la versione di nxm-git"
short_description: "Aggiungere a nxm-git i tool mancanti per creare l'integration branch di una feature da una base esplicita (default develop) e, DA QUELLO, il worktree/branch di lavoro. Risolve il bug di git_create_branch (ramifica da HEAD corrente). Bump versione nxm-git."
created: "2026-10-21T14:10:00Z"
updated_at: "2026-10-21T14:10:00Z"
revision: 1
workspace: "/Users/devdaniele/Projects/private/nxm-git"
status: consumed
trigger: "manual"
consumed_by: null
consumed_at: "2026-10-21T12:46:20Z"
---

# Handoff / Spec — nxm-git: tool di creazione integration-branch + worktree

## Perché questa session esiste (origine)
2026-10-21, mentre si lavorava ai micro-interventi GEMV Q4 di `engine-metal`:
dopo aver reso `develop` valido (merge G4b + reconcile MoE), l'owner ha chiesto di
creare un NUOVO integration branch da `develop` per i prossimi ticket. Usando i tool
attuali di `nxm-git` è emerso un **bug concreto**: `git_create_branch` (→
`nxm_git/src/write.rs::create_branch`) ramifica SEMPRE dal **HEAD del checkout
corrente** (commento nel codice: *"G2: the new work branch is based on the current
HEAD branch"*; esegue `git branch <name>` senza start-point). Il main checkout era su
`work/32-qkv-fused`, quindi il nuovo `integration/*` è nato dal punto SBAGLIATO
(`1c951b1c`) invece che da `develop` (`90ecd203`). Rilevato e corretto a mano
(`git branch -D` + `git branch <name> develop` + `git worktree add` via shell).

Inoltre: **non esiste alcun tool worktree** in `nxm-git` (verificato: nessun
`worktree` in `nxm_git/src/`). Creare il worktree isolato del filone — passo
OBBLIGATORIO della convenzione in `WORKFLOW.md` §"Parallel workstreams in isolated
worktrees" — oggi si fa solo via `shell`, fuori da ogni policy-gate.

Questa session copre il pezzo di CREAZIONE che la spec gemella
`ses_20261018_nxmgit_workstream_worktree_enforcement` (enforcement di policy) non
tocca. Le due sono complementari: quella impedisce i commit diretti su `develop`;
questa fornisce i due tool che materializzano correttamente la gerarchia.

## Goal (cosa implementare) — i tre requisiti dell'owner
1. **Un tool per creare l'INTEGRATION branch della feature da una base ESPLICITA.**
   Deve ramificare da `develop` (o da una base passata esplicitamente), MAI dal HEAD
   del checkout corrente. Questo è il fix diretto del bug.
2. **Un tool per creare il branch/worktree di LAVORO partendo da QUELL'integration
   branch.** Prima l'integration branch (da develop), POI da quello il worktree
   isolato del filone. Ordine forzato e meccanico, così la convenzione del WORKFLOW
   non dipende più da gesti shell a mano.
3. **La session deve FAR AVANZARE la versione di nxm-git.** Bump SemVer esplicito
   (vedi §Versioning) + tag per-crate sulla promozione, come da `WORKFLOW.md`.

## Stato attuale di nxm-git (as-built, da cui partire)
- Versione: `0.1.0` (workspace-wide, `Cargo.toml` `[workspace.package].version`).
- Write tools policy-gated in `nxm_git/src/write.rs`: `commit`, `create_branch`,
  `switch_branch`, `push` (+ `pr.rs`, `prepush.rs`). Tutti passano da `gate()` →
  `check_policy` PRIMA di eseguire git; git è sempre shell-out con argv-array.
- **BUG**: `create_branch` esegue `run_git(repo, &["branch", name])` — NESSUNO
  start-point → ramifica da HEAD corrente. `source_branch` passato al gate è
  `current_branch(repo)`, non la base reale del nuovo branch.
- **Assente**: qualunque supporto `git worktree`.
- Policy engine: `nxm_git/src/policy.rs` + `config.rs`. Campi rilevanti già previsti
  dalla spec gemella: `develop_branch` (default `"develop"`), `integration_branches`,
  `work_branch_prefix` (default `"work/"`), `develop_direct_commit`.
- MCP server: `nxm_git_mcp/src/server.rs` (registrazione tool) + `inputs.rs` (schemi
  input). I due nuovi tool vanno esposti qui.
- Convenzione autorevole: `WORKFLOW.md` §"Parallel workstreams in isolated worktrees"
  — `integration/<workstream>` da `develop`; worktree stabile `.worktrees/<workstream>`.

## Specifiche proposte (da raffinare in implementazione)

### A. Tool 1 — integration branch da base esplicita
Due forme possibili (decidere in design; preferenza: fare ENTRAMBE, la 2 sopra la 1):
1. **Fix additivo di `create_branch`**: nuovo parametro opzionale
   `base: Option<String>` (alias `start_point`). Se presente → `git branch <name>
   <base>`; se assente → default alla policy `develop_branch` (NON più HEAD corrente).
   Il `source_branch` passato al gate diventa la BASE reale, non `current_branch`.
   Retro-compatibilità: il vecchio comportamento "da HEAD" resta raggiungibile solo
   passando esplicitamente `base = <current>` — l'implicito cambia a `develop`.
2. **Tool dedicato `git_create_integration_branch { workstream, base? }`**: costruisce
   il nome `integration/<workstream>`, ramifica da `base` (default `develop_branch`),
   valida il nome contro `integration_branches`. Ergonomico e a prova di typo.

### B. Tool 2 — worktree di lavoro DA quell'integration branch
**`git_worktree_add { workstream, branch?, path? }`** (naming da decidere):
- Default: `path = .worktrees/<workstream>`, `branch = integration/<workstream>`.
- Esegue `git worktree add <path> <branch>` (argv-array, mai stringa interpolata).
- PRECONDIZIONI (rifiuto esplicito e tipizzato se non rispettate):
  - l'integration branch DEVE già esistere (ordine forzato: prima Tool 1, poi Tool 2);
  - il path del worktree NON deve già esistere;
  - `<path>` deve stare sotto `.worktrees/` (niente worktree sparsi).
- Policy-gated come gli altri write (carica la policy, fallisce su policy malformata).
- (opz.) un `git_worktree_remove`/`list` come corollario, se utile al TUI.

### C. Composizione (il punto centrale della richiesta)
I due tool si usano IN SEQUENZA e rendono meccanica la gerarchia del WORKFLOW:
```
git_create_integration_branch { workstream }   # integration/<ws> DA develop
        ↓
git_worktree_add { workstream }                 # .worktrees/<ws> DA integration/<ws>
        ↓
(poi, per ogni micro-task) work/<nn>-<slug> da integration/<ws>  # già coperto da Tool 1 forma 1 con base
```
Risultato: il bug "ramifica dal checkout corrente" non può più ripetersi, e il
worktree isolato nasce sempre dalla base giusta.

### D. Versioning (requisito 3 — NON opzionale)
- Due tool nuovi = capacità retro-compatibile ⇒ **minor bump: `0.1.0 → 0.2.0`**
  (`[workspace.package].version` in `Cargo.toml`; oggi il bump è workspace-wide).
- Tag per-crate sulla promozione su `main`, come da `WORKFLOW.md` §Releases:
  es. `nxm-git-v0.2.0` / `nxm-git-mcp-v0.2.0` sul commit di promozione.
- Aggiornare `CHANGELOG.md` con la voce dei due tool + il fix del bug.

## Criteri di accettazione (test)
- `write.rs`: `create_branch` con `base` esplicito ramifica da quella base (test: crea
  da un branch X mentre HEAD è su Y → il nuovo branch punta a X, non a Y). Senza `base`
  ramifica da `develop_branch`, NON da HEAD.
- `git_create_integration_branch`: produce `integration/<workstream>` da `develop`;
  nome validato; rifiuta duplicati.
- `git_worktree_add`: crea `.worktrees/<ws>` da `integration/<ws>`; rifiuta se
  l'integration branch non esiste, se il path esiste già, o se fuori da `.worktrees/`.
- MCP: i due tool esposti in `server.rs` con schema input in `inputs.rs`; test in
  `nxm_git_mcp/tests/http_tool.rs`.
- `Cargo.toml` versione = `0.2.0`; `CHANGELOG.md` aggiornato.
- `cargo test --workspace` verde (è anche il `require_pre_push` del repo).

## Approcci / vincoli (ciclo nxm-git stesso)
- Seguire `WORKFLOW.md`: `feature/<name>` da `develop` → `work/<nn>-<slug>` → review +
  re-run build/test/lint → merge `--no-ff` in `develop`. NON implementare su
  develop/main direttamente.
- Nuovi campi/param ADDITIVI con default retro-compatibili (assenti ⇒ comportamento
  odierno) TRANNE il default implicito di `create_branch`, che cambia da "HEAD" a
  "develop" — è il fix voluto; documentarlo nel CHANGELOG come change di comportamento.
- Rules repo: max 300 righe/file, test in `tests/`, niente `unwrap`/`expect`, git solo
  argv-array (mai stringhe interpolate).

## File rilevanti (ordine di lettura)
- `nxm-git/WORKFLOW.md` (convenzione: integration branch da develop + worktree isolato)
- `nxm-git/sessions/ses_20261018_nxmgit_workstream_worktree_enforcement.md` (spec gemella: enforcement policy)
- `nxm-git/nxm_git/src/write.rs` (il bug in `create_branch`; dove innestare Tool 1/2)
- `nxm-git/nxm_git/src/policy.rs` + `config.rs` (`develop_branch`, `integration_branches`)
- `nxm-git/nxm_git/src/argv.rs` (`ensure_ref_arg` per validare i ref)
- `nxm-git/nxm_git_mcp/src/server.rs` + `inputs.rs` (esposizione MCP dei nuovi tool)
- `nxm-git/Cargo.toml` (bump versione) + `CHANGELOG.md`

## Prossimi passi (todo)
1. [ ] Design di dettaglio: forma dei due tool + default retro-compatibili + naming.
2. [ ] `feature/create-integration-and-worktree-tools` da `develop`.
3. [ ] Tool 1: `base` su `create_branch` (+ eventuale `git_create_integration_branch`) + test.
4. [ ] Tool 2: `git_worktree_add` + precondizioni/rifiuti tipizzati + test.
5. [ ] Esporre entrambi in MCP (`server.rs` + `inputs.rs`) + test `http_tool.rs`.
6. [ ] Bump versione `0.2.0` + CHANGELOG + aggiornare `WORKFLOW.md` se serve.
7. [ ] review + `cargo test --workspace` + merge `feature → develop --no-ff`; tag sulla promozione.

> NB: complementare a `ses_20261018` (enforcement). Questa session è il lato CREAZIONE
> (i due tool mancanti) + il fix del bug + il bump di versione.
