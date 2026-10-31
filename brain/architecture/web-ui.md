---
type: spec
title: Web UI Architecture (lp-server + web/ SPA)
description: How the local web UI is built, a loopback axum server embedding a Vite + React + shadcn SPA over the no-HTTP lp-service logic crate, with workspace-scoped URLs and OpenAPI as the contract.
tags: [architecture, web, react, axum, lp-server, lp-service, workspaces, openapi]
timestamp: 2026-11-01
---

# Web UI Architecture

Back to [Architecture Index](/architecture/index.md) | [Brain Index](/index.md). Related: [ADR-0006](/decisions/0006-web-ui-over-dioxus.md), [Server API](/architecture/server-api.md), [System Architecture](/architecture/system.md). Full spec: `docs/web/README.md`.

## Shape
- `learnpuppy serve` (crate `lp-server`) binds `127.0.0.1` only and serves the SPA embedded from `web/` plus a JSON/SSE API.
- `lp-service` holds all app logic (settings, engine routing, chat, review, digest, graph, accounts, ingest, paths) with no HTTP and no UI types; `lp-server` maps its structs to DTOs.
- `web/` is Vite + React + TypeScript + Tailwind 4 + shadcn (bun only). TypeScript renders and sends intent; FSRS, merge, layout, plan statuses and option shuffle/verdict stay in Rust.
- OpenAPI is the contract: Rust DTOs -> `web/openapi/openapi.json` (`make api-gen`, checked by `make api-check`) -> generated TS types -> MSW mocks -> contract tests.

## Workspaces
Workspaces are isolated knowledge bases under the data root (`lp_service::paths`). The workspace id lives in the URL (`/w/:ws/...`), never in server-global state, so two tabs can show two workspaces. Every root is passed to `Stores::open(root)`.

## Rules
- Run blocking store/filesystem work in `spawn_blocking`; no `unwrap()` in routes.
- Never expose OAuth tokens or keychain data in responses.
- Frontend tasks build against MSW until backend lands; E2E runs with the real server (`VITE_API=real`).
