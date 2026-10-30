---
type: adr
title: "ADR-0006: Local Web UI (Rust Server + Vite/React SPA) Over Dioxus Desktop"
description: Replace the Dioxus 0.7 desktop app (lp-app), its Rust UI kit (lp-ui) and the Ratatui TUI (lp-tui) with a local axum server (lp-server) over a no-HTTP logic crate (lp-service) and a Vite + React + shadcn SPA in web/. Supersedes ADR-0005.
tags: [adr, web, react, axum, lp-service, lp-server, architecture, ui]
timestamp: 2026-10-31
---

# ADR-0006: Local Web UI (Rust Server + Vite/React SPA) Over Dioxus Desktop

Back to [Decisions Index](/decisions/index.md) | [Brain Index](/index.md). Related: [ADR-0005](/decisions/0005-dioxus-desktop-over-tui.md) (superseded), [System Architecture](/architecture/system.md), [Task DAG](/tasks/index.md).

## Status
**Accepted** (2026-10-31) — Supersedes [ADR-0005](/decisions/0005-dioxus-desktop-over-tui.md). The full plan lives in `docs/web/` (`00-decisions.md` is the locked decision table; `08-task-dag.md` the waves).

## Context
1. The product goal is a finished, polished UI with real animations, every feature working, and multiple workspaces. A hand-rolled Rust component kit (`lp-ui`) cannot reach shadcn/Radix polish, motion and accessibility at a reasonable cost.
2. Dioxus desktop builds were the largest consumer of a tight disk budget (wry/tao/webkit trees), and the WKWebView shell added nothing a browser tab does not already give a local-only app.
3. Every backend crate (`lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest`) is UI-agnostic and stays unchanged; only the UI layer and the app glue move.

## Decision
1. **Remove `lp-ui`, `lp-app` and `lp-tui`** from the Cargo workspace and delete them. TUI history stays on branch `legacy/tui-cli`.
2. **New `crates/lp-service`** (no HTTP, no UI types): settings and profiles, `CliPoolEngine` role routing, the classroom turn loop and view (`run_to_rest`, `pending_call`, `classroom_view`, answer rendering, option resolution, verdicts), FSRS review (`due_cards`, `grade`, `sync`, `export_cards`, `streak`), digest, graph edits, accounts, ingest, and the data-root `paths` helper. Ported from `lp-tui`/`lp-app` with their unit tests.
3. **New `crates/lp-server`**: binary `learnpuppy` (`serve`, `migrate`, `seed`, `doctor`, `openapi`) — axum + SSE + OpenAPI + embedded SPA, bound to `127.0.0.1` with a launch token and Host/Origin/CSRF checks.
4. **`web/`** is a Vite + React 19 + TypeScript + Tailwind 4 + shadcn SPA (bun). OpenAPI generated from Rust DTOs is the contract; domain logic (FSRS, merge, layout, plan statuses, option shuffle/verdict) is never duplicated in TypeScript.
5. **Workspaces** (isolated knowledge bases) are first-class: each lives under `~/.learnpuppy/workspaces/<ws_id>/` and is passed as the root to the unchanged store constructors.

## Consequences
- The workspace has exactly seven crates: `lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest`, `lp-service`, `lp-server`.
- The Dioxus rules ([Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md)) and the TUI spec ([Legacy TUI](/architecture/tui.md)) are archived; they describe code that no longer exists on `staging`.
- Windows/mobile Dioxus phases (`T70`, `T80`) are cancelled; a native shell can wrap the web app later without blocking anything.
- The root `e2e.config.ts`/`tests/` Dioxus harness is gone; browser E2E moves to Playwright under `web/e2e/`.
