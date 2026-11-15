---
type: log
title: UI Polish Plan Review Log
description: Elephant-Goldfish review verdicts on the UI polish plan, the changes they caused, and the owner's final verdict.
tags: [review, egm, tribe, log]
timestamp: 2026-11-11
---

# UI Polish Plan Review Log

Back to [UI Polish Plan](/ui-polish/index.md). Method: Elephant writes and seals the plan, fresh Goldfish reviewers with no chat context read the sealed files and return `pass` or `revise`.

| Round | Reviewer | Verdict | Findings | Plan change |
|---|---|---|---|---|
| 1 | Claude (fresh `claude -p`, plan mode) | revise | 9 findings: wrong paths (`web/scripts/contrast.ts`, `web/openapi/openapi.json`), U0 output dir clash, import call count vs cap, dictation privacy claim, untestable acceptance wording, bundle number, mislabelled link, U9 scope | All fixed in the plan; the OKF frontmatter finding was wrong (OKF forbids it on folder indexes; `make okf` passes) |
| 1 | Antigravity (fresh `agy`) | revise | 6 findings: U1 and U2 both edit `MarkdownView.tsx`, U2 and U3 both edit `styles.css`, parallel `openapi.json` collisions, Mermaid needs `htmlLabels: false` and DOMPurify, PDF page offsets and long chapters, no async import protocol, U8 missing the U4 dependency | All fixed: U2 after U1 with its own css, regenerate-on-rebase rule, sanitising, offset detection and chapter splitting, job lifecycle, U4 to U8 edge |
| 1 | Codex (goldfish task 26) | not run | Codex login refresh token revoked; run `codex login` for a third reviewer | Optional round 2 after login |

## U7 build notes (textbook backend)

Built from [Textbook Mode Backend Spec](/learning-method/textbook-spec.md) on branch `pratham404/u7-build`, one commit per build step, `make gate` and `make ci` green.

**D7 answer.** Closed as a hard lifetime cap of 40 cheap calls per book, counted from the ledger's `Usage` lines (`auto_calls + requested_calls`) and re-read before every chapter, every part and every retry. Tests: `auto_import_never_exceeds_40_calls`, `requested_import_cannot_exceed_lifetime_40`, `retry_rechecks_cap_at_39`.

**Changes from the spec.**

| Area | Change | Why |
|---|---|---|
| `ChapterView` | Gained `retryable: bool`. | `pending_chapters` must tell a retryable failure from `no_cards`; only the fold sees the ledger. |
| `TextbookError::Conflict` | Carries the stable reason code (`import_running`, `nothing_to_import`, `call_budget_used`, `chapter_not_ready`, `already_answered`), not a sentence. | The route puts it in `details.reason`; a code needs no string matching. Messages are written in the route. |
| `TextbookService::answer` | Takes `AnswerMeta { confidence, ms, client_id }` instead of loose arguments. | The spec's `clientId` replay needs the id in the service, and eight arguments trips clippy. |
| Cancel | A call cancelled in flight still gets its `Usage` line. | A call counts once it was sent; skipping it would let the 40 cap be exceeded by cancel and retry. |
| Retry | A failed chapter is retried whole (all parts) once, with the cap checked before the retry starts and before each call. | Literal reading of "retries the whole chapter once". |
| Workspace export | `textbooks/` is **not** added to the export zip. | The zip has an explicit file list with checksums, `courses/` is not in it either, and adding a tree needs a manifest and import change. Left for its own task. |
| Test hook | `AppState::with_textbook_pdf` swaps the PDF extractor for route tests. | Spec left the choice to the builder. |
| Abort race | A job whose final record is being written is marked `ending` and cannot be aborted. | Found by a route test: abort of a just-finished job returned 200 instead of 409. |
| Import body | `POST …/import` requires a JSON body (`{}` is fine). | Same `ApiJson` extractor as the course routes. |

**Manual check not run in CI.** `python3 scripts/real_smoke.py --textbook BOOK.pdf` uploads, polls the jobs route, prints chapters and two objectives each, and asserts the call cap. It needs real CLI quota and was not run during the build.

## Owner verdict

(pending the real comparison after U0 to U9)
