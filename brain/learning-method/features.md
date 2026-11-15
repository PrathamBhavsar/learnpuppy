---
type: spec
title: Learning Method Features
description: Catalogue of features enforcing the Alvar loop and Huberman protocols, in session order, each with its source, enforcement level and model cost.
tags: [features, learning-method, enforcement, mastery-bar, bout, session]
timestamp: 2026-11-04
---

# Learning Method Features

Back to [Learning Method Plan](/learning-method/index.md). Related: [Architecture and Folders](/learning-method/architecture.md), [Task DAG](/learning-method/task-dag.md), [Teaching Protocols](/alvar-method/teaching-protocols.md), [Super-Protocol](/huberman-learning/neuroplasticity-super-protocol.md).

## Enforcement levels

| Level | Meaning |
|---|---|
| **Hard** | Server refuses the action. The UI has no way around it. |
| **Gate** | Must be passed in order, though it can be answered quickly. |
| **Nudge** | Prompted once. The learner decides. |
| **Track** | Measured and reported. |

Source: **A** is Alvar, **H** is Huberman. Model column: **0** means no model call.

## Before the bout

| ID | Feature | What it does | Source | Level | Model |
|---|---|---|---|---|---|
| F01 | Study schedule | Weekly slots, target 2 to 3 bouts a day on 5 days, first slot about 3 hours after waking. Tracks kept and missed slots. **v1 ships the slots and tracking; `.ics` export is deferred past G4.** | H | Nudge | 0 |
| F02 | Your why | One line on why this goal matters, required when creating a course. Shown at the ready gate. | H | Hard | 0 |
| F03 | Ready gate | Phone away, other tabs closed, then a 30 to 60 second focus-point timer. The breathing routine is optional, with a warning: never in water or while driving, and stop if dizzy. | H | Gate | 0 |
| F04 | Sleep check-in | First bout of the day asks how you slept. Poor sleep suggests a review-only day. Tracks the 80% target. Self-reported only. | H | Nudge | 0 |
| F05 | Review first | Due cards come before new material, mixed across topics. Free recall first, multiple choice second. Skip is allowed, logged, and moves them to tomorrow. Silent when nothing is due. | A, H | Gate | 0 |

## Probe

| ID | Feature | What it does | Source | Level | Model |
|---|---|---|---|---|---|
| F06 | Strand probe | One call writes 3 strands at 3 levels each. Code binary-searches each strand (at most 2 questions, to fix a position among 3 levels), at most 6 questions total. Labels each strand `known`, `edge`, `unknown` or `blocked`. Right while guessing counts as `edge`. No teaching and only a one-line verdict. | A | Gate | 1 cheap |

## Plan

| ID | Feature | What it does | Source | Level | Model |
|---|---|---|---|---|---|
| F07 | Plan map | Lessons declare dependencies. Code rejects cycles and a start in `unknown` with no ramp. The learner sees the graph and confirms, can drop or reorder lessons, and then it freezes. Teaching cannot start before this. | A | Hard | 1 strong, shared with Author |
| F08 | Fact-check ledger | The Author lists its checkable claims. A background job finds sources, then **code fetches each page and checks the quote is really there**, because a cheap worker has already invented citations in this project. Verdicts: confirmed, qualified, contradicted, unknown. Contradicted items hide at once, and unknown ones are labelled unverified. Never blocks study. | A | Hard | 1 cheap |

## Teach

| ID | Feature | What it does | Source | Level | Model |
|---|---|---|---|---|---|
| F09 | Predict first | Ungraded question before each conceptual explanation, with a known misconception as an option. Skipped on pure definitions. | A, H | Gate | 0 |
| F10 | One-step lessons | Each lesson is one DAG node and one reasoning step, opened with a short stored hook on why it matters. Sometimes adds a brief related aside. | A, H | Hard | 0 |
| F11 | Notes gate | Screen between explanation and quiz: type thoughts or questions, or press "All clear". Saved to the log. A typed question gets one cheap reply. | A | Gate | 0 or 1 cheap |
| F12 | Honest quiz | 3 claims, then "I don't know" last, plus a reasoning box and sure / think so / guessing in the same step. "I don't know" is a gap, not a miss. Options shuffled by code unless order is the material. No "because" in the question. | A | Hard | 0 |
| F13 | No-reveal re-teach | A miss says only that it is wrong, then shows a stored alternate explanation and asks another item. After 3 failed attempts the item is **parked**: it stops asking, its answer is shown at Close, and it returns next day at the first rung. A cheap call runs only when stored angles run out, before parking. See the reveal decision in the [Plan](/learning-method/index.md). | A | Hard | 0 or 1 cheap |
| F14 | Mastery bar | Tracked per key fact: `open`, `taught`, `mc-passed`, `confirmed`, `parked`. Confirmed needs a right multiple choice and right free recall, on separate turns with teaching between. A miss returns it to `open`. Shows "11 of 14 confirmed, 3 open, 1 parked". Next lesson requires every **non-parked** fact in this one to reach `mc-passed`; a parked fact is exempt and does not block advancement, so F13's park at 3 fails can never deadlock the lesson. | A, H | Hard | 0 |
| F15 | Recall grading | Exact match, then a stored rubric of what the answer must name (`must_name`), checked by code with no model call — a match confirms the fact directly. Only when the rubric does not match does it fall to one cheap batched call per lesson, then to a pending self-grade if no model is available. For procedures, recall is a new problem to solve. Rubric false negatives (a correct answer phrased differently) are accepted; the rubric must never produce a false positive. | A, H | Hard | 0 or 1 cheap |
| F16 | Faded examples | For procedures: full worked example, then middle steps blanked, then solo. A miss drops one rung. | A, H | Gate | 0 |
| F17 | Mixed pass | Every third quiz draws from earlier lessons, unlabelled. Counts for mastery. | A, H | Hard | 0 |
| F18 | Feynman pass | On lessons with 2 or more dependents and goal lessons: explain it to a ten-year-old. Graded on whether the mechanism survives. Wording is kept verbatim. | A, H | Gate | 1 cheap |
| F19 | Misconception register | Each wrong option stores the belief it implies, so code writes the row the moment you miss, with no model. Tracks `open`, `retested-ok` and `recurring`. A recurring one gets a question in every session on that topic. | A | Hard | 0 |
| F20 | Prerequisite insert | Repeated failure reveals a missing foundation, and a short prerequisite lesson is added to the frozen plan. | A | Gate | 1 cheap |
| F21 | Break prompt | Two straight misses on one step offers a break instead of a third angle, once, with no insistence. | A | Nudge | 0 |
| F22 | Teaching figures | For load-bearing lessons, the existing visual job draws one picture for one stated claim and must review its render. Off when AI is limited. | A | Track | 1 cheap, later |

