---
type: adr
title: "ADR-0007: Course As Data, Strong Model Rarely"
description: Proposed move from one model call per learner step to a stored course written by a rare strong call and run by deterministic code.
tags: [adr, cost, lesson-packs, routing, memory]
timestamp: 2026-11-03
---

# ADR-0007: Course As Data, Strong Model Rarely

Back to [Decisions Index](/decisions/index.md) | [Brain Index](/index.md). Related: [Cheap Tutor Plan](/cheap-tutor/index.md), [Benchmark](/cheap-tutor/benchmark.md).

## Status

**Proposed** (2026-11-03). Awaits the owner's approval and gate G1 in the [Task DAG](/cheap-tutor/task-dag.md).

## Context

1. A learner's Spanish chats stopped after 3 and 4 questions, before any teaching.
2. Each step is a new CLI process plus the whole transcript. A one-word reply costs about 20,700 input tokens on Codex and 11,300 on Antigravity.
3. An hour in today's shape projects to 620,000 to 1,030,000 input tokens on cheap models. The project cannot survive this.

## Decision

1. A strong model writes about an hour of lessons in one call. The result is stored as data.
2. Deterministic code runs the session: sequencing, multiple-choice grading, typed answer matching, spaced repetition.
3. The only thing a cheap model sees is a fixed-size context packet, and it runs only on learner request.
4. A strong model returns about once a study hour, with a compact digest, to write the next pack.
5. Every role always falls back. If everything is limited, study continues from stored lessons.

## Alternatives considered

| Option | Hour's usage, Spanish | Judged quality | Why not |
|---|---|---|---|
| Today's per-step | about 670,000 to 1,042,000 | 3 / 3 / 2 / 2 | Unaffordable |
| Per-step with fixed state, Haiku | about 114,000 | Not judged | A call per step, so limits still stop study, and each step waits 6 seconds |
| Strong skeleton + Haiku | about 68,000 | 4 to 5 | 6x the tokens for no clear gain |
| Cheap skeleton + Haiku | about 65,000 | 4 to 5 on most | **Kept as the fallback, and the default if G1 fails** |
| Skeleton + Qwen | about 6,000 to 9,000 | Accuracy 1 to 4 | Wrong facts taught |

Chosen: one shot, Opus. About 11,500 tokens, accuracy 4 to 5.

## Consequences

- **Good:** an order of magnitude less usage. Study survives limits. Steps show instantly. The Review and Graph screens fill up as a side effect.
- **Bad:** content cannot adapt within a pack, only between packs. There is a wait of about 96 seconds on first use. The tutor feels less like a live conversation.
- **Risk:** Opus's quota weight is unknown. The lead over Haiku holds only if an Opus token weighs under about 5.7 Haiku tokens. The meter in T8 gives real data, and the fallback path is measured and ready.
- **Risk:** the evidence is one run per cell, and a judge noisy by about 1 point. G1 tests this before any large build.
