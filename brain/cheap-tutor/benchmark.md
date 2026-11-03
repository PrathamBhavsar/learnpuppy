---
type: spec
title: Benchmark
description: Method, raw results and limits of the local benchmark comparing tutoring approaches on usage and on blind-judged quality.
tags: [benchmark, cost, quality, evidence]
timestamp: 2026-11-03
---

# Benchmark

Back to [Cheap Tutor Plan](/cheap-tutor/index.md). Related: [Usage Audit](/cheap-tutor/usage-audit.md), [Lesson Packs](/cheap-tutor/lesson-packs.md).

## 1. The question

Which approach gets the most teaching out of the least model usage?

## 2. How to rerun it

```
cargo build -p lp-cli-driver --example bench_call
python3 bench/cheap_tutor/run.py selftest
python3 bench/cheap_tutor/run.py perstep --run r1
python3 bench/cheap_tutor/run.py packs   --run r1
python3 bench/cheap_tutor/run.py judge   --run r1
python3 bench/cheap_tutor/run.py report  --run r1
cd bench/cheap_tutor && python3 round2.py gen && python3 round2.py judge && python3 round2.py report
```

Every call is logged to `bench/cheap_tutor/results/<run>/calls.jsonl`, with its full text and usage. A call already logged is not repeated, so a rerun spends nothing twice.

## 3. Method

**Scenarios.** Two real learners: Spanish from the owner's own chat (fluent in English, Gujarati and Hindi, works with a client in Peru) and Rust from an older chat (knows Python well). Probe results are included as input.

**Session size.** One hour is modelled as 60 steps: 10 lessons of 6 items each. This is an assumption. Probe answers in the real chats took about 15 to 30 seconds, so a session may need more items; see section 8.

**Approaches.**

| Name | What the model does |
|---|---|
| Today | Real `shared.md` + `teacher.md` + full transcript, one call per step. 10 real steps, projected to 60. |
| State | One call per step with a fixed-size prompt: learner, plan, mastered, weak, last exchange. 10 real steps, projected. |
| Skeleton + cheap | A course outline, by a strong or a cheap model. A cheap model then writes each lesson, one call each. 3 real lessons, projected to 10. |
| One shot | One strong call writes every lesson. Round 1 ran 3 lessons; round 2 ran the full 10. |

**Models.** Strong: `claude-opus-5-5`, `claude-sonnet-5-5` at medium. Cheap: `claude-haiku-5-5` (default and low effort), `gemini-3.8-flash-low`, Kiro `qwen3-coder-next`. All Claude calls used a one-line `--system-prompt`, which production does not do yet. Kiro reports credits only, so its tokens are estimated as characters divided by 4.

**Quality.** Two checks. Rule gates in `gate_item`. And a blind judge: `claude-opus-5-5` scores shuffled, anonymised variants from 1 to 5 on accuracy, teaching, level and variety, and lists errors. It saw two lessons per variant.

## 4. Usage results: one hour

Projected from real calls. Spanish scenario. Recall answers are graded by a cheap call per lesson in the skeleton rows, as tested, and by code in the one-shot row.

| Approach | Strong in + out | Cheap in | Cheap out | Total tokens |
|---|---|---|---|---|
| Today, on Flash low | 0 | 1,032,996 | about 9,300 | about 1,042,000 |
| Today, on Haiku with prompt cut | 0 | 620,880 | about 49,500 | about 670,000 |
| State, on Flash low | 0 | 701,753 | about 7,100 | about 709,000 |
| State, on Haiku | 0 | 77,853 | about 36,600 | about 114,000 |
| Strong skeleton + Flash low | about 7,400 | 232,176 | about 6,800 | about 246,000 |
| Strong skeleton + Haiku | about 7,400 | 24,253 | about 36,700 | about 68,000 |
| Cheap skeleton + Haiku | 0 | 24,782 | about 40,000 | about 65,000 |
| **One shot, Opus, 10 lessons** | **906 + 10,546** | 0 | 0 | **11,452** |
| One shot, Sonnet medium, 10 lessons | 910 + 12,430 | 0 | 0 | 13,340 |

One-shot rows measure 10 real lessons. Learner follow-up questions are not in this table: at the State row's rate of about 1,000 in and 610 out, 10 follow-ups on Haiku would add about 16,000.

## 5. Quality results: blind judge

Accuracy / teaching / level / variety, each 1 to 5.

