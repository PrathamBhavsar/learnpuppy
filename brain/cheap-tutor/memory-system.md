---
type: spec
title: Memory System
description: The tiered learner and course memory, its files and schemas, update rules, and the fixed-size context packet that is all a cheap model ever sees.
tags: [memory, learner-model, fsrs, context-packet, storage]
timestamp: 2026-11-03
---

# Memory System

Back to [Cheap Tutor Plan](/cheap-tutor/index.md). Related: [Architecture](/cheap-tutor/architecture.md), [Lesson Packs](/cheap-tutor/lesson-packs.md), [Hybrid Storage](/architecture/storage-memory.md), [Practice & FSRS](/architecture/practice-fsrs.md).

## 1. Principles

1. **Nothing is remembered by re-sending it.** The transcript is never a model input.
2. **The event log is the only truth.** Everything else about the learner is derived by a pure fold, so it can be rebuilt and tested, and it never drifts.
3. **Writes are mostly code.** Models write course content. Code writes what happened.
4. **Reads are bounded.** Every model call gets a packet with a hard size cap, however long the course runs.

## 2. The tiers

| Tier | What it holds | Where | Written by | Sent to models |
|---|---|---|---|---|
| Learner profile | Background, goal, preferences, in 3 to 5 lines | `workspaces/<ws>/learner.json` | Placement, Author; the learner can edit it | Always |
| Course | Lessons, explanations, items, key facts | `courses/<id>/course.json` | Author and Review calls | Current lesson only |
| Event log | Every answer, skip, flag and question | `courses/<id>/events.jsonl` | Code | Never raw |
| Derived model | Mastery, weak items, must-fix list | Rebuilt by `fold_learner` | Code | As a digest |
| Schedule | FSRS state per item | Tracker `cards`, `fsrs_reviews` | Code, via `lp_core::fsrs` | Never |
| Long-term graph | Propositions across topics | `graph/nodes.jsonl` | Code, from key facts | Not in this plan |

## 3. Event log

Append-only JSONL. One line per event. Uses the existing `lp-store` JSONL helpers, including torn-tail recovery.

```json
{"ts":1793712788313,"kind":"answered","item":"l02-i3","correct":false,"chosen":"Mi llamo","confidence":3,"ms":14200}
{"ts":1793712802000,"kind":"self_graded","item":"l02-i5","grade":"hard"}
{"ts":1793712900000,"kind":"asked","lesson":"l02","question":"why me and not mi?","answer_chars":240}
{"ts":1793712950000,"kind":"flagged","item":"l02-i4","reason":"answer looks wrong"}
{"ts":1793713000000,"kind":"lesson_done","lesson":"l02","correct":4,"total":6}
{"ts":1793713001000,"kind":"usage","role":"follow_up","model":"claude-haiku-5-5","in":1020,"out":610}
```

## 4. Derived model

`fold_learner(course, events, now) -> LearnerState`. Pure, in `lp-core`, property-tested: replaying the same events always gives the same state, and any prefix is valid.

```
LearnerState {
  lesson_scores: { lesson_id: { correct, total } },
  weak: [item_id],            // wrong, or right at confidence 1 or 2
  overconfident: [item_id],   // wrong at confidence 4 or 5
  must_fix: [text],           // probe mistakes not yet answered correctly
  flagged: [item_id],         // never shown again
  asked: [text],              // last 20 learner questions
}
```

Overconfidence is tracked apart because Roediger & Karpicke found that restudy inflates confidence without improving retention, and a wrong answer given confidently is the case most worth revisiting.

## 5. The context packet

`render_packet(role, profile, lesson, state) -> String`. The only thing a cheap model ever receives about the learner. Pure and unit-tested, including its cap.

```
LEARNER: Fluent in English, Gujarati, Hindi. Client in Peru. Goal: talk with coworkers.
LESSON: Introducing yourself. Objective: say your name and what you do.
KEY FACTS: me llamo = my name is; me, not mi, before llamo; ...
WEAK: me llamo vs mi llamo; mañana means tomorrow
JUST ASKED: "How do I say my name?" LEARNER ANSWERED: "Mi llamo Alex" (wrong)
```

Cap: **400 tokens**, estimated as characters divided by 4. When over, it drops in this order: oldest weak items, then key facts beyond the first 8. The State approach in the [Benchmark](/cheap-tutor/benchmark.md), with a comparable prompt, held at about 1,000 total input on Haiku over 10 steps.

## 6. The digest for a Review call

When the pack runs low, the Review call receives a digest instead of a packet, capped at **800 tokens**: profile, lesson scores in a table, weak and overconfident items with their text, learner questions, flagged items, and titles of what has already been taught.

This is the owner's "review at large intervals" idea. The strong model sees the whole picture about once an hour, in under 1,000 tokens, and every other call is cheap or absent.

## 7. Spaced repetition across sessions

Each gated item becomes a tracker card, reusing `lp_core::fsrs` and the existing `cards` and `fsrs_reviews` tables:

- Answering in a lesson logs an FSRS review: wrong is Again, right at low confidence is Hard, otherwise Good.
- Self-graded recall maps directly to the four FSRS ratings.
- `next_step` opens each session with due cards, so returning learners review first, with no model call.
- The existing Review screen and `/review` work unchanged, and this data will fill them.

FSRS timing is already implemented and tested. Only the card creation and rating mapping are new.

## 8. The long-term graph

Key facts are written by the Author in a stated, checkable form, so at `lesson_done` code can append them to `graph/nodes.jsonl` as propositions without a model call. This fills the Graph screen, which is empty today, as a side effect of studying.

**Open question:** whether key facts are good enough as propositions without the Auditor stage. Task T9 checks this. If not, it folds a batch at session end, on a cheap model.

## 9. Failure and privacy

- **A corrupt tail** in `events.jsonl` is dropped, as with existing ledgers. The fold still works on the valid prefix.
- **A course written by a model is untrusted.** It is parsed, gated, and rendered as text. It never runs as instructions in a later prompt unless fenced as data, as with imported notes.
- **Secrets:** learner text goes through the existing `lp_core::redact` before any model call.
- **Isolation:** all paths come from `Stores::open(root)`, per workspace, as `CLAUDE.md` requires.
