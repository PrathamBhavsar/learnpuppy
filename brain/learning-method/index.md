# Learning Method Plan

Back to [Brain Index](/index.md). Sources: [Alvar Method](/alvar-method/index.md), [Huberman Learning](/huberman-learning/index.md). Details: [Features](/learning-method/features.md), [Architecture and Folders](/learning-method/architecture.md), [Task DAG](/learning-method/task-dag.md).

**Status:** built through M13 and integrated (2026-11-10), including the course `why` (F02) and the strand probe (F06) at creation, see [ADR-0008](/decisions/0008-learning-method.md). Open and owner-only: M14, G4 and the T9/G2 baseline, runbook in [Real Hour](/learning-method/real-hour.md). G3 is done, see [G3 result](/learning-method/g3-result.md).

## Goal

The app, not the learner's discipline, makes study follow the method.

- **Alvar decides what happens in a session:** Review, Probe, Plan, Teach, Close, and when something counts as learned.
- **Huberman decides the conditions of a session:** when and how long to study, how to prepare, when to pause, how hard it should be, and what happens afterwards.
- **The cheap tutor decides what it may cost:** stored steps need no model, and study continues when every AI is limited.

## Starting point

The plan builds on the cheap tutor (ADR-0007, `brain/cheap-tutor/` on `staging`), not on Classroom chat. Classroom spends one model call per step, which is the cost problem ADR-0007 removes.

| Cheap tutor task | State on 2026-11-04 |
|---|---|
| T0 to T6: driver, core, store, author, service, routes, course screen | Merged to `staging` (`49977e3`) |
| FSRS cards, strong-call budget, usage route | Merged to `staging`, inside T4 |
| T9 and G2: real hour test, owner verdict | Not done |

T6 is in. Only T9 and G2 remain, and they gate just **G4** (did it teach better), not the start of this plan: M0 through M4 are pure `lp-core` schema and logic with no dependency on a real-hour result, so they start now. See [Task DAG](/learning-method/task-dag.md), where T9/G2 run independently of M0–M14 and feed only G4.

## Three design rules

1. **Enforcement lives in code, not prompts.** This is the Mathesis lesson in [Community Implementations](/alvar-method/implementations.md). `lp-core` decides the next step and rejects actions that break the rules. The UI cannot skip a gate, and TypeScript holds no method logic.
2. **Teaching material is written once, ahead of time.** Predictions, alternate explanations, misconception labels and rubrics are stored in the course, so most teaching moves cost no model call.
3. **Judgment gets a small, capped model budget.** Only judging free-text answers needs a model, and these calls are batched and use cheap models. Without a model, study continues, but nothing new is marked confirmed.

## Where the sources conflict

Both rows once marked "needs owner" now ship with the recommendation below as a tunable **setting**, not a blocking approval step: `make okf` and M0 do not wait on a sign-off. The owner can change either value at any time from settings; G4 asks whether they'd change the default.

| Conflict | Default shipped | Tunable? |
|---|---|---|
| Alvar never reveals an answer. Huberman says testing works as long as you see the correct answers. | No reveal during attempts. After 3 failed attempts the item is parked, its answer appears at Close, and it returns next day at the first rung. | Yes, `attempts_before_parking` |
| Alvar: self-report never confirms. Cheap tutor: self-grading saves model calls. | Self-grade still feeds FSRS, but alone it stops at `mc-passed`. A code-checked rubric match (see F15) confirms with no model call; only an unmatched self-grade stays pending until a grader is available. | No |
| Alvar: a long probe is a feature. Cheap tutor: teach within two minutes. | 3 strands, each binary-searched over 3 levels (at most 2 questions per strand, 6 total), one-line verdict each. Lesson 1 by about 3:00, the time already planned in `hour-session.md`. | No |
| Alvar: learner sets review dates. Cheap tutor: FSRS sets them. | FSRS proposes, learner may change a date. The first test is never more than 1 day out. | No |
| Huberman wants repeat attempts straight after an error. Runner waits 3 steps, and Alvar warns an immediate re-ask only tests memory of the sentence. | After an error, ask a different item on the same fact. The same item waits. | No |
| Huberman says enforce the 90-minute limit. A real deadline may need more. | Hard lock by default, with one logged override per day that needs a typed reason. | Yes, `bout_limit_minutes` |

## Evidence limits

- The Huberman notes disagree with each other: gap pauses are "10x" in one place and "20 to 30x" in another, and the comparison is deep sleep in one and REM in another.
- `brain/cheap-tutor/research.md` verified only retrieval practice. The gap rate, 15% error rate and 90-minute limit are unverified here.
- Therefore every number is a tunable setting, and the UI states what the app will do without presenting neuroscience as fact. That follows Alvar's own rule: an unverified claim is not taught as fact.
- The breathing protocol involves breath holds and is never required. See F03 in [Features](/learning-method/features.md).
- Gap pauses (F24) ship at a mild default — short, infrequent, and off entirely in one setting — since their numbers are the least verified of the set. See F24.

## MVP cut (v1)

To reach G4 sooner, v1 is: F05, F06, F07, F09 to F15, F17, F19, F23, F29, F30, F32, F35 — review, probe, plan, the full teach loop and mastery bar, the bout limit, and enough of Close to write confirmed facts to the graph and schedule the next review. Deferred past G4: F22 (teaching figures), F25 (focus guard), F28 (random reward), F33 (rest prompt), F36 (learning proof), and the `.ics` export half of F01 (the weekly-slot nudge itself ships; the calendar file does not). See [Features](/learning-method/features.md) for the full list and [Task DAG](/learning-method/task-dag.md) for which milestones carry which features.

## Budget

Estimates, except where marked measured. Gate G3 replaces them with measurements.

| Job | Per bout | Tokens |
|---|---|---|
| Placement | 1 | About 3,000 |
| Author, richer schema | 1 (strong call) | About 28,000 (today 14,000, measured) |
| Recall grading, one batch per lesson | 10 | About 14,000 (1,400 per call, measured for Haiku) |
| Feynman grading | 3 | About 4,000 |
| Note replies | 5 | About 8,000 |
| Fresh re-teach, when stored angles run out | 3 | About 5,000 |
| Fact-check, background job | 1 | About 20,000 |
| **Total** | **22 cheap + 1 strong** | **About 82,000, against a limit of 100,000** |

22 cheap calls (10 + 3 + 5 + 3 + 1 placement), not 20: M13's accept check is corrected to match (see Task DAG). Fact-check runs as an async background job, not inside the synchronous bout loop, so it does not count against the cheap-call cap, but its ~20,000 tokens **are** inside the 100,000 total — it is the single biggest line item and the one most likely to blow the budget on a slow lesson.

**The risk is provider overhead, not the prompts.** A one-word call costs about 20,700 input tokens on Codex and 11,300 on `agy`. Twenty-two cheap calls falling back to Codex would add about 455,000 tokens. So on any non-Claude fallback, grading drops to one batch at the end of the bout. Mitigated fallback: about 5 calls (placement, author, one grading batch, one Feynman batch, one note batch), roughly 5 x 20,700 = 104,000 overhead on Codex alone, which breaks the 100,000 limit. So non-Claude fallback is best effort: the budget is judged on the Claude path, and on fallback the app defers grading and notes to the next bout instead of calling.
