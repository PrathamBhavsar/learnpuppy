---
type: spec
title: G3 Result, Richer Author Schema
description: Measured result of the learning-method Author prompt on three topics, two runs each, what it found wrong in the shipped code, and what was fixed.
tags: [g3, benchmark, author, schema, learning-method, measurement]
timestamp: 2026-11-10
---

# G3 Result, Richer Author Schema

Back to [Learning Method Plan](/learning-method/index.md). Related: [Task DAG](/learning-method/task-dag.md), [ADR-0008](/decisions/0008-learning-method.md), [Benchmark](/cheap-tutor/benchmark.md).

**Verdict: pass**, after two fixes the first run forced. Harness: `bench/cheap_tutor/round5.py`, which sends the shipped prompt (`crates/lp-agent/src/course/prompts/author.md`) unchanged, Opus at low effort, 10 lessons of 6 items each. Topics: Spanish and Rust (as in G1) and music theory (neither a language nor programming). Two runs each. Raw calls and outputs are in `bench/cheap_tutor/results/r5` (before the fixes) and `r5b` (after).

## What the first run found (r5, 6 calls)

1. **All 6 outputs failed to parse.** The prompt asks `"claims":["plain string"]`, but `lp_core::course::Claim` only accepted an object, so every real Author reply was an error (`invalid type: string ..., expected struct Claim`). Every earlier test fed the parser objects. Fixed: a `Claim` loads from a string or an object (`claims_load_as_plain_strings_or_full_objects`).
2. **3 to 5 alternate explanations per run stated a quiz answer outright**, which defeats a no-reveal re-teach (F13). Fixed twice: rule 7 in the prompt, and `gate_course` now drops any angle containing an answer of its lesson (`gate_drops_angles_that_state_an_answer`).

## After the fixes (r5b, 6 calls + 3 blind judge calls)

| Check | Result |
|---|---|
| Every run parses with the real Rust parser and gate | 6 of 6, 10 lessons and 60 items each, 0 lessons needing repair |
| Output tokens under about 28,000 | Pass: 17,026 to 27,335. The top is close to the limit; Rust run 1 used 27,335. |
| Angles that state an answer | Raw 0 to 2 per run (4 in total, down from 26); the gate drops them, so none reach a learner |
| Late lessons within 1 judge point of early | Pass: the largest early-to-late drop on any of accuracy, teaching, level and variety is 1.0 |
| Items the gate dropped | 0 to 1 per run |
| Cost | About $0.34 to $0.56 per course (API-equivalent). r5 about $2.33, r5b about $2.77, judge about $0.47. |

## What the judge still found (not blockers, worth fixing)

- **Answer keys that contradict their own rubric**: a recall item whose `accept` list includes an answer that lacks a `must_name` term (Rust: `cargo build -r` against `--release`; `translated to machine code` against `binary`). The code-only confirmation would reject a correct accepted answer. A gate check, every `accept` entry must contain the `must_name` terms or the rubric is dropped, would catch it.
- **Overstated or untaught claims late in a course**: Spanish "say no twice", Rust code that needs an unstated `let mut`. Rust run 2 scored 3 on accuracy in its late lessons. This is what the F08 fact-check job is for; G3 did not run it.
- **Small samples**: two runs per topic, one judge, one learner level. The pass is for this schema and prompt, not a quality guarantee.

## Decision

Keep the one-shot Opus-at-low-effort Author for the learning method. The Haiku enrichment fallback in the DAG was not needed. Re-run `round5.py` after any change to `author.md`.
