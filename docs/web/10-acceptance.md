# 10 — Acceptance: "Clean UI, every feature working"

Release = every box ticked, with evidence (test name or screenshot path) filled by A70. State honestly which checks ran against the real engine vs `FauxEngine`.

> **Engine caveat (2026-11-01):** every check below ran against `FauxEngine` or MSW mocks, except the capped real-engine run at the end of this file.

## A. Boot & safety
- [x] `learnpuppy serve` prints URL, opens browser, SPA loads with no console errors/warnings.
  - Evidence: Manual 2026-11-01: serve prints the launch JSON + URL; `journeys.spec.ts` "deep links cold-load every top-level screen without errors" (mock + serve). Browser auto-open itself is not automated.
- [x] Opening the bare URL without token/cookie shows the "open from terminal" screen; API 401.
  - Evidence: `shell.test.tsx` "shows the full-screen auth state when the session is rejected"; `auth.serve.spec.ts` "health is public; other API routes need the session cookie" (401 `auth_required`).
- [x] Wrong `Host`/cross-origin POST/missing CSRF header rejected (tests).
  - Evidence: `auth.serve.spec.ts` "wrong Host is refused with 421", "mutations need our own Origin and X-LP-CSRF".
- [x] Second `serve` reuses running instance. Ctrl-C shuts down cleanly; running jobs resume on next boot.
  - Evidence: Manual 2026-11-01 (faux engine): 2nd `serve` printed `existing:true` + same URL, exit 0; SIGINT stopped the server and removed the lock. Rust: `serve.rs` `stale_lock_is_replaced_and_removed_on_drop`. Job resume on boot: `main.rs` prints `resumed:`; UI: `classroom.test.tsx` "offers Resume after a restart interrupted the turn".
- [x] Migration of a copy of the real `~/.learnpuppy` preserves node/chain/chat/card counts; revert works.
  - Evidence: Manual 2026-11-01 on a copy of the real `~/.learnpuppy` (`LEARNPUPPY_HOME`): `migrate` moved 5 entries into a workspace, `doctor` ok, `migrate --revert` restored the tree (`diff -rq` shows only an added `state.json`). Entries are moved, not rewritten, so counts are preserved by construction; counts not diffed. Rust: `directory.rs` `migration_runs_once_and_leaves_a_banner`.

## B. Workspaces
- [x] Create / edit / archive / restore / delete(to trash) / restore from trash / purge / export / import all work.
  - Evidence: `workspaces.spec.ts` "create, rename, archive, delete and restore a workspace"; `workspaces.test.tsx` (archive/restore, trash list + purge, export progress -> Download, import validation).
- [ ] Switch via chip, ⌘1–9, palette; section preserved; two tabs on two workspaces independent.
  - Evidence: Chip switch keeping the section: `shell.test.tsx` "switches workspace from the chip popover, keeping the current section". NOT covered: ⌘1–9 and two tabs on two workspaces.
- [x] Onboarding from zero state; damaged workspace repair; not-found page.
  - Evidence: `workspaces.test.tsx` "empty: first run shows onboarding", "creates the first workspace, offers detected folders and starts ingest", "degraded: damaged workspace offers Repair"; `shell.test.tsx` "shows a not-found state for an unknown workspace id"; Rust `repair_fixes_only_damaged_workspaces`.
- [x] Ingest source detection, overlap warning, empty allow-list disabled.
  - Evidence: `ingest.test.tsx` "warns when a folder overlaps another workspace", "empty allow-list: explains itself and keeps Run disabled"; `ingest-accounts-settings.spec.ts` "an empty allow-list explains itself and disables Run".

## C. Graph
- [x] 800 and 5k node fixtures render at budget; hulls/labels LOD; minimap; fit/zoom/pan inertia.
  - Evidence: `graph.spec.ts` "performance budgets" + "12k nodes switches on performance mode"; `perf/budgets.perf.spec.ts`; `graph-screenshots.spec.ts` "5k nodes"; hull LOD `camera.test.ts`; minimap/toggles `graph.spec.ts`. Mock fixtures.
- [x] Search, filters, hover-dim, select, multi-select, marquee, keyboard navigation, GraphList mirrors.
  - Evidence: `graph.spec.ts` hover/select/Esc, "keyboard: arrows walk to a neighbour", "shift-drag draws a marquee", "the accessible list mirrors selection"; `selection.test.ts`; `GraphScreen.test.tsx`.
