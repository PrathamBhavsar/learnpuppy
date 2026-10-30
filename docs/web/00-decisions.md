# 00 — Scope & Locked Decisions

## Goal (from you)
A finished, polished local web product: clean UI, real animations, every feature working, multiple workspaces. Not a prototype.

## In scope
- Every feature the Dioxus app was meant to have, plus what the TUI had: Knowledge Graph, Classroom (Teach + Practice, 3 phases), FSRS review, Decision Digest, transcript Ingest, Accounts/role slots, Settings, command palette.
- Multiple workspaces: create, rename, archive, delete (to trash), export/import, switch, per-workspace ingest sources.
- Light **and** dark theme (dark-first), RTL-ready layout, reduced-motion support, keyboard-first.
- Single binary experience: `learnpuppy serve` serves API + built SPA.

## Non-goals (explicit, so scope cannot creep)
- No mobile layout below ~900px wide (works, not designed). No native shell (Tauri/Electron) — can wrap later, nothing blocks it.
- No remote/multi-user access, no accounts/login system, no cloud sync. `127.0.0.1` only.
- No moving nodes between workspaces (v1). Cross-workspace = search only.
- No in-browser LLM calls or API keys. All model work goes through the existing local CLI pool (`claude`/`codex`/`agy`).
- No Mermaid *rendering* library. The server parses `plan_init` Mermaid into `PlanTree`; UI draws its own DAG.
- No SSR/Next. Pure SPA.

## Locked decisions

| ID | Decision | Why | Rejected |
|---|---|---|---|
| D1 | New crate `lp-service` (logic, no HTTP) + `lp-server` (axum) | Logic testable without sockets; server stays thin | Everything in one crate (untestable), keep logic in `lp-tui` (being deleted) |
| D2 | Remove `lp-tui`, `lp-app`, `lp-ui` from the Cargo workspace; port reusable code (`review.rs`, `session.rs`, `engine.rs`, settings) into `lp-service` | You asked to drop them; Dioxus builds also eat the 1.4 GB free disk | Keep as dead code |
| D3 | `web/` replaced by Vite app. Old Next lab deleted after palette/tokens are carried over | One frontend, no confusion | Second dir `apps/web` |
| D4 | shadcn init exactly: `bunx --bun shadcn@latest init --preset buFyzGS --base radix --template vite --rtl --pointer` (verified in V00 before use) | Your instruction | Hand-rolled components |
| D5 | Palette = existing torlink palette over preset structure (colour vars only), generated from hex by `scripts/hex2oklch.ts` | "with the colors we have" | Hand-picked OKLCH |
| D6 | OpenAPI (utoipa) is the contract → `openapi-typescript` + `openapi-fetch` | Drift is detectable in CI; MSW mocks free | ts-rs/specta (no HTTP contract), hand-written types |
| D7 | One multiplexed **SSE** stream per tab (`GET /api/v1/events?workspace=…`); commands are plain POSTs | Simple, proxy-friendly, replayable via `Last-Event-ID`; avoids WebSocket state | WebSocket, per-chat EventSource (HTTP/1.1 6-connection cap) |
| D8 | Domain logic never duplicated in TS (FSRS, merge, layout, plan statuses, option shuffle/verdict) | One source of truth; Rust already has proptests | Re-implement "just the easy bits" |
| D9 | Workspace id lives in the URL path; server has no "current workspace" | Multi-tab correctness, deep links | Server-side session state |
| D10 | Graph = custom **Canvas2D** renderer + d3-zoom camera + quadtree hit-test, DOM overlay for labels/selection, parallel accessible list | 5k+ nodes at 60fps; lyra square look is easy to draw | SVG (slow >1.5k), sigma/WebGL (hard to match style) |
| D11 | Plan DAG = SVG with `@dagrejs/dagre` layout | Small graphs, crisp, easy motion | Canvas |
| D12 | Server state: TanStack Query. Client UI state: React state + tiny Zustand stores (graph camera/selection, palette). Router: TanStack Router (typed, file-based) | Typed `/w/$ws` params; cache + SSE invalidation is clean | Redux, React Router |
| D13 | Animation lib: `motion` (Framer Motion successor) for orchestrated/layout/presence; `tw-animate-css` + Radix `data-state` for primitives; Canvas tweens via own rAF clock | Right tool per layer; reduced-motion handled centrally | CSS-only (no layout/exit), GSAP |
| D14 | Fonts bundled locally (`@fontsource-variable/geist-mono`, whatever the preset names for heading/sans); zero network at runtime; strict CSP | Local app must work offline | Google Fonts |
| D15 | Icons: lucide-react (V00 verified: radix-lyra preset uses lucide; Hugeicons dropped) | Matches preset/earlier choice | Mixed icon sets |
| D16 | Tests: Vitest + Testing Library + MSW (unit/component), Rust tests per crate, **Playwright** (system Chrome + WebKit if disk allows) for E2E against real server running `FauxEngine` | Real browsers now; deterministic LLM | Agentic-only E2E; tester-army kept optional |
| D17 | Server binds `127.0.0.1:7878` (auto-increments if busy, writes `~/.learnpuppy/server.lock`), per-launch random token, Host/Origin checks, no CORS | Server can spawn authenticated CLIs — must not be drivable by a random website | Open localhost API |
| D18 | Package manager: **bun** in `web/` only. Root `package.json` e2e leftovers removed or repointed | Matches prior rule | npm/pnpm |
| D19 | Delete = move to `~/.learnpuppy/.trash/<ts>-<id>/` (manual purge in Settings) | Irreversible loss of a learning history is unacceptable | Hard delete |
| D20 | Time is server-authoritative (epoch ms from server); client never computes due/interval | FSRS correctness, clock-skew safe | Client `Date.now()` |

## Naming
- Product: LearnPuppy. Binary: `learnpuppy`. Crates: `lp-*`. Web package: `learnpuppy-web`.
- API prefix: `/api/v1`. IDs: workspace `ws_<8hex>`, chat/node/card IDs unchanged from existing crates.
- Commit style unchanged: short lowercase `feat:/fix:/test:/docs:`; branch `prod <- staging <- pratham404/<task-id>`; `make gate` before every merge.
