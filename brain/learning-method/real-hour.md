---
type: guide
title: Real Hour Runbook (T9, G2, M14, G4)
description: How the owner runs the real study hour and bout that decide whether the method teaches better, what to record, and how each gate is read.
tags: [runbook, g4, m14, t9, real-engine, learning-method]
timestamp: 2026-11-10
---

# Real Hour Runbook (T9, G2, M14, G4)

Back to [Learning Method Plan](/learning-method/index.md). Related: [Task DAG](/learning-method/task-dag.md), [G3 result](/learning-method/g3-result.md), [Hour-Long Session](/cheap-tutor/hour-session.md).

This is the one part of the plan code cannot do: it needs you to study a real topic. Everything else is built, enforced by the server and tested. This page says what to run and what to write down so the verdict is evidence, not memory.

## Before you start: the machine check

```bash
cargo build -p lp-server
python3 scripts/real_smoke.py
```

`scripts/real_smoke.py` runs one whole course on the real CLIs against a throwaway data root: placement, the Author's strong call, the refusal of a step with no open bout, the ready gate, a walk through stored steps, Close, and the gap lock. It prints the tokens the walk itself caused (it should be zero strong tokens and no cheap tokens unless a lesson asks a question). It spends one strong call. If a check fails, fix that before spending an hour.

## T9: the baseline hour (cheap tutor, method settings relaxed)

Run this first if you want G4 to compare against something. In workspace settings, Study method, set `attempts_before_parking` to 10 and leave the rest. Study a topic you do not know for one hour. Then write down the numbers in the table below.

## M14: the real bout (method on, defaults)

1. Pick a different new topic of similar size. Write 10 short questions about it before you start, from a source you trust, and answer them cold: that is the pre-test.
2. Start the app (`make install`, then `learnpuppy serve`), create the course from your goal, answer placement.
3. Press Start lesson. Answer the sleep check-in, pass the ready gate (phone away, tabs closed, the 30 second focus point), study until Close.
4. At Close, do the brain dump first, then read the report.
5. Answer the same 10 questions from memory straight after: the post-test. Do the same 10 again a week later, without looking: the retention test.
6. Settings, Study method shows every number the bout used. Note any you changed.

## What to record

| Field | T9 baseline | M14 bout |
|---|---|---|
| Topic | | |
| Pre-test score (of 10) | | |
| Post-test score (of 10) | | |
| One-week retest (of 10) | | |
| Tokens on the meter (`GET /courses/{id}/usage`: strong and cheap, in and out) | | |
| Strong calls this hour | | |
| Cheap calls | | |
| Ever stopped by a provider limit? | | |
| Gates that felt pointless | | |
| "I learned more, and would do it again"? | | |

## Reading G4

| Check | Pass |
|---|---|
| Limits | Never stopped by a provider limit |
| Usage | Under 100,000 tokens, at most 2 strong calls and 22 cheap |
| Learning | Post-test gain and one-week retention beat T9 |
| Gates | No gate you found pointless. Each one you did gets cut or softened (change its setting, or remove it). |
| Verdict | "I learned more, and would do it again" |

One learner proves little, and two topics are not directly comparable, but the decision is about this owner. On a fail, read what failed before writing more code. A gate that annoys without helping is cut, however good its theory.
