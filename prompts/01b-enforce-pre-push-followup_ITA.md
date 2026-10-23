# Delega — 01b Follow-up: rules compliance per enforce require_pre_push

> **Stato:** PRONTO · **Creato:** 2026-10-04
> Companion: `01b-enforce-pre-push-followup_ENG.md` (tenere in sync).
> Segue: `01-enforce-require-pre-push_ITA.md`. Branch esistente: `work/01-enforce-pre-push` @ `cf846ca`.

## Perché questo follow-up
La review ha approvato la Spec ma trovato 2 violazioni HARD di `nxm-docs/RULES.md`, in tensione con
la regola "non uscire dallo scope" del brief 01. **Lo scope era troppo stretto: le rules vincono.**
Questo brief ESTENDE lo scope per renderlo conforme. Continua sullo stesso branch `work/01-enforce-pre-push`.

## Skill
**`tdd`** — mantieni verdi i test esistenti mentre splitti; aggiungi il test mancante (empty-string).

## Correzioni richieste (tutte sullo stesso branch work/01-enforce-pre-push)
1. **Limite 300 righe** (`nxm-docs/RULES.md`): oggi `write.rs`=311, `tests/write.rs`=327. Splitta per funzionalità:
   - Estrai la logica pre-push (variant `PrePushFailed`, runner dei comandi `require_pre_push`, suo
     `to_value`) in un modulo dedicato `nxm_git/src/prepush.rs`; `write.rs::push` lo richiama.
     Dichiara `mod prepush;` dove si dichiarano gli altri moduli. `write.rs` deve tornare < 300.
   - Sposta i 3 test pre-push da `tests/write.rs` a un nuovo `nxm_git/tests/prepush.rs`
     ("one file per module tested", RULES.md). `tests/write.rs` deve tornare < 300.
   - NON cambiare il comportamento: solo spostamento + visibilità (`pub(crate)` dove serve).
2. **CHANGELOG.md** (`nxm-docs/RULES.md`): aggiungi una voce (sezione corrente / Unreleased) tipo:
   "require_pre_push is now enforced as a hard gate in git_push (commands run after the forbidden
   scan, before push; a failing command blocks the push with a structured pre_push_failed error)."
3. **Test mancante** (Spec soft): aggiungi `empty_command_string_is_noop` (o estendi
   `empty_pre_push_is_noop`) per coprire un comando VUOTO/solo-whitespace nella lista
   (req 3 del brief 01: "comando vuoto o lista vuota").
4. **EXIT log sul fallimento** (Standards P3): sul ramo di fallimento pre-push, emetti il log EXIT
   coerente con lo stile `tracing` entry/exit già presente.
5. **Test robusto** (Spec soft): se un test usa un path temp con `touch <tempdir>`, evita rotture con
   TMPDIR contenente spazi (usa un nome file senza spazi dentro la temp dir, o passa l'argv array).

## Fuori scope (NON toccare)
- NON cambiare la semantica di `check_policy`/`gate`/altri write.
- Il double `Policy::load` e il `Vec<String>` split su whitespace sono scelte sancite dal brief 01:
  LASCIALI (non refactorare l'astrazione dei comandi in questo giro).
- Nessuna dipendenza nuova. Nessuna shell (`sh -c`): argv-array only.

## Rules da rispettare
- `nxm-docs/RULES.md`: max 300 righe/file; test in file per-modulo; CHANGELOG aggiornato; niente
  unwrap/expect in prod; logging `tracing` con target `nxm::git::...`; English nel codice.

## Branch di lavoro
- Continua su `work/01-enforce-pre-push` (NON ripartire da zero). NON committare su main/develop.
- Dopo le fix, il merge work→feature e feature→develop lo fa il revisore/supervisore dopo ri-review.

## STOP e consegna alla revisione
- Completa le 5 correzioni, poi FERMATI.
- Mostra `git diff develop..HEAD` (file nuovi inclusi: prepush.rs, tests/prepush.rs, CHANGELOG.md) e
  l'output REALE di verifica. Conferma il conteggio righe < 300 per write.rs e tests/write.rs.
- Consegna a revisore esterno (`code-review`). Merge solo dopo approvazione.

## Verifica attesa
```bash
wc -l nxm_git/src/write.rs nxm_git/tests/write.rs   # entrambi < 300
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
Tutti i test verdi (inclusi i pre-push spostati + il nuovo empty-string); clippy pulito; incolla l'output reale.
