---
type: spec
title: Model Routing
description: Which model does which job, the fallback ladders, escalation triggers, usage budgets and the driver changes that cut per-call overhead.
tags: [routing, models, fallback, budget, cli]
timestamp: 2026-11-03
---

# Model Routing

Back to [Cheap Tutor Plan](/cheap-tutor/index.md). Related: [Usage Audit](/cheap-tutor/usage-audit.md), [Benchmark](/cheap-tutor/benchmark.md), [CLI Auth Pool](/architecture/cli-auth-pool.md).

## 1. Jobs

Every job has a limit on how often it runs. Most steps of a session use no model at all.

| Job | Frequency | Default | Input | Output |
|---|---|---|---|---|
| Show explanation, ask question, grade multiple choice, schedule | Every step | **No model** | 0 | 0 |
| Match a typed answer | Every recall | **No model** | 0 | 0 |
| Placement | Once per course | `claude-haiku-5-5` | about 1,000 | about 2,000, not yet measured |
| Author | Once per course | `claude-opus-5-5` at low effort | about 1,150 | about 12,000 to 14,000 |
| Review and refill | About once per study hour | `claude-opus-5-5` at low effort | under 1,500, target | about 12,000 to 14,000, not yet measured |
| Learner follow-up | On request | `claude-haiku-5-5` | about 1,000 | about 600 |
| Check my answer | On request | `claude-haiku-5-5` | about 850 | about 550 |

**Planned usage for one study hour:** about 14,000 Opus tokens, about $0.25 to $0.28 at API list prices, plus about 16,000 Haiku tokens if the learner asks 10 questions. Against about 1,000,000 for today's shape on Flash low, in raw tokens. See section 9 of the [Benchmark](/cheap-tutor/benchmark.md).

## 2. Ladders

Every role falls back, always, as the owner decided. A ladder is ordered by the evidence in the [Benchmark](/cheap-tutor/benchmark.md). It moves on a limit or a failure, and the existing `AccountPool` cooldowns handle resets.

**Author and Review:**

1. `claude-opus-5-5` at low effort on each Claude account, with the system prompt cut
2. `claude-opus-5-5-high` via `agy` (adds about 11,000 input, and is still one call)
3. Codex `gpt-6-sol` high (adds about 21,000 input, not benchmarked for this job)
4. **Fallback writer, no strong model:** `claude-haiku-5-5` writes the outline, then each lesson. Measured accuracy 3 to 5, about 50,000 to 57,000 Haiku tokens, about $0.02 at API list prices.
5. `gemini-3.8-flash-medium` by the same two-stage path. Measured accuracy 3 to 4, about 250,000 tokens.
6. Nothing left: no error. The learner continues with stored lessons and due reviews, and refill retries when a limit resets.

Not used for writing content: Qwen (accuracy 1 to 3, see [Benchmark](/cheap-tutor/benchmark.md)). Free opencode models: unknown quality, currently limited through the driver. Sonnet: no saving measured, and later lessons fell away. It stays available if Opus proves too heavy on quota.

**Follow-up and Check my answer:**

1. `claude-haiku-5-5`
2. Kiro `qwen3-coder-next`, at 0.05x credits, only once the driver hang is fixed and only with the key facts in the packet
3. `gemini-3.8-flash-low`
4. Nothing left: "I can't answer questions right now, but you can keep studying." The question is saved and answered later.

## 3. When to escalate

The owner's rule: strong models run at the start or between large intervals, and everything needed is stored for what follows.

| Trigger | Action | Strong call? |
|---|---|---|
| New course | Author | Yes |
| Fewer than 2 unstudied lessons left | Review and refill | Yes |
| Lesson under 50% | Reinsert its items spaced apart, show the explanation again | No |
| Same lesson under 50% twice | One cheap call rewrites the explanation from its key facts | No |
| Still under 50% | Add to the digest, and the next refill reworks it | Waits |
| Learner redirects: "I want more business Spanish" | Saved to profile, and the next refill uses it | Waits, unless they ask for it now |
| More than 2 strong calls in a study hour | Refuse and queue | Blocked |

A failing learner never triggers a live strong call. That keeps the budget predictable, and the worst case is one waiting refill.

## 4. Budget

- **Enforced in `lp-service`,** not in a prompt: a counter by role and hour, from `usage` events in the log.
- **Visible to the learner:** a small meter in the session header, reporting tokens by tier. The UI never hides what a session cost. This also gives real weighting data over time.
- **Settings:** the existing `profiles` in `settings.json`, with new roles `author`, `placer` and `follow_up`, and `fallback: true` as the default for all roles.

## 5. Driver changes

| Change | Effect | Task |
|---|---|---|
| Optional `system` on `TurnRequest`, passed to Claude as `--system-prompt` | Overhead from about 3,650 to about 650 per Claude call | T0 |
| Same for Codex via `model_instructions_file` | Overhead from about 21,000 to about 13,200 | T0 |
| Find and fix the Kiro hang | Kiro becomes usable. Today, a fallback to Kiro may stall a turn. | T0 |
| Correct "already tried your other AIs" to name what was really allowed | Honest errors | T0 |

Both overhead changes need the prompt wired so tutor calls stay tool-less. The security isolation in `docs/web/12-cli-isolation-and-failures.md` stays unchanged.

## 6. What to do about settings today

The owner's `settings.json` was changed on 2026-11-03: `fallback: true` everywhere and free models on the per-step roles. Since then, the audit found free opencode limited through the driver for about 9.7 hours, and Kiro hanging through it in one run. A turn may therefore fall through, and could stall on Kiro, until T0 lands. A previous version is at `~/.learnpuppy/settings.json.bak-20261103`.
