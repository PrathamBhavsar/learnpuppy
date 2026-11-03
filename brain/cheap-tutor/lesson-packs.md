---
type: spec
title: Lesson Packs
description: The lesson pack schema, the Author prompt contract, deterministic quality gates, repair rules, answer matching and pack refills.
tags: [lesson-packs, schema, prompts, quality-gates]
timestamp: 2026-11-03
---

# Lesson Packs

Back to [Cheap Tutor Plan](/cheap-tutor/index.md). Related: [Architecture](/cheap-tutor/architecture.md), [Benchmark](/cheap-tutor/benchmark.md), [Memory System](/cheap-tutor/memory-system.md).

## 1. Schema

The Author returns JSON only. Code assigns ids and stores it.

```json
{
  "learner": "3 to 5 lines: level, goal, known gaps",
  "lessons": [{
    "id": "introducing-yourself",
    "title": "Introducing yourself",
    "objective": "Say your name and what you do",
    "explain": "Under 120 words, with one concrete example",
    "key_facts": ["me llamo = my name is", "me, not mi, goes before llamo"],
    "fixes": ["Wrote 'Mi llamo Alex'"],
    "items": [
      {"type": "mcq", "q": "...", "options": ["a","b","c","d"], "answer": "exact option text",
       "why": "One sentence", "tests": 1},
      {"type": "recall", "q": "...", "answer": "model answer", "accept": ["other right answers"],
       "why": "...", "tests": 0}
    ]
  }]
}
```

Fields added after the benchmark, for reasons it exposed:

| Field | Why |
|---|---|
| `key_facts` | Grounds follow-ups, and feeds the graph with no model call |
| `tests` | Index into `key_facts`. Lets a rule catch items testing something not taught, the most common flaw in every variant. |
| `fixes` | Forces probe mistakes to be covered. The judge often found them ignored. |
| `accept` | Lets code accept alternative right answers with no model call |

**These fields were not in the benchmarked prompt, so they will cost more output than measured.** Task T3 measures how much.

## 2. Author prompt contract

Input, all short: the goal in the learner's words, placement questions with answers, the profile, and the number of lessons.

Required rules, each answering a benchmark finding:

1. Order lessons by prerequisite, and start at the edge of what the learner knows.
2. Teach everything you test. Every item names the key fact it tests.
3. Put every placement mistake in some lesson's `fixes`, and test it there.
4. Four multiple-choice then two recall items per lesson. Options similar in length and all plausible.
5. Use the learner's other languages and context when they truly help, and state only comparisons you are sure of. Two judged errors were false claims comparing Hindi word order with Spanish.
6. Output JSON only.

Output is untrusted data until gated, and is stored only after it passes.

## 3. Gates

`gate_item` and `gate_course` in `lp-core`. Pure, no model call. A starting version is in `bench/cheap_tutor/run.py`, with a self-test.

| Rule | Level | Source |
|---|---|---|
| Valid JSON and schema | reject pack | Parse failures for Qwen and Flash |
| 4 distinct options; answer is one of them | drop item | Found in Flash output |
| All or none of the above | drop item | Standard item-writing rule |
| Answer much longer than other options | drop item | Found in Haiku and Flash output |
| Question contains the answer | drop item | Standard item-writing rule |
| `tests` index exists, and its key fact is in `explain` | warn | Tested without being taught |
| Every `must_fix` appears in some `fixes` | repair | Probe mistakes ignored |
| Duplicate question across the course | drop item | Judge's variety findings |
| Lesson left with under 3 items | repair | Keeps every lesson usable |

Answer order needs no rule. The judge saw correct options first in several variants, and `lp_core::plan::shuffle_options` already fixes that at display time.

## 4. Repair

One retry only, on the same model, with the failed rule list and the broken lessons. Never the full course.

If repair fails, it keeps the valid lessons and marks the rest for regeneration in the next call. A course with 8 good lessons is better than a failed course.

## 5. What gates cannot do

**Rules catch form, not truth.** Cheap skeleton + Qwen passed 16 of 16 items, and the judge scored it 1 out of 5 for accuracy. So content accuracy rests on three other things:

1. **Who writes it.** The strong model by default. Qwen is not allowed to write content.
2. **The learner.** Every item has a "this looks wrong" control. A flagged item is hidden at once, logged, and given to the next Review call.
3. **Sampled audit.** At most once a week, a strong call audits a random 20 items and reports wrong keys. Off by default until the flag rate justifies it.

## 6. Answer matching

`match_answer(item, text) -> Exact | Close | Wrong`, pure, so most typed answers need no model.

- Normalise: lowercase, trim, drop punctuation.
- Compare with `answer` and each `accept`. Accents are ignored at first, then flagged as "right, but note the accent".
- Edit distance within 15% of the length gives `Close`.
- `Close` and `Wrong` show the model answer, and the learner grades themselves: Again, Hard, Good or Easy.
- "Check my answer" is an optional button making one cheap call. Learners who never press it spend nothing.

**Why self-grading:** it is the standard flashcard method, and the comparison with the model answer is itself useful study. **Risk:** learners may grade themselves too kindly. The real-hour test checks this, and the fallback is automatic cheap grading of a batch at lesson end, which the benchmark measured at about 850 in and 550 out per call on Haiku.

## 7. Refills

- **When:** in the background when fewer than 2 unstudied lessons remain, so a learner never waits. The first course is the only visible wait: about 96 seconds in the benchmark.
- **With what:** the digest from [Memory System](/cheap-tutor/memory-system.md), and a list of titles already taught.
- **Size:** 10 lessons by default. Task T3 tests whether larger is safe.
- **Budget:** at most 2 strong calls per study hour, enforced in `lp-service`. Past that, it queues and the session continues on reviews.
- **If strong models are limited:** see the ladder in [Model Routing](/cheap-tutor/model-routing.md).
