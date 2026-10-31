# 10 — Acceptance: "Clean UI, every feature working"

Release = every box ticked, with evidence (test name or screenshot path) filled by A70. State honestly which checks ran against the real engine vs `FauxEngine`.

## A. Boot & safety
- [ ] `learnpuppy serve` prints URL, opens browser, SPA loads with no console errors/warnings.
- [ ] Opening the bare URL without token/cookie shows the "open from terminal" screen; API 401.
- [ ] Wrong `Host`/cross-origin POST/missing CSRF header rejected (tests).
- [ ] Second `serve` reuses running instance. Ctrl-C shuts down cleanly; running jobs resume on next boot.
- [ ] Migration of a copy of the real `~/.learnpuppy` preserves node/chain/chat/card counts; revert works.

## B. Workspaces
- [ ] Create / edit / archive / restore / delete(to trash) / restore from trash / purge / export / import all work.
- [ ] Switch via chip, ⌘1–9, palette; section preserved; two tabs on two workspaces independent.
- [ ] Onboarding from zero state; damaged workspace repair; not-found page.
- [ ] Ingest source detection, overlap warning, empty allow-list disabled.

## C. Graph
- [ ] 800 and 5k node fixtures render at budget; hulls/labels LOD; minimap; fit/zoom/pan inertia.
- [ ] Search, filters, hover-dim, select, multi-select, marquee, keyboard navigation, GraphList mirrors.
- [ ] NodeDrawer with provenance/chains/cards; Open source chat works.
- [ ] Chain table 3-level, sort, virtualization, CSV export.
- [ ] Practice-from-selection creates and opens a Practice chat.
- [ ] Delete preview (blast radius) → confirm → animated removal; token-stale rejection.
- [ ] Relayout animation; new-node birth animation after merge; empty/pending/error states.

## D. Learn
- [ ] Full Faux session: Probe (≤8) → Plan DAG appears → Teach; Practice session with problem + gate.
- [ ] All six question tools render and submit; quiz reveal animations; confidence slider.
- [ ] Streaming text smooth; stick-to-bottom + Jump to latest; thinking block; stop/cancel keeps partial.
- [ ] Rate-limit queue banner with countdown; engine error with Retry; restart-mid-turn Resume.
- [ ] Workbench jobs run/abort/deliver; artifact viewer safe (sandbox); PDF upload.
- [ ] Chat rename/export/delete; stale-question 409 recovers silently.
- [ ] Markdown: code (shiki), tables, math, links safe.

## E. Review & Digest
- [ ] Keyboard-only review (1–4 / space / esc) with previewed intervals; optimistic advance < 200 ms; rollback on error.
- [ ] Stats, forecast, streak, session summary, card browser with history.
- [ ] `/review-all` across workspaces.
- [ ] Digest ranges/filters/search; decision drawer with alternatives & commits; regenerate cards; "Review these".

## F. Ingest, Accounts, Settings
- [ ] Ingest run with live progress, cancel, history, error reporting.
- [ ] Accounts list with cooldown ring; add Claude profile; rescan; disable; role-slot edit + diff + test ping.
- [ ] Theme (light/dark/system), direction (LTR/RTL), reduce-motion override, density, concurrency cap persist.
- [ ] Diagnostics copy contains no secrets (test with planted token).

## G. Shell & cross-cutting
- [ ] Palette: navigate, switch workspace, actions, node search (workspace + all), recents.
- [ ] Shortcuts sheet matches actual bindings (generated from registry).
- [ ] Connection banner on SSE drop; resync after reconnect.
- [ ] Every screen has loading/empty/error/degraded/populated states with assertions.
- [ ] Title/focus/announce on route change; scroll restoration.

## H. Design, motion, a11y
- [ ] Palette = preset structure + torlink colors only (diff script); no hard-coded colors outside tokens.
- [ ] Light & dark contrast script passes; no flash on load.
- [ ] RTL: shell + each screen mirrored correctly; canvas LTR; mixed-direction message OK.
- [x] All 45 animations in 05 implemented (checklist table below); reduced-motion variant verified in vitest (`reduced-motion.test.ts`), Playwright `getAnimations()` check pending.
- [ ] axe: zero serious/critical on all screens/states; keyboard-only walkthrough passes; focus visible everywhere.
- [ ] Perf budgets in 07 pass; 30-min soak has no leak.
- [ ] Visual baselines committed for light/dark × LTR/RTL; 3 consecutive green `make e2e` runs.

## I. Engineering hygiene
- [ ] `make gate` and `make e2e` green; `openapi.json` drift-free; `cargo deny` green.
- [ ] No `unwrap()` in routes; no physical-direction classes; no external network requests (Playwright asserts zero non-localhost requests).
- [ ] `grep` audit: no FSRS/graph/plan logic in `web/src`.
- [ ] CLAUDE.md, AGENTS.md, brain/ (ADR 0006), Makefile, README updated; OKF strict passes.
- [ ] `prod` untouched until you explicitly ask.

## Animation tracker (from 05)
`1 route · 2 ws switch · 3 sidebar · 4 overlays · 5 toasts · 6 palette · 7 banner · 8 skeleton · 9 buttons · 10 countup · 11 focus · 12 ws cards · 13 delete · 14 graph load · 15 node birth · 16 layout tween · 17 hover dim · 18 select/fly-to · 19 pan/zoom · 20 multi-select · 21 hull LOD · 22 delete anim · 23 chain rows · 24 messages/stream · 25 thinking · 26 stepper · 27 DAG reveal · 28 status change · 29 dock · 30 quiz reveal · 31 slider · 32 probe bar · 33 job chips · 34 card transition · 35 reveal · 36 rating bar · 37 streak · 38 summary · 39 forecast · 40 digest · 41 ingest · 42 accounts · 43 settings · 44 empty · 45 error`
Mark each: `[ ] implemented  [ ] reduced-motion variant  [ ] perf checked`.

### P30 status (code audit + vitest; Playwright/perf not run)
- Implemented (code present): 1–34, 36–45. Evidence: `AppShell` route/ws motion, `Sidebar`/`WorkspaceChip` layoutId, `motion.css` keyframes, `CountUp`, `PhaseStepper`, `CooldownRing`, graph `camera-controller`/`tween`, review `rating-bar`/`session-summary`/`forecast`.
- #35 flip: N/A by spec (reserved for open-recall cards; 4-choice uses overlay).
- #2 content blur-to-sharp added in P30 (`.lp-ws-enter`).
- Reduced-motion variant: CSS (`@media` + `data-reduce-motion`) and `MotionConfig` verified by `src/lib/reduced-motion.test.ts`; the Playwright `document.getAnimations()` check is still open.
- Perf checked: not run (needs `make e2e` trace spec) — leave `[ ] perf checked` unticked.

