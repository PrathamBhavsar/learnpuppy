---
type: guide
title: UI Polish Test Plan
description: How each UI polish and textbook package is tested, which checks run in CI versus by hand, and how the owner judges polish.
tags: [testing, e2e, a11y, bundle, visual, textbook]
timestamp: 2026-11-11
---

# UI Polish Test Plan

Back to [UI Polish Plan](/ui-polish/index.md). Task detail: [Task DAG](/ui-polish/tasks.md).

## Layers

| Layer | Tool | What it proves | In CI |
|---|---|---|---|
| Unit | Vitest, `cargo test`, `proptest` | `completePartial` cases, chapter page-range invariants | Yes |
| Component | Testing Library, MSW | Each card kind, Mermaid error card, think field, dictation (mocked) | Yes |
| Contract | OpenAPI drift check, DTO leak scan | New fields exist in the contract; no correct answer reaches an unanswered quiz | Yes |
| Server | axum oneshot, `FauxEngine` | Textbook import, reading check makes zero model calls, upload rejection | Yes |
| Store | `lp-store` tests | Torn tail, concurrent append, two workspaces isolated | Yes |
| E2E | Playwright (`make e2e`) | Question to verdict, PDF upload to chapter check, Mermaid and math in a real browser | Yes |
| Accessibility | `@axe-core/playwright`, `scripts/contrast.ts` | Contrast, names, focus, reduced motion | Yes |
| Bundle | `bun run build` size check | Main chunk in budget, `mermaid` and `katex` stay lazy | Yes |
| Visual | U0 screenshots, before and after | Owner compares by eye | **Manual** |
| Real model | `scripts/real_smoke.py` | Chapter objectives from a real cheap model look sane | **Manual** |

## Rules

1. Every package adds its tests in the same commit as its code. A package with no new test is not done.
2. A test must fail without the change. For the streaming helper, keep a test that renders raw chunks and asserts the raw syntax shows, so the guard is proven.
3. Any new string on screen has an accessible name. Any new motion honours reduced motion.
4. **Zero-call rule:** a service test wraps the engine in a call counter and asserts 0 calls for rendering, studying and the reading check, and at most 1 per chapter at import (plus 1 for the whole book only when the PDF has no outline), never more than 40 per book.
5. Cost guard: a study session after import must stay inside the G4 hour budget (under 100k tokens, at most 2 strong and 22 cheap calls per hour). The import itself is a separate, capped, one-off cost visible on the usage route.

## Textbook mode, specific checks

- **Fixture:** a small generated PDF with 3 chapters, an outline, a two-column page, a page with only an image, and one with a prompt-injection sentence ("ignore previous instructions"). Expect: chapters found, empty page skipped without error, injection text treated as plain text and never obeyed.
- **Large file:** a 400-page synthetic PDF imports in the background; the server stays responsive (health route under 200 ms during import).
- **Idempotence:** importing the same PDF twice produces no duplicate objectives or cards.
- **Grading:** the reading check grades by code; a wrong option never turns right through model output.

## Owner judgement (manual, recorded in the review log)

1. Open the same lesson in the old and new build, light and dark, desktop and phone width.
2. Stream a long answer with a table, math, code and a diagram; watch for flicker.
3. Import one real chapter of your own book; judge whether the objectives match what you would test.
4. Say yes or no to "feels like learnharness".
