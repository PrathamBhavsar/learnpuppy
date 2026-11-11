# Report Fixes Plan (R1 to R6)

Back to [Brain Index](/index.md). Source: the Antigravity direction and token audit of `staging` at `8794ab4`. Owner answers: G4 weighted gate yes, lazy textbook import yes, hide legacy classroom yes after R1 and R3 land, F36 code runner deferred.

**Status:** R1 to R6 built and merged to `staging` on 2026-11-12. `make ci`, `make e2e` (127 plus 17 serve) and the real course smoke pass. Open: textbook smoke with a real PDF, codex flags untested (login revoked), F08 and the parked items.

**R4 built** on branch `worktree-agent-a549359da41bf6c60`: upload maps only, a chapter imports when opened, per-chapter try plus one retry, 8 manual imports per book per rolling hour, 48-call job backstop. `pdftotext` was already windowed (10 pages per spawn), so the adapter is unchanged and a test pins it. See [Textbook Mode Backend Spec](/learning-method/textbook-spec.md) D7.

## Verified before planning

| Claim | Check | Result |
|---|---|---|
| `Confirmed` never reached | `next_step` skips facts at `McPassed` (`method/mod.rs` ~467), so `Recall` items are never served; `apply_outcome` needs a `Recall` pass to confirm | True. Close sees 0 confirmed |
| `run_verify_job` unused | only caller is its own test | True. Parked, see Not now |
| `distiller` leaks to default pin | in `ROLES`, missing from `PROFILE_ROLES` and `DEFAULT_PROFILES` | True. Falls to `claude:auto` |
| Textbook upload auto-imports to the 40 cap | `upload` makes zero map calls with an outline but starts a job that imports every pending chapter (`textbook.rs` ~376, `upload_maps_outline_with_zero_map_calls` expects 3 chapter calls) | True. R4's real change is dropping the auto start |
| `pdftotext` once per page | reviewer: `adapters.rs` already extracts a page window and the import reads 10 pages at a time | Likely stale. R4 drops this item unless a test shows per-page spawns |

## Packages

| Id | Package | Wave | Touches | Tokens |
|---|---|---|---|---|
| R1 | Fix `Confirmed` state machine, gate Feynman on `correct` | 1 | `lp-core/src/method/*` | 0 |
| R2 | `distiller` profile; strip CLI subprocess bloat only where flags are verified in `--help` | 1 | `lp-service/src/settings.rs`, `lp-cli-driver/src/engine/subprocess.rs` | saves |
| R3 | FSRS `FlashCard` keeps free recall; textbook recall graded by `must_name` rubric | 1 | `lp-core/src/ports`, `lp-service/src/course`, `lp-service/src/textbook.rs` grading, `web/src/features/review/*` | 0 |
| R4 | Textbook lazy import: upload maps only, chapter imported on first open; cap per chapter not per book; `pdftotext` per range; leaf chapters under `Part` | 2 after R3 | `lp-service/src/textbook.rs`, `lp-core/src/textbook.rs`, `lp-agent` pdf adapter, textbook web | saves 350k to 450k per book |
| R5 | G4 weighted quota gate in `bench/cheap_tutor` (Opus 5x Sonnet, 15x Haiku or Flash) | 1 | `bench/cheap_tutor` | 0 |
| R6 | Hide legacy `/sessions/:id` classroom behind an advanced toggle | 3 after R1 and R3 | `web/src/features/*` nav and route | 0 |

R3 and R4 both edit `lp-service/src/textbook.rs`, so R4 runs after R3 merges. R1, R2, R3, R5 touch disjoint files.

## Rules

1. Domain logic stays in Rust. Zero model calls added while studying.
2. Each package ships its tests in the same commit and must fail without the change.
3. The 40 cap: the lifetime-per-book rule (D7) becomes a per-chapter retry cap plus a per-hour rate limit of auto imports. Cap logic stays counted from the ledger `Usage` lines.
4. R2 may only add a flag after running `<cli> --help` here and seeing it. No guessing flags.
5. Worktree per package under `../learnpuppy-wt-<id>`, branch `pratham404/<id>`, `make gate` before commit, `git merge --no-ff` into `staging`, then delete the worktree and branch. At most 4 workers.
6. No push, no `prod`.

## Not now

`run_verify_job` wiring (F08), quick lookup mode, interactive rungs, math delimiter recovery, owned-vs-borrowed graph, batching card generation, extractor debounce, prompt-cache reorder, F36 code runner. Each needs its own plan after R1 to R6.

## Done means

`make ci` and `make e2e` pass on `staging`; a `method_version = 1` walk reaches `Confirmed` and Close writes nodes; a 500-page fixture uploads with at most 1 map call and zero chapter calls.

## Round 1 changes (from the Goldfish review)

1. **R1 compatibility.** Folding is pure over stored events, so old courses replay under the new rule. A fact at `McPassed` with no `Recall` item authored counts as passed (nothing to confirm). Otherwise it must reach `Confirmed`. The Feynman gate allows at most 2 tries, then advances and records the miss. Rewrite the tests at `mod.rs` ~700, ~997, ~1038 and the walk ~1217 that assume `McPassed` is enough. Add a walk that reaches `Confirmed` for every fact and a Close that writes nodes.
2. **R2 distiller.** Which wins, pin or profile, for a user who pinned `distiller`? The pin wins (existing `settings.pin("distiller")` use in `app.rs`). Update tests that count `PROFILE_ROLES`, the generated TS types and snapshots. CLI flags: argv snapshot test of `turn_args` per CLI, and a recorded `--help` check; report a measured token number or write "unmeasured".
3. **R3 paths.** Builder finds where `FlashCard` lives and lists exact paths. Existing FSRS card data: old cards keep rendering as MCQ, only new and graduated recall cards use free recall; no data migration. R3's web files are `web/src/features/review/*` only; R4's are the textbook web files; they must not overlap.
4. **R4 contract.** Dropping the auto start changes `Upload.job`: DTO, OpenAPI, `schema.d.ts`, the web upload flow, MSW and tests all change. Open chapter route imports that one chapter. The 40 lifetime cap becomes: per chapter, one try plus one retry; per book, 8 manual chapter imports per rolling hour, using the server clock; `usage()` and its `cap` field change shape, and the D7 tests (`textbook.rs` ~1848-1906) are rewritten. Test with a fake `Pdf` fixture, not real `pdftotext`.
5. **R6 deep links.** `/sessions/:id` keeps working by direct URL. Only the sidebar entry and palette entry hide behind the advanced toggle. Existing e2e specs that use the nav entry are updated; snapshots refreshed.
