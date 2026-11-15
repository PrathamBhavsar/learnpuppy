---
type: adr
title: "ADR-0009: Borrow UI Ideas From learnharness, Not Code"
description: LearnPuppy adopts learnharness's UI ideas with its own small implementations, keeps math and Mermaid lazy, and builds textbook mode on the cheap-tutor cost rule.
tags: [adr, ui, licence, markdown, textbook, cost]
timestamp: 2026-11-11
---

# ADR-0009: Borrow UI Ideas From learnharness, Not Code

Back to [Decisions Index](/decisions/index.md) | [Brain Index](/index.md). Related: [UI Polish Plan](/ui-polish/index.md), [ADR-0007](/decisions/0007-course-as-data.md), [ADR-0008](/decisions/0008-learning-method.md).

## Context

`amosblomqvist/learnharness` is licensed AGPL-3.0 and renders richer chat content than LearnPuppy. LearnPuppy already renders KaTeX math, but not Mermaid, half-streamed markdown, prose typography, per-event cards, or chapter-level reading checks.

## Decision

1. Implement the ideas ourselves on the installed stack (`react-markdown`, `remark-math`, `rehype-katex`, `shiki`, `motion`). Copy no learnharness code.
2. Prefer a small helper over a new dependency. Add `mermaid` only as a lazy chunk. Evaluate `streamdown` only if the stream tests show a gap.
3. UI changes cause zero model calls. Textbook mode uses at most one cheap call per chapter at import and none while studying.
4. No plugin API now.

## Consequences

- No AGPL obligations enter the codebase.
- Bundle budgets in `docs/web/04-frontend-foundation.md` still hold.
- Textbook mode stays inside the G4 budget by storing objectives and items as data.

## Alternatives rejected

- **Adopt `streamdown` now:** a heavier dependency before we know the small helper falls short.
- **Port their plugin system:** large surface, no current owner need.
- **Vision-graded drawings:** adds a model call per answer; self-grade first.