- [x] NodeDrawer with provenance/chains/cards; Open source chat works.
  - Evidence: `graph.spec.ts` drawer open/Enter; `graph-screenshots.spec.ts` "drawer" (light/dark × LTR/RTL); `GraphScreen.test.tsx` `?node=` cold load; Rust `node_detail_search_and_chain_table`. Mock + Rust API.
- [x] Chain table 3-level, sort, virtualization, CSV export.
  - Evidence: `graph.spec.ts` "chain table opens, sorts, scopes to the selection and exports CSV"; `chains.test.ts` (3-level reveal, CSV).
- [ ] Practice-from-selection creates and opens a Practice chat.
  - Evidence: Practice chat from the New chat dialog is covered (`chat-list.test.tsx` "picks graph nodes and starts a practice chat"); the graph-selection entry point has no dedicated test.
- [x] Delete preview (blast radius) → confirm → animated removal; token-stale rejection.
  - Evidence: `graph.spec.ts` "Delete key previews the blast radius, needs the acknowledgement, then removes the node"; `GraphScreen.test.tsx` cancel keeps graph; Rust `graph_changes_bump_the_version_and_drop_the_stale_layout`; animations `diff.test.ts`/`tween.test.ts`.
- [x] Relayout animation; new-node birth animation after merge; empty/pending/error states.
  - Evidence: `diff.test.ts` (moved/new nodes on layout version bump), `tween.test.ts`; Rust `graph_snapshot_carries_positions...`; error state `GraphScreen.test.tsx` "retryable error".

## D. Learn
- [x] Full Faux session: Probe (≤8) → Plan DAG appears → Teach; Practice session with problem + gate.
  - Evidence: FAUX: Rust `full_probe_plan_teach_over_http_and_sse` (`chats_write.rs`), `probe_handoff_plan_teach_lifecycle`; `learn.serve.spec.ts` "a practice chat shows its problem".
- [x] All six question tools render and submit; quiz reveal animations; confidence slider.
  - Evidence: `dock.test.tsx` (pick, multi-select, a–d/1–4 keys, confidence slider, hint ladder); `learn.spec.ts`.
- [ ] Streaming text smooth; stick-to-bottom + Jump to latest; thinking block; stop/cancel keeps partial.
  - Evidence: `classroom.test.tsx` "streams the reply from chat.delta events", "Jump to latest". Thinking block and stop-keeps-partial have no dedicated assertion.
- [x] Rate-limit queue banner with countdown; engine error with Retry; restart-mid-turn Resume.
  - Evidence: `learn.spec.ts` "rate-limit banner counts down; engine errors offer Retry"; `classroom.test.tsx` "counts down while queued behind a rate limit and cancels", "offers Resume"; `learn.serve.spec.ts` Retry.
- [x] Workbench jobs run/abort/deliver; artifact viewer safe (sandbox); PDF upload.
  - Evidence: `plan.test.tsx` (jobs list, abort, iframe sandbox, artifact error+retry); `learn.spec.ts` "jobs open their artifacts, HTML inside an inert iframe"; `classroom.test.tsx` "uploads a PDF"; Rust `pdf_upload_job_lifecycle_abort_and_artifact_sandbox`.
- [x] Chat rename/export/delete; stale-question 409 recovers silently.
  - Evidence: `learn.serve.spec.ts` "rename, export and delete use the real endpoints", "a question answered elsewhere resyncs silently"; `classroom.test.tsx` "resyncs silently on a stale question".
- [x] Markdown: code (shiki), tables, math, links safe.
  - Evidence: `markdown.test.tsx` (code blocks, shiki highlight, math, tables, link safety).

## E. Review & Digest
- [ ] Keyboard-only review (1–4 / space / esc) with previewed intervals; optimistic advance < 200 ms; rollback on error.
  - Evidence: `session-keys.test.ts`, `use-session.test.ts` (reveal, rollback, stale answers, server preview intervals). The < 200 ms optimistic advance is not timed anywhere.
- [x] Stats, forecast, streak, session summary, card browser with history.
  - Evidence: `review/api.test.tsx`, `use-session.test.ts` (summary), `src/features/review/*` forecast/streak; Rust review routes in `api.rs`.
- [x] `/review-all` across workspaces.
  - Evidence: `ReviewAllScreen.test.tsx` (list, nothing-due, error, keeps loaded on refresh failure).
- [x] Digest ranges/filters/search; decision drawer with alternatives & commits; regenerate cards; "Review these".
  - Evidence: `DigestScreen.test.tsx` (range, search, drawer, regenerate + toast, disabled while running); Rust `decision_detail_lists_cards_and_session_and_regenerates_cards`.

