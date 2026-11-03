# Cheap Tutor Plan

Back to [Brain Index](/index.md).

## Goal

A learner can study for an hour without hitting a limit, and actually learn. If the product cannot do this, it does not survive.

| Target | Value | Benchmark result |
|---|---|---|
| Tokens for a 60-minute session | under 100k, at least 10x below today | about 11,500, against 670,000 to 1,042,000 for today's shape, projected |
| Strong-model calls per 60-minute session | at most 2 | 1 |
| Learner-visible steps needing no model call | at least 60% | 100%, unless the learner asks something |
| Teaching quality | no worse than today on the blind rubric | Above today on Spanish, on small samples |

## The plan in short

A course is data, not a conversation. One strong call writes an hour of lessons, code runs the session, and cheap models answer only when the learner asks. If every AI is limited, study continues. Read the limits in [Benchmark](/cheap-tutor/benchmark.md) first: this rests on small samples, and no person has studied with it yet. The decision is [ADR-0007](/decisions/0007-course-as-data.md), still proposed.

## Work order

1. Audit where usage goes, with measured numbers: [Usage Audit](/cheap-tutor/usage-audit.md).
2. Research prior art, delegated to a cheap worker: [Research](/cheap-tutor/research.md).
3. Benchmark competing approaches locally, then pick by evidence: [Benchmark](/cheap-tutor/benchmark.md).
4. Write the chosen design, only after the benchmark:
   - [Architecture](/cheap-tutor/architecture.md)
   - [Memory System](/cheap-tutor/memory-system.md)
   - [Lesson Packs](/cheap-tutor/lesson-packs.md)
   - [Model Routing](/cheap-tutor/model-routing.md)
   - [Hour-Long Session](/cheap-tutor/hour-session.md)
5. Break into tasks with acceptance checks: [Task DAG](/cheap-tutor/task-dag.md).

Nothing in this plan changes app behaviour until the task DAG is approved. The harness is in `bench/cheap_tutor/`.
