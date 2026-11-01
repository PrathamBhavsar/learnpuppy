# LearnPuppy Web — Plan Folder

Status: **PLAN ONLY. Nothing here is implemented.** Written 2026-10-29 on `staging`.
Supersedes: the Dioxus phases of `docs/PLAN.md` and all of `docs/DESIGN_SYSTEM_PLAN.md` (palette + token ideas are carried over, see `05-design-and-motion.md`).

## What we are building
A **local-only web app**: `learnpuppy serve` starts a Rust HTTP server on `127.0.0.1`; the browser opens a **Vite + React + shadcn** UI. The existing Rust backend crates (graph, FSRS, agents, CLI-auth pool, ingest, storage) stay and are reused unchanged wherever possible. **Multiple workspaces** (isolated knowledge bases) are a first-class concept from day one.

Dropped for good: the Ratatui TUI (`lp-tui`), the Dioxus desktop app (`lp-app`), and the Rust UI kit (`lp-ui`). TUI history stays reachable on branch `legacy/tui-cli`.

## Read in this order
| # | File | Answers |
|---|---|---|
| 00 | [00-decisions.md](00-decisions.md) | Scope, non-goals, every locked decision and why |
| 01 | [01-architecture.md](01-architecture.md) | Processes, crates, repo layout, data flow, dev/prod run, security |
| 02 | [02-workspaces.md](02-workspaces.md) | Multi-workspace model, disk layout, migration, switching, ingest scoping |
| 03 | [03-server-api.md](03-server-api.md) | Every endpoint, DTO rule, SSE protocol, turn state machine, errors |
| 04 | [04-frontend-foundation.md](04-frontend-foundation.md) | shadcn init, stack, folders, routing, state, data layer, conventions |
| 05 | [05-design-and-motion.md](05-design-and-motion.md) | Tokens/palette, typography, RTL, **full animation spec** |
| 06 | [06-screens.md](06-screens.md) | Every screen: layout, states, interactions, keyboard, testids |
| 07 | [07-testing-and-quality.md](07-testing-and-quality.md) | Test pyramid, fixtures, gates, perf and a11y budgets |
| 08 | [08-task-dag.md](08-task-dag.md) | Waves, tasks, owned files, done-when, order |
| 09 | [09-grill-and-risks.md](09-grill-and-risks.md) | 34 issues found by attacking this plan, and the fix for each |
| 10 | [10-acceptance.md](10-acceptance.md) | The "clean UI, every feature working" checklist = definition of done |
| 11 | [11-help-and-tour.md](11-help-and-tour.md) | In-app help for first-time learners: `InfoTip`, `ScreenGuide`, the first-run tour, how to extend them |

## Five-line summary
1. `crates/lp-service` (new, no HTTP) = app logic lifted from `lp-tui`/`lp-app`; `crates/lp-server` (new) = axum + SSE + OpenAPI + embedded SPA.
2. `web/` is rebuilt as Vite+React 19+TS+Tailwind 4+shadcn (`--preset buFyzGS --base radix --rtl --pointer`) with the torlink palette; the old Next lab is deleted.
3. Domain logic stays in Rust (FSRS, merge, layout, plan statuses). TypeScript only renders and sends intent.
4. OpenAPI is the contract: Rust DTOs → `openapi.json` → generated TS types → MSW mocks → contract tests.
5. Workspace id is in the URL (`/w/:ws/...`), never server-global state, so two tabs can show two workspaces.

## Open questions for you (only 4; all have a default so work can start)
See bottom of `09-grill-and-risks.md`.