| Variant | Spanish | Rust |
|---|---|---|
| Today, per step, Haiku | 3 / 3 / 2 / 2 | not run |
| One shot Opus, 10 lessons, first two | 4 / 5 / 5 / 4 | not run |
| One shot Opus, 10 lessons, last two | 5 / 4 / 4 / 5 | not run |
| One shot Opus, 3 lessons | 5 / 4 / 5 / 5 | 5 / 4 / 5 / 5 |
| One shot Sonnet, first two | 5 / 3 / 4 / 4 | 5 / 4 / 3 / 4 |
| One shot Sonnet, last two | 5 / 3 / 3 / 3 | 3 / 3 / 2 / 3 |
| Strong skeleton + Haiku | 5 / 5 / 5 / 4, then 4 / 4 / 4 / 3 | 4 / 5 / 5 / 4, then 4 / 5 / 5 / 4 |
| Strong skeleton + Haiku low | 3 / 4 / 4 / 4 | 4 / 3 / 5 / 4 |
| Cheap skeleton + Haiku | 5 / 4 / 5 / 4 | 4 / 5 / 4 / 3 |
| Strong skeleton + Flash low | 4 / 4 / 4 / 3 | 4 / 5 / 4 / 4 |
| Cheap skeleton + Flash low | 4 / 3 / 5 / 3 | 3 / 4 / 3 / 4 |
| Strong skeleton + Qwen | 4 / 3 / 4 / 3 | 3 / 3 / 4 / 4 |
| Cheap skeleton + Qwen | 1 / 1 / 1 / 3 | 2 / 3 / 3 / 4 |

## 6. Reliability

- Round 1: 95 calls, none failed. Round 2: 11 calls, none failed.
- JSON that parsed, per 3 lessons: Qwen 1, 2 and 3 of 3 across runs. Flash 1 of 3 in one run and 3 of 3 in the others. Haiku 3 of 3 everywhere. Opus and Sonnet parsed every time.
- Rule gates passed 94 to 100% of items among those that parsed.
- Time per call: Haiku 12 seconds, Kiro 15, Flash low 22, Opus 64 (96 for the full 10 lessons).

## 7. What the evidence says

1. **One shot wins on total tokens, by a wide margin.** About 11,500 tokens for an hour, against about 65,000 for the best cheap path and about 1,000,000 for today's shape on Flash low.
2. **Its quality held to the end.** Opus lessons 9 and 10 scored as well as lessons 1 and 2, on one Spanish sample.
3. **Sonnet was not a saving.** It used more output than Opus on Spanish (12,430 against 10,546), and its later lessons fell away, most clearly on Rust.
4. **Haiku is a good fallback writer.** Accuracy of 4 to 5 with no Opus at all. It reports about 2,760 output tokens for about 2,460 visible characters, roughly 600 tokens' worth, so most of its output is probably hidden thinking. The cause was not confirmed.
5. **Low effort did not pay.** Haiku low used about the same tokens after its first call and scored no better. Not adopted.
6. **Qwen is not safe for content.** It scored 1 to 3 on accuracy in three of four runs, and passing the rule gates did not reveal it. Grading might be fine; that was only run once.
7. **Antigravity is ruled out per step.** Its 11,000-token overhead makes the input of every cheap-writer row about 10x the same row on Haiku.
8. **The most common flaw everywhere was testing what was not taught.** The judge raised it for almost every variant, including Opus. Probe mistakes were also often left uncorrected. Both are prompt problems, addressed in [Lesson Packs](/cheap-tutor/lesson-packs.md).

## 8. Limits of this evidence

Read the conclusions with these in mind:

- **Small samples.** One run per cell. Opus 10-lesson was run once, on Spanish only.
- **The judge is noisy.** The same Haiku anchor lessons scored 5/5/5/4 in round 1 and 4/4/4/3 in round 2. **Treat a difference of 1 as no difference.**
- **The judge may favour itself.** Opus judged Opus. Blind labels hide the variant, but not writing style.
- **The judge rates content, not learning.** Nothing here measures whether a person remembers anything. That needs the real-hour test in the [Task DAG](/cheap-tutor/task-dag.md).
- **Raw tokens are not quota.** Opus tokens surely weigh more than Haiku tokens, and subscriptions do not publish the weighting. The one-shot lead over Haiku is real only if an Opus token weighs under about 5.7 Haiku tokens (65,000 / 11,452). Today's shape loses under any plausible weighting.
- **Projections are linear.** Today's row assumes steady growth and ignores compaction and retries.
- **"Today" was run on cheap models for fairness of comparison.** The owner's settings used stronger models, so the real figure is likely worse, not better. The judged "today" sample is also short and truncated to 900 characters per step.
- **Free opencode models were not tested.** They were limited through the driver all day. See [Usage Audit](/cheap-tutor/usage-audit.md).
- **Wasted calls.** A race in the harness wrote the Opus skeleton 3 times per topic. The tables count one.
