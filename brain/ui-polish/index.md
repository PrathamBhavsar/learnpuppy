# UI Polish and Textbook Mode Plan

Back to [Brain Index](/index.md). Details: [Task DAG](/ui-polish/tasks.md), [Test Plan](/ui-polish/test-plan.md), [Review Log](/ui-polish/review.md). Decision: [ADR-0009](/decisions/0009-ui-polish-borrow-ideas-not-code.md). Related: [Learning Method Plan](/learning-method/index.md), [Cheap Tutor Plan](/cheap-tutor/index.md).

**Status:** planned 2026-11-11, not started. Review by the Elephant-Goldfish pass is logged in [Review Log](/ui-polish/review.md).

## Why

The owner compared LearnPuppy with `amosblomqvist/learnharness` and judged its UI more polished and closer to the goal. A code read (not a rendered comparison) found the gaps below. LearnPuppy already renders KaTeX math, including `\( \)` and `\[ \]`, with tests, so math is **not** a gap. Mermaid, stream tolerance, prose typography, per-event cards, spoken or drawn answers, and chapter-level reading checks are.

## What we borrow, and what we do not

| Idea in learnharness | Our version | Why this size |
|---|---|---|
| `streamdown` for half-streamed markdown | A small `completePartial()` helper in front of the existing `react-markdown`. Adopt `streamdown` only if U1 tests show gaps it would close | Reuses the installed stack, no new dependency for a few lines |
| Mermaid blocks | Lazy `mermaid` chunk, `securityLevel: "strict"`, error card fallback | Models draw flow diagrams; the plan DAG already proves the need |
| Type ladder, CMU Serif titles, reading font setting | One prose font for lesson text, mono kept for chrome, `readingFont` workspace setting | Mono-only prose hurts long reading; identity stays mono |
| One card per tool event | Cards for quiz asked, job running or done, visual ready, steer, probe done | Chat reads as a lesson, not a log |
| Think-out-loud fields plus dictation | Optional "explain your thinking" box, Web Speech API when present | Matches the Alvar method; no server model call |
| Drawing pad answers | Canvas answer type, PNG saved with the answer, self-graded | Cheap tier: no vision call by default |
| KaTeX glyphs for chain connectives | `∧` and `⟹` rendered through the existing KaTeX seam | Logic is the payload |
| Plugin API | **Not borrowed** | Large surface, no owner need; AGPL licence makes copying code unsafe |

**Licence rule:** learnharness is AGPL-3.0. Read it for ideas; write our own code. Never paste its files.

## Non-negotiables (from the project guardrails)

1. Domain logic stays in Rust. The SPA renders DTOs and sends intent. FSRS, grading, objective extraction and card generation never move to TypeScript.
2. Cheap tutor cost rule ([ADR-0007](/decisions/0007-course-as-data.md)): UI work causes **zero** model calls. Textbook mode may use **one cheap-tier call per chapter** at import (capped at 40 per book, outside the G4 hour cap, shown on the usage route) and none while studying.
3. Bundle budget: main chunk stays at or under 220 kB gzip (`docs/web/04-frontend-foundation.md`); `mermaid`, `katex`, `shiki` stay lazy.
4. Accessibility: keyboard, light and dark, RTL, reduced motion, contrast 4.5:1 all keep passing.
5. Every package merges through `make gate` in its own worktree, then `git merge --no-ff` into `staging`.

## Packages

Ids `U0` to `U9`. Full file lists, acceptance and tests are in [Task DAG](/ui-polish/tasks.md).

| Id | Package | Wave | Model tier |
|---|---|---|---|
| U0 | Baseline: screenshots, bundle sizes, streaming fixture | 0 | Codex medium |
| U1 | Stream-safe markdown (`completePartial`) | 1 | Codex medium |
| U2 | Mermaid lazy block | 1 | Codex medium |
| U3 | Type ladder and reading font | 1 | Claude medium |
| U4 | Event cards | 2 | Codex medium |
| U5 | Think-out-loud field and dictation | 2 | Codex medium |
| U6 | Chain connectives as KaTeX glyphs | 2 | Codex low |
| U7 | Textbook mode backend (F37 part 1): page map, objectives, cards | 2 | Claude high plan, Codex medium build |
| U8 | Textbook mode screens (F37 part 2): chapter check, "I read this" quiz | 3 | Claude medium |
| U9 | Drawing answer type (parked) | 3 | Codex medium |

U9 is parked: it needs a stored image field and an upload path, so it waits for the owner's verdict.

## Order and merge flow

- Branches `pratham404/u<n>-<slug>` from `staging`, one worktree each, at most **4 workers at once** (tribe cap).
- Merge in wave order. Wave 1 and wave 2 packages touch disjoint files except `Message.tsx` (U4 owns it; U1, U2 expose components only).
- After each merge: run `make gate`, delete the worktree and its branch, and stop that worker. No idle worktrees or workers stay around.
- Commit messages are short and lowercase: `feat: ...`, `fix: ...`, `test: ...`, `docs: ...`.
- No push and no `prod` merge without the owner's approval.

## Done means

- `make ci` and `make e2e` pass on `staging`.
- Before and after screenshots from U0 are compared by the owner in the browser.
- Textbook mode imports a real PDF chapter and produces reviewable FSRS cards, checked in the browser with the faux engine and once with a real cheap model.
- Owner verdict on "does it feel like learnharness" is recorded in [Review Log](/ui-polish/review.md).