## F. Ingest, Accounts, Settings
- [x] Ingest run with live progress, cancel, history, error reporting.
  - Evidence: `ingest-accounts-settings.spec.ts` "save sources, run with live progress, then read the finished report", "cancel stops a running ingest"; `ingest.test.tsx` (progress, history, summary).
- [x] Accounts list with cooldown ring; add Claude profile; rescan; disable; role-slot edit + diff + test ping.
  - Evidence: `ingest-accounts-settings.spec.ts` "cooldown ring counts down, disable works, add profile validates", "role slots: edit, diff preview, save and test ping"; Rust `settings_test_pings_a_profile_without_erroring_on_failure`.
- [x] Theme (light/dark/system), direction (LTR/RTL), reduce-motion override, density, concurrency cap persist.
  - Evidence: `ingest-accounts-settings.spec.ts` "theme, direction, motion and density apply at once and survive a reload", "concurrency cap and diagnostics"; `theme.test.ts`; `settings.test.tsx`.
- [x] Diagnostics copy contains no secrets (test with planted token).
  - Evidence: `settings.test.tsx` "never copies a planted token"; `diagnostics.test.ts`.

## G. Shell & cross-cutting
- [ ] Palette: navigate, switch workspace, actions, node search (workspace + all), recents.
  - Evidence: `shell.spec.ts` "⌘K palette: type, arrow, enter navigates"; `smoke.spec.ts`; `shell.test.tsx`. Node search across all workspaces and recents not asserted.
- [x] Shortcuts sheet matches actual bindings (generated from registry).
  - Evidence: `shell.spec.ts` "g chords and ? shortcuts sheet"; `settings.test.tsx` "lists the registered shortcuts from the hotkey registry".
- [x] Connection banner on SSE drop; resync after reconnect.
  - Evidence: `shell.spec.ts` "connection banner appears after 2 s of SSE downtime and clears on recovery"; `offline.spec.ts`; `offline.serve.spec.ts` (real server).
- [ ] Every screen has loading/empty/error/degraded/populated states with assertions.
  - Evidence: Per-screen state tests exist (workspaces, settings, graph, digest, ingest, review-all) but there is no matrix proving all five states on every screen.
- [ ] Title/focus/announce on route change; scroll restoration.
  - Evidence: `routes.test.tsx` document title; `shell.test.tsx` "moves focus to the page heading and announces the title". Scroll restoration has no test.

## H. Design, motion, a11y
- [x] Palette = preset structure + torlink colors only (diff script); no hard-coded colors outside tokens.
  - Evidence: `web/scripts/palette.test.ts`, `bun run tokens:check`, eslint `no-color-literal` (all in `bun run check`).
- [x] Light & dark contrast script passes; no flash on load.
  - Evidence: `bun run contrast` (in `bun run check`); `theme.test.ts` "applies the stored theme and direction before React mounts" (no flash).
- [x] RTL: shell + each screen mirrored correctly; canvas LTR; mixed-direction message OK.
  - Evidence: `smoke.spec.ts` ?dir=rtl, `graph.spec.ts` RTL drawer, `learn.spec.ts` RTL plan panel, `visual.spec.ts` screens × light/dark × LTR/RTL; eslint `no-physical-direction`.
- [x] All 45 animations in 05 implemented (checklist table below); reduced-motion variant verified in vitest (`reduced-motion.test.ts`), Playwright `getAnimations()` check pending.
- [ ] axe: zero serious/critical on all screens/states; keyboard-only walkthrough passes; focus visible everywhere.
  - Evidence: axe specs: shell/gallery, graph+drawer, classroom, ingest/accounts/settings; `keyboard.spec.ts` walkthrough. Review, digest and workspaces screens have no axe spec yet.
- [x] Perf budgets in 07 pass; 30-min soak has no leak.
  - Evidence: `perf/budgets.perf.spec.ts`: LCP 428 ms, no long task > 100 ms, graph frame times in budget. Soak (`LP_SOAK_MIN=30`, 2026-11-01, mock build): 12,411 navigations + SSE drops, heap 37.3 MB (min 5) -> 42.9 MB (min 30), budget < 30 MB growth. Slight upward drift of ~0.2 MB/min is worth re-checking if it grows; real-server soak not run.
