---
type: spec
title: Alvar Method Teaching Protocols
description: The mastery bar stop condition, graded quiz format, prediction-before-reveal, faded worked examples, interleaving, Feynman pass, spaced review ladder, misconception register, confidence tracking, no-reveal rule, and notes gate.
tags: [alvar-method, protocols, mastery-bar, quiz, interleaving, fsrs, spaced-repetition, misconception]
timestamp: 2026-11-04
---

# Alvar Method Teaching Protocols

Back to [Alvar Method Index](/alvar-method/index.md) | [Brain Index](/index.md). Related: [The Process](/alvar-method/process.md), [Visual & Verify](/alvar-method/visual-verify.md), [Skills Reference](/alvar-method/skills-reference.md).

> These protocols are the mechanics that do not change between topic mode and textbook mode. They are the stop condition, the quiz format, the teaching moves, and the spaced-review ledger.

## 1. The Mastery Bar

The stop condition for the entire session. A session ends when its items are confirmed, not when the learner says they understood.

### States
`open` → `taught` → `mc-passed` → `confirmed`

A miss at any state returns the item to `open`.

### Confirmation requires all three:
1. A correct graded multiple-choice answer (via the harness quiz UI).
2. A correct free-recall answer in the learner's own words (typed as ordinary chat).
3. The two on **separate turns**, with at least one intervening teaching step.

> One right answer never confirms anything. A content option is right by guessing among the real answers, and recognising a definition on a list is not the skill an exam tests. An immediate re-ask tests memory of the sentence just read.

### Free recall grading
- Grade on substance, not wording. Different words are fine. A vague answer that names no mechanism is a miss.
- For math/procedural items: free recall is a **new instance they solve** (change the numbers/matrix/function), not a definition they recite. Grade whether the steps are right and would transfer.

### Count line
Report the count at every quiz pass and at session end: "11 of 14 confirmed, 3 open", naming the open ones. Never call anything understood on any other basis.

## 2. Prediction Before Reveal

Open every conceptual teaching step with a prediction before explaining anything.

- One quiz question, plausible options, and where the step has a known misconception, that misconception is one of the options.
- A wrong prediction is the point. It is never graded, never scored, and never counted against the mastery bar.
- Log what the learner picked, then teach.
- Committing to a wrong answer and then seeing the right one beats reading the right one cold.
- Skip on purely definitional steps where a guess carries no information.

## 3. Confidence, Folded Into the Quiz

Pair every quiz question with "How sure are you?" — options *sure* / *think so* / *guessing* — in the **same** quiz call. No extra round trip.

- Record the pair on every quiz row: what they answered, how sure they were, and the verdict.
- At session end report: "answered sure and was wrong: 2 of 9", naming those items.
- High-confidence errors survive into exams. They are invisible without this.
- Any item in that set re-enters the review queue at the first rung.

## 4. Faded Worked Examples

For procedural or derivation material, replace the single worked example with three rungs:

1. Full worked example — every step shown.
2. Same-shape problem with middle steps blanked — the learner fills them in.
3. One they do alone.

Advance a rung only on success. A miss drops back a rung and is re-taught from a different angle. Conceptual material with no procedure keeps the single worked example.

## 5. Interleaving

Blocked practice (every question tests the step just taught) feels best while learning and transfers worst.

Two rules correct it:
- The session-start review pass is mixed across topics by construction.
- Every third quiz is a **mixed pass** — draw questions from earlier steps and earlier chapters, not the current step. Do not tell the learner which item each question is testing.

A mixed pass counts for the mastery bar exactly as any other quiz does.

## 6. The Feynman Pass

On load-bearing concepts only (DAG nodes with 2+ dependents, plus goal nodes):

1. After free recall passes, ask the learner to explain the concept to a ten-year-old.
2. Plain language, analogy or metaphor, no jargon.
3. Grade on whether the mechanism survives the translation. A fluent analogy that drops the mechanism is a miss — say that it is one, naming what got lost.
4. Put their wording in the log verbatim. It is theirs and it is what they reread.

## 7. Spaced Review

One ledger, every topic and every course, one row per confirmed item:

```
| id | topic-or-course | item | log basename | confirmed | next_review | history |
```

