---
type: spec
title: Hour-Long Session
description: What an hour of study looks like step by step, the sequencing rules behind it, and how to tell whether the learner actually learned.
tags: [session, pedagogy, runner, sequencing, retention]
timestamp: 2026-11-03
---

# Hour-Long Session

Back to [Cheap Tutor Plan](/cheap-tutor/index.md). Related: [Research](/cheap-tutor/research.md), [Memory System](/cheap-tutor/memory-system.md), [Lesson Packs](/cheap-tutor/lesson-packs.md).

## 1. The failure this answers

The owner: "right now I didn't learn anything and it was a waste of time." The two Spanish chats on disk show why. Every question so far was an assessment, and teaching had not started when the session stopped. No lesson had been given, so nothing could have been learned.

**Rule: the learner is being taught within two minutes of stating a goal.** Placement is short, and answers show their explanation immediately, so placement itself teaches.

## 2. First session

| Time | What happens | Model use |
|---|---|---|
| 0:00 | Learner types a goal | None |
| 0:00 | Placement questions written | 1 cheap call, about 12 seconds, estimated |
| 0:15 | 5 questions, each with immediate feedback. Self-rating is an option on each. | None |
| 1:30 | Author starts in the background. Screen shows what was learned from placement. | 1 strong call, about 96 seconds |
| 1:30 | While waiting: placement misses are shown as a first mini-lesson, from their `why` text | None |
| 3:00 | Lesson 1 | None |
| Then | Lessons continue, mixing in earlier items | None |
| 25:00 | Suggest a 5-minute break, skippable | None |
| On request | Follow-up questions | 1 cheap call each |
| About 50:00 | Refill starts in the background, if needed | 1 strong call |
| End | Summary: what you learned, what is due and when | None |

The wait at 1:30 is the weakest moment of this design. It is about 96 seconds, measured once. T4 must fill it with real content, not a spinner.

## 3. Returning session

1. Up to 10 due reviews, via FSRS. No model call.
2. Anything failed last time, spaced apart.
3. The next lesson.

If every AI is limited, steps 1 to 3 still work. This is the direct answer to "All your AIs are limited" ending a session.

## 4. Runner rules

`next_step(course, state, now) -> Step`. Pure, in `lp-core`, property-tested. In priority order:

1. A due review, up to a cap of 10 at session start.
2. A failed item from this session, if at least 3 steps have passed since it was missed.
3. An earlier lesson's item, as every 4th step: interleaving.
4. The next item in the current lesson.
5. Explanation of the next lesson, if the current one is done.
6. Nothing stored left: start a refill, keep reviewing.

Lesson skipping: 5 or more of 6 right, at confidence 4 or more, offers "skip ahead". This replaces much of what the prober did, by observing rather than testing first.

Starting values, all tunable. Only retrieval practice has verified evidence behind it; see [Research](/cheap-tutor/research.md).

| Setting | Start | Basis |
|---|---|---|
| Items per lesson | 6 | Benchmark size |
| Interleave rate | Every 4th step | Unverified studies; a judgment call |
| Review cap at start | 10 | A judgment call |
| Break suggestion | 25 minutes | Unverified; a judgment call |

## 5. Why this should teach better, not just cost less

- **Retrieval beats rereading at one week**, 61% against 40% in [Roediger & Karpicke 2006](/cheap-tutor/research.md). Every lesson is mostly retrieval.
- **Immediate feedback** comes from a stored `why`, with no wait for a model.
- **Latency falls to nothing.** In the benchmark, per-step calls took 6 to 22 seconds. A stored step shows at once, which matters over an hour.
- **Nothing is forgotten between sessions**, because items are FSRS cards.

## 6. Is it enough material?

Modelled at 60 steps. Probe answers in the real chats took about 15 to 30 seconds, so 60 items might last only 30 minutes, plus reading time. If so, the options, cheapest first, are: more interleaving from earlier lessons, more items per lesson, or a second pack, which stays inside the budget of 2 strong calls per hour. T9 measures this.

## 7. How we will know the learner learned

Content scores cannot answer this. The acceptance test is a person:

| Check | Pass |
|---|---|
| The owner studies Spanish for 60 minutes in one sitting | Never stopped by a limit |
| Usage shown by the meter | Under 100,000 tokens, at most 2 strong calls |
| Pre-test and post-test, 10 written items set from the plan before the session | Post-test clearly above pre-test |
| One week later, the same test with no study in between | Retains a clear majority of the gain |
| The owner's verdict | "I learned something and would do it again" |

One learner proves little scientifically, but it is exactly the risk that matters here: whether this project is worth keeping. The owner should say if they want a different test.