- [x] Visual baselines committed for light/dark × LTR/RTL; 3 consecutive green `make e2e` runs.
  - Evidence: `visual.spec.ts` + `graph-screenshots.spec.ts` baselines; 2026-11-01 three consecutive `make e2e` runs, each 90 mock + 16 serve passed, 0 flaky. Fixes that got there: `post_turn` returns `stale_question` (not `turn_in_progress`) for an answer posted mid-turn; Playwright `retries: 1` for cold vite chunk-fetch failures.
- [x] `make gate` and `make e2e` green; `openapi.json` drift-free; `cargo deny` green.
  - Evidence: 2026-11-01: `make gate` exit 0 (includes openapi drift via `api-check`), `make e2e` exit 0, `make deny` passed earlier (A70).
- [x] No `unwrap()` in routes; no physical-direction classes; no external network requests (Playwright asserts zero non-localhost requests).
  - Evidence: `cargo clippy -p lp-server --lib --bins -- -D clippy::unwrap_used` clean; eslint `no-physical-direction`; `web/e2e/serve/localhost-only.serve.spec.ts` asserts every request goes to 127.0.0.1.
- [x] `grep` audit: no FSRS/graph/plan logic in `web/src`.
  - Evidence: 2026-11-01: grep for fsrs/schedule/merge in `web/src` (non-test) finds only a comment in `review/time.ts` and an unrelated `schedule` callback in `Minimap.tsx`.
- [x] CLAUDE.md, AGENTS.md, brain/ (ADR 0006), Makefile, README updated; OKF strict passes.
  - Evidence: D60 sweep; `make okf` strict passes (inside `make gate`).
- [x] `prod` untouched until you explicitly ask.
  - Evidence: Superseded: `prod` fast-forwarded to `staging` on the user's explicit request, 2026-11-01.

## Animation tracker (from 05)
`1 route · 2 ws switch · 3 sidebar · 4 overlays · 5 toasts · 6 palette · 7 banner · 8 skeleton · 9 buttons · 10 countup · 11 focus · 12 ws cards · 13 delete · 14 graph load · 15 node birth · 16 layout tween · 17 hover dim · 18 select/fly-to · 19 pan/zoom · 20 multi-select · 21 hull LOD · 22 delete anim · 23 chain rows · 24 messages/stream · 25 thinking · 26 stepper · 27 DAG reveal · 28 status change · 29 dock · 30 quiz reveal · 31 slider · 32 probe bar · 33 job chips · 34 card transition · 35 reveal · 36 rating bar · 37 streak · 38 summary · 39 forecast · 40 digest · 41 ingest · 42 accounts · 43 settings · 44 empty · 45 error`
Mark each: `[ ] implemented  [ ] reduced-motion variant  [ ] perf checked`.

### P30 status (code audit + vitest; Playwright/perf not run)
- Implemented (code present): 1–34, 36–45. Evidence: `AppShell` route/ws motion, `Sidebar`/`WorkspaceChip` layoutId, `motion.css` keyframes, `CountUp`, `PhaseStepper`, `CooldownRing`, graph `camera-controller`/`tween`, review `rating-bar`/`session-summary`/`forecast`.
- #35 flip: N/A by spec (reserved for open-recall cards; 4-choice uses overlay).
- #2 content blur-to-sharp added in P30 (`.lp-ws-enter`).
- Reduced-motion variant: CSS (`@media` + `data-reduce-motion`) and `MotionConfig` verified by `src/lib/reduced-motion.test.ts`; the Playwright `document.getAnimations()` check is still open.
- Perf checked: not run (needs `make e2e` trace spec) — leave `[ ] perf checked` unticked.


## Real-engine run (2026-11-01, capped)
Isolated `LEARNPUPPY_HOME`, `serve --engine real`, one workspace, one teach chat, 2 turns, over HTTP (no browser).
- Result: Probe turn 1 reached `idle` with a pending `probe` question after 6 s; the answer was accepted (202) and turn 2 reached a new pending `probe` after 8 s. No errors.
- Bug found only by this run and fixed: the driver's `env_clear` allow-list omitted `USER`/`LOGNAME`, so every non-default Claude profile failed with `Not logged in` (exit 1, empty stderr) even though the pool listed it `ready`. Added both to `ENV_ALLOW`.
- Known gap: `/accounts` shows `tokensIn: 0, tokensOut: 0` after real calls (`requests` does increment), so usage accounting is not parsed from the real CLIs.
- Not exercised on the real engine: Plan/Teach phases, workbench jobs, PDF, the browser UI, `codex`/`agy` roles (a direct `codex exec` ping with the prober settings worked).