- `history` is a dated hit/miss string: `2026-08-17 ok, 2026-08-20 miss`.
- Store the log **basename**, never a path (logs get reorganised into folders).
- **The learner sets the dates, you propose them.** At session end, present newly confirmed items in one batched quiz with a proposed date from the default ladder: 1 day, then 3, 7, 14, 30 from the last successful recall.
- At session start, read the queue. Due items: say how many and which topics, then offer to run now, pick a subset, or skip. Skipping is a first-class answer.
- Quiz a due item by free recall first, MC second (recall tests more; a visible option list contaminates it).
- A hit advances a rung. A miss returns the item to `open`, re-teaches it this session, returns to first rung.

## 8. The Misconception Register

The moment you grade a miss, append a row to the learner profile:

```
| date | topic | what they believed | what is true | status |
```

- `status`: `open`, `retested-ok`, or `recurring` (missed again after a `retested-ok`).
- Write the row when you grade, not at session end — a session that dies loses the register otherwise.
- A `recurring` misconception gets a question in every session that touches its topic until it clears.

## 9. The Break Prompt

Two consecutive misses on the same step: offer a break instead of a third angle. One prompt, their decision, no insistence, no second offer.

> The incubation research behind "diffuse mode" is thin, so it is a prompt after a specific failure rather than a scheduled protocol.

## 10. Graded Quiz Format

The harness quiz UI has no `correctAnswer` and never marks right or wrong. Grading is the next message and the log.

### What the learner sees
- **4 written options**: 3 content claims + "I don't know" last.
- The widget always adds **Other** (free-text). That is 5 controls total.
- Other is not a content claim and not "I don't know" — it is the thought-process channel.

### Graded vs not graded
- **Graded:** probe, teaching quizzes, mixed passes, review multiple-choice.
- **Not graded:** predictions (§2), notes gates, plan confirmation, review-date prompts.

### "I don't know"
- Last written option, never shuffled with content options.
- Scored as a **gap**, not a miss: the item does not advance; no misconception row; not sure-and-wrong.
- Probe: a gap moves that strand more basic.
- Teaching: stay on the step, teach or re-teach, then quiz again.
- Review MC: ladder consequence same as a miss (return to `open`, first rung) without a misconception row.

### Other is thought process
- On a graded question: the learner types reasoning for context. Log it. Let it steer the follow-up. Not itself a scored pick.
- If the text names a content option, grade that pick. If it says they don't know, score a gap. If only reasoning with no committed pick, ask them to choose.

### Shuffle
- Permute the 3 content options before the call so the correct one is not in a stable slot.
- Keep "I don't know" last.
- Do not shuffle when order is the material (magnitudes, a sequence of steps).
- Confidence questions keep *sure* / *think so* / *guessing* in that order.

### No reveal in the popup
- Options are bare claims. No "because…" on the correct one.
- After the learner picks, grade in the next message.
- Probe: one line right, one line wrong; explanation waits for the lesson.
- Teaching: say correct or incorrect; a miss goes to re-teach, not "the answer was C because…".

### The no-reveal rule
On a teaching miss: say only that it is wrong, then re-teach from a different angle and re-quiz. Do not state the correct option, do not point at it, do not give a "hint" that is the answer in other words. Not after one miss, not after several. The learner reaches the answer by producing it themselves. The tutor's move on a miss is a new explanation, never the solution.

## 11. The Notes Gate ("Yap Note")

Before any quiz, open a notes gate: one quiz question with options like "Continue to quiz" / "All clear, skip ahead", where Other is the notes channel.

- The learner types their thinking, questions, or context there.
- Whatever they write gets a response at the current step's level.
- The full explanation goes into the session log under the step, same prose as the chat, not a one-line digest.
- A question that reveals a gap changes the path before the quiz.
- Never jump from teaching straight to a quiz.

## Session Log Format

- Append every step, quiz, and outcome as you go. A quiz entry carries the full question text, every option, the one picked, and the verdict.
- Write as markdown headings: `#### Question`, `#### Picked`, `#### Verdict`, `#### Notes gate`, `#### Prediction`, `#### Answer`.
- Never a packed table, never `**bold**` labels.
- No process notes, no tutor self-corrections, no "rule restated".
- The log is the lesson — it is what the learner re-reads later.