## During the bout

| ID | Feature | What it does | Source | Level | Model |
|---|---|---|---|---|---|
| F23 | Bout limits | 90 active minutes, then Close. 2 hours before the next bout. 270 minutes a day. Enforced by the server, so a refresh or a new tab cannot reset them. Idle over 10 minutes moves the clock back 10 minutes. | H, A | Hard, one logged override a day | 0 |
| F24 | Gap pauses | Blank 10 second pause at random. **Default is mild: one per 5 active minutes, and a setting to turn it off entirely** — the "10x/20-30x" and sleep-stage numbers behind the original rate are unverified (see Evidence limits). Only between steps, never mid-answer. Cannot be skipped when on. Code picks the moment, so replay is deterministic. | H | Hard | 0 |
| F25 | Focus guard | Distraction-free mode. Counts each time the learner leaves the tab and reports it at Close. **Deferred past G4.** | H | Track | 0 |
| F26 | Difficulty control | Items carry a level. Over the last 20 answers, above about 25% errors moves easier, below about 8% moves harder or offers skip-ahead, aiming near 15%. | H, A | Hard | 0 |
| F27 | Retry after error | A miss is followed at once by a different item on the same fact, while attention is high. The copy treats errors as useful. | H | Hard | 0 |
| F28 | Random reward | Small, unpredictable celebration, more likely after a hard item or a retry. Never tied to spending time, and nothing is lost by stopping. **Deferred past G4.** | H | Track | 0 |

## Close and after

| ID | Feature | What it does | Source | Level | Model |
|---|---|---|---|---|---|
| F29 | Brain dump | Blank box first thing at Close: what were the key points? No notes visible. Code compares it with key facts and shows what was left out. | H | Gate | 0 |
| F30 | Close report | Takeaways, what to focus on, count line, "answered sure and was wrong: 2 of 9", active time, focus count, resume point. | A | Gate | 0 |
| F31 | Session log | Rereadable markdown written by code as you go, so a crash loses nothing. Saved per course. Fact-checks stay out of it. | A | Track | 0 |
| F32 | Review dates | One batched screen proposes dates from FSRS, and the learner may change them. Sure-and-wrong goes back to the first rung. First test within 1 day. | A, H | Gate | 0 |
| F33 | Rest prompt | After Close: a 10 or 20 minute rest timer within the hour, a sleep reminder, and a warning on bouts booked late in the evening. **Deferred past G4.** | H | Nudge | 0 |
| F34 | Learner profile | Dated entries for edges, outcomes and preferences, which supersede rather than overwrite. Editable, and feeds the capped packet. | A | Track | 0 |
| F35 | Skill levels | Per lesson, derived: unskilled, skilled, mastery. Only `confirmed` facts enter the Knowledge Graph. Mastery needs confirmed facts, Feynman where required, and a spaced recall pass. The code-checked rubric path in F15 is what keeps this from staying empty on a long AI-limited run: it confirms facts with zero model calls, so the graph still fills without any provider. | H, A | Hard | 0 |
| F36 | Learning proof | Optional pre-test, post-test and one-week retest using written items. Shows gain and retention per course. This is the app checking its own method. **Deferred past G4.** | H | Track | 0 |

## Later, once the core is proven

Deferred out of the [MVP cut](/learning-method/index.md), past G4:

| ID | Feature | Why later |
|---|---|---|
| F22 | Teaching figures | Depends on the visual job; not needed to prove the loop |
| F25 | Focus guard | Track-only; adds nothing the gates already enforce |
| F28 | Random reward | Track-only; motivational polish, not core to mastery |
| F33 | Rest prompt | Nudge-only; `make gate` priority is the bout clock, not the prompt after it |
| F36 | Learning proof | The owner's single bout in G4 already measures this; a general harness can wait |
| F01 (`.ics` half) | Calendar export | The in-app slot nudge ships; exporting it is separable |
| F37 | Textbook mode: map PDF pages, take chapter objectives, quiz every objective | Large, and builds on the PDF job and notes import |
| F38 | Pace bar, slightly faster than your own median | Could add stress, and is the weakest fit for a mastery tool |

## Deliberately left out

| Item | Why |
|---|---|
| Caffeine, cold exposure, sunlight, food, exercise | Health choices the app cannot see and should not police. At most a plain tips page. |
| Forced breathing | Safety. Optional only. |
| Study-alone enforcement, focus meditation | The app cannot observe either. |
| Anki export | FSRS is built in. |
| Streak-loss penalties, casino-style loot | Random reward should motivate study, not compulsion. |
| Adding the method to Classroom chat | Per-step calls would bring back the cost problem. Classroom stays as is. |
