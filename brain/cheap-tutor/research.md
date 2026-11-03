---
type: reference
title: Research Notes
description: Prior art on cheap tutoring, learner models, memory, model cascades, pedagogy and item quality, with each claim marked verified or unverified.
tags: [research, prior-art, pedagogy, memory, cascades]
timestamp: 2026-11-03
---

# Research Notes

Back to [Cheap Tutor Plan](/cheap-tutor/index.md). Related: [Memory System](/cheap-tutor/memory-system.md), [Hour-Long Session](/cheap-tutor/hour-session.md).

## How to read this page

An Antigravity `gemini-3.8-flash-medium` worker wrote the first draft in read-only plan mode. Its raw output is kept at `bench/cheap_tutor/results/research-raw-unverified.md`. **Do not cite the raw file.** Spot-checking found these errors:

| Worker's claim | What the source says |
|---|---|
| arXiv 2404.14441 is a tutoring study by St-Hilaire et al. | That ID is a paper on contrail detection. The claim is dropped. |
| arXiv 2412.18619 is the Mem0 paper | That ID is a survey of next-token prediction. Mem0 is arXiv 2504.19413. |
| RouteLLM cut cost 85% | The abstract says "over 2 times in certain cases". 85% may come from elsewhere; it is not used here. |
| Retrieval gives "up to 400%" better recall | Roediger & Karpicke 2006 report 61% against 40% at one week. |
| Elo beats BKT with few observations (Pelánek) | A search found Elo work by Pelánek but nothing directly testing this claim. |

Given a 5-for-5 error rate on the claims checked, everything below is marked. **Verified** means checked against the primary source today. **Unverified** means worker-reported and not checked, so treat it as a lead and nothing more.

## 1. Learning products limit cost by not calling a model

- Unverified: Duolingo serves stored exercises chosen by its Birdbrain statistical model, and keeps live LLM use to premium features such as roleplay and explanations. The [Birdbrain post](https://blog.duolingo.com/learning-how-to-help-you-learn-introducing-birdbrain/) exists; its content was not read.
- Unverified: Math Academy runs on a skill graph and templates with no live LLM. The cited URL returned 404.
- Unverified: Khanmigo and Speak do call models live, and charge subscriptions to pay for it. Quizlet gated Q-Chat, then retired it.

**Use in this plan:** a direction, not evidence. The evidence for the direction is our own [Benchmark](/cheap-tutor/benchmark.md).

## 2. Learner models that need no LLM

- Verified that the work exists, not the comparison: Elo-style models have been applied to factual learning by [Pelánek et al.](https://www.fi.muni.cz/~xpelanek/publications/umuai-adaptive-practice.pdf) and [Řihák, Pelánek & Nižnan (EDM 2015)](https://www.educationaldatamining.org/EDM2015/proceedings/full109-116.pdf).
- Unverified: FSRS leads forgetting-curve benchmarks, and should run on default weights with little data.

**Use in this plan:** FSRS already exists in `crates/lp-core/src/fsrs.rs`, so it is what we use. Elo is skipped until simple per-lesson accuracy proves too blunt.

## 3. Memory for LLM agents

- Verified: [Mem0, arXiv 2504.19413](https://arxiv.org/abs/2504.19413) reports over 90% token saving and 91% lower p95 latency against a full-context approach on the LOCOMO benchmark, plus a 26% relative improvement over OpenAI on an LLM-as-judge metric. Caveats: vendor-authored, and it tests long conversations, not tutoring.
- Not found: any verified study of tutoring quality with structured state replacing a transcript. The one the worker cited does not exist at that ID.

**Use in this plan:** supports structured state over transcripts in general. Tutoring-specific proof has to come from our own hour-long test.

## 4. Cheap-first model cascades

- Verified: [FrugalGPT, arXiv 2305.05176](https://arxiv.org/abs/2305.05176) reports matching the best single LLM with up to 98% cost reduction, or 4% higher accuracy at the same cost.
- Verified: [RouteLLM, arXiv 2406.18665](https://arxiv.org/abs/2406.18665) reports cost reductions of over 2x in certain cases without losing quality.

**Use in this plan:** cascades work for question answering. Our benchmark found a different result for content writing: one strong call beat a cascade on total tokens, see [Benchmark](/cheap-tutor/benchmark.md).

## 5. Pedagogy for a long session

- Verified: [Roediger & Karpicke 2006](https://psychology.ecu.edu/wp-content/pv-uploads/sites/216/2019/03/Roediger-Karpicke-2006.pdf). At one week, repeatedly tested students recalled 61% and repeated readers 40%, even though the readers had read about 14 times against 3.4. At 5 minutes the pattern reversed, and restudy inflated confidence.
- Unverified: Hu & Nation 2000 on 98% word coverage for reading; interleaving studies by Rohrer & Taylor; 10 to 20% new material per session; attention falling after 20 to 30 minutes.

**Use in this plan:** retrieval practice is the backbone of the [Hour-Long Session](/cheap-tutor/hour-session.md). The unverified numbers appear there only as tunable starting values.

## 6. Rules that catch bad multiple-choice items

Unverified as a citation (Haladyna, Downing & Rodriguez 2002), but each rule can be checked on its own merits and costs no model call:

- The correct option is much longer than the others.
- "All of the above" or "none of the above".
- A word from the question appears only in the correct option.
- Options that are duplicates or overlap.

**Limit found in our benchmark:** the rule gates passed 16 of 16 items in a variant the judge scored 1 out of 5 for accuracy. Rules catch form, not truth. See [Lesson Packs](/cheap-tutor/lesson-packs.md).
