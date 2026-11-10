---
type: reference
title: UI Polish Baseline
description: Build chunk sizes and screenshot recipe recorded before any UI polish package lands, for before and after comparison.
tags: [baseline, bundle, screenshots, ui]
timestamp: 2026-11-11
---

# UI Polish Baseline

Back to [UI Polish Plan](/ui-polish/index.md). Recorded at package U0, before any source change. `bun run bundle-size`: 129 chunks within the 120 kB gzip budget.

## Largest build chunks (`bun run build`, raw / gzip bytes)

| Chunk | Raw | Gzip |
|---|---|---|
| `index` (main JS) | 501,773 | 158,660 |
| `index` (CSS) | 134,349 | 21,001 |
| `MarkdownMath` (katex, lazy) | 273,588 | 81,907 |
| `MarkdownView` | 165,915 | 49,936 |
| `GraphScreen` | 155,246 | 51,299 |
| `core` | 94,338 | 29,171 |
| `schemas` | 78,571 | 22,456 |
| `tooltip` | 60,172 | 21,166 |
| shiki grammars (`cpp` 796,960 / 55,629, `typescript`, `tsx`, `javascript`) | lazy | each under 17 kB gzip except `cpp` |

Hashes change per build; compare by name prefix.

## Screenshots

`cd web && bunx playwright test ui-baseline` writes classroom, course lesson (a freshly started course) and digest at 1440 and 390 px, light and dark, to `web/e2e/.shots/` (gitignored). Run it on this baseline commit and again after a package to compare by eye.

## Streaming fixture

`web/e2e/fixtures/streaming-message.ts`: one message in 12 chunks with a fence, `$$ x^2 + 1 $$`, a table and a Mermaid block. U1 and U2 consume it.
