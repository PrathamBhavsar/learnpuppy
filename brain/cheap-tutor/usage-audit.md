---
type: spec
title: Usage Audit
description: Measured breakdown of where LearnPuppy's model usage goes today, per CLI and per learner step, with the code that causes it.
tags: [cost, audit, cli, overhead, measurement]
timestamp: 2026-11-03
---

# Usage Audit

Back to [Cheap Tutor Plan](/cheap-tutor/index.md). Related: [Benchmark](/cheap-tutor/benchmark.md), [Model Routing](/cheap-tutor/model-routing.md).

All numbers were measured on 2026-11-03 on this machine with real CLI calls, unless marked otherwise.

## 1. The shape of the problem

Today's tutor is a conversation. `crates/lp-agent/src/loops/mod.rs` documents it as "every step rebuilds the agent and its prompt from the transcript". Four facts combine:

1. **One model call per learner step.** `run_turn` calls the model for each question. A rejected tool call or a missing one costs another call, up to `MAX_STEPS = 16`.
2. **A new CLI process per call.** Each process loads that CLI's own system prompt and tool list, so the overhead below is paid every time.
3. **The whole transcript is re-sent.** `render_prompt` appends every message. Compaction only starts at `COMPACT_TRIGGER_TOKENS = 60_000` (`crates/lp-service/src/chat/runtime.rs`), and a single chat can therefore re-send tens of thousands of tokens per step.
4. **Grading and sequencing are model work.** The model decides what comes next and judges answers, even though most of that is simple logic.

## 2. Per-call overhead: the cost of a one-word reply

Prompt: `Reply with exactly: ok`. Input tokens as reported by each CLI.

| CLI and model | How it was run | Input tokens | Seconds |
|---|---|---|---|
| Codex `gpt-6.1-sol` low | production driver flags | 20,674 to 21,122 (about 12,400 cached) | 24 |
| Codex `gpt-6.1-sol` low | plus `model_instructions_file` with one line | 13,181 | not recorded |
| Opencode free models | direct, private HOME | about 5,200 to 13,200 (mostly cached) | not recorded |
| Antigravity `gemini-3.8-flash-medium` | production driver flags | 11,256 | 28 |
| Claude `claude-haiku-5-5` | production driver flags | 3,651 to 3,672 | 3 |
| Claude `claude-haiku-5-5` | plus `--system-prompt` with one line | 641 to 654 | 3 |

What this means:

- **On Codex and Antigravity, overhead dwarfs the lesson.** A question costs about 100 tokens to write and at least 11,000 to ask for.
- **Antigravity has no flag to shrink it.** `agy --help` shows no system prompt option. Per-step use on `agy` can never be cheap, whatever the model.
- **Claude can be cut by 82% with one flag.** The driver does not pass `--system-prompt` today.
- **Cached does not mean free, as far as we know.** How each subscription weighs cached tokens against its limit is not published, so this plan counts raw tokens and flags that gap.

## 3. Per-step growth

Ten real steps on the Spanish scenario, with a fixed-seed simulated learner.

| Approach | Model | Input, step 1 | Input, step 10 | Mean output per step |
|---|---|---|---|---|
| Today's prompt, full transcript | Haiku, system prompt cut | 1,675 | 4,321 | 825 |
| Today's prompt, full transcript | Flash low | 11,982 | 13,579 | 155 |
| Fixed-size state prompt | Haiku, system prompt cut | 878 | 1,006 | 610 |
| Fixed-size state prompt | Flash low | 11,414 | 11,500 | 118 |

Today's shape grows about 180 to 300 tokens per step, and that growth carries on until compaction at 60,000.

## 4. Why the Spanish chats stopped so early

Two chats are on disk: `chat_87f7e26a5ca0` stopped after 3 questions, `chat_217423420f81` after 4 questions and the probe handoff. The second one reached planning, then stopped at `plan_init`.

What is established:

- Every role had `"fallback": false` in `settings.json`, so a role never left its provider. A limit on one provider ended the chat.
- The teacher was pinned to `gemini-4-argon-high`, and the status memory already recorded `agy` quota as out until about 2026-11-03.
- Each call on the prober's Codex profile carries at least about 20,700 input tokens, from the one-word measurement above.

What is not established: how much of the limit these two chats used themselves, against how much earlier testing had already used. The plan does not depend on that answer, because the arithmetic in section 3 fails either way.

The error text "LearnPuppy already tried your other AIs" was also wrong under `fallback: false`. That is a separate fix in the [Task DAG](/cheap-tutor/task-dag.md).

## 5. Roles that do not spend per step

The extraction pipeline (Extractor, Auditor, Merger) is called from `crates/lp-service/src/digest.rs` and `crates/lp-service/src/ingest.rs`, not from the learning loop. It does not explain a chat dying after 4 questions. Its cost is real, but it runs on demand and is not part of this audit.

## 6. Driver problems found during the audit

| Finding | Evidence | Status |
|---|---|---|
| Opencode free models return `provider.free-limit` 429 through the driver, `retryAfterMs` about 34,790,000 (9.7 hours) | Every free model tried, through `bench_call` | Open |
| The same model answers when `OPENCODE_CONFIG_CONTENT` (the driver's lockdown) is absent | One direct run with and without it | Open, see below |
| Kiro through the driver did not answer within 120 seconds | One run; direct `kiro-cli` answered in 12 seconds | Open, needs a repro |

**About the opencode result.** It suggests the free tier's limit depends on how the client is configured, and only two configurations were compared. The lockdown was not bisected further and was not removed, for two reasons: varying the configuration to get around a provider's limit is not something to build a product on, and the lockdown is a security control that stops models using tools on untrusted prompts. For this plan, free opencode counts as unavailable. Whether to ask opencode about it, or pay for it, is the owner's call.
