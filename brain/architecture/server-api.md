---
type: spec
title: Server API Summary (lp-server)
description: Brain-level summary of the lp-server HTTP and SSE surface, its conventions, and where the authoritative endpoint list lives.
tags: [architecture, api, axum, sse, openapi, lp-server]
timestamp: 2026-11-01
---

# Server API Summary

Back to [Architecture Index](/architecture/index.md) | [Brain Index](/index.md). Related: [Web UI](/architecture/web-ui.md), [ADR-0006](/decisions/0006-web-ui-over-dioxus.md). Authoritative: `docs/web/03-server-api.md` and the generated `web/openapi/openapi.json`.

## Groups
- **Global**: `/health`, `/bootstrap`, `/events` (SSE), `/search`, `/accounts`, `/settings`, `/trash`, `/downloads/{token}`.
- **Workspaces**: list, create, patch, archive/restore, delete to trash, export, import.
- **Per workspace** (`/workspaces/{ws}/...`): graph (read, search, relayout, delete-preview and delete, practice), chats (turn, cancel, jobs, artifacts, PDF sources), review (queue, answer, grade, stats, sync), digest, ingest.

## Conventions
- Loopback only, Host-checked, errors as typed codes (for example `409 turn_in_progress`).
- Chat turns return `202 {turnId}`; progress arrives over SSE, never by polling the turn.
- Destructive graph deletes require a preview token bound to the previewed content.
- Answers are withheld from review queue payloads until the answer endpoint is called.
