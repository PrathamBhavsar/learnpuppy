---
type: spec
title: Core Domain Contracts & Algorithms (lp-core)
description: Pure domain types, 3-phase session state machine, Mermaid plan_init DAG parser, position-from-calls derivation, question-core grading, and argument healing.
tags: [core, domain, mermaid, dag, phases, quiz, healing]
timestamp: 2026-10-27
---

# Core Domain Contracts & Algorithms (`lp-core`)

Back to [Architecture Index](/architecture/index.md). Related: [Classroom Engine](/architecture/classroom-engine.md), [Knowledge Graph](/architecture/knowledge-graph.md).

## 1. Event-Derived Session Phases (`phases.rs`)

Sessions (`ChatKind::Teach`) never store a mutable `phase` field. Instead, `phase_at_3(messages: &[LhMessage]) -> Phase3` folds the transcript deterministically:
1. **`Phase3::Probe`**: Default until a `probe_done` tool result with `ok == true` appears.
2. **`Phase3::Plan`**: After succeeded `probe_done`, until a `plan_init` tool result with `ok == true` appears.
3. **`Phase3::Teach`**: After succeeded `plan_init`.

## 2. Mermaid Subset `plan_init` Parser & Position Derivation (`plan_tree.rs`)

- **Input**: Mermaid `graph TD` / `flowchart TD` source block.
- **Node Syntax**: `slug["Title — 1-2 sentence summary"]` (em-dash `—` or `--` splits `title` and optional `summary`).
- **Edge Syntax**: `a --> b`, chained `a --> b --> c`, and ampersand fan-in/fan-out `a & b --> c & d`.
- **Validation**:
  - Rejects empty node sets, invalid slugs, undeclared edge endpoints, self-loops, and cycles (reports exact cycle path `a -> b -> c -> a` with 1-based line numbers).
- **Position-From-Calls Status Derivation (`derive_plan_statuses`)**:
  - Scans assistant tool calls for `quiz` and `explain` whose `node_id` is in `tree.nodes`.
  - Collapses consecutive identical `node_id`s into `runs: Vec<SmolStr>`.
  - `current = runs.last().cloned()`.
  - `visited = runs[..runs.len().saturating_sub(1)].iter().cloned().collect()`.
  - All other nodes in `tree.nodes` are `PlanNodeStatus::Upcoming`.

## 3. Question Core & Seeded Fisher-Yates Shuffling (`question_core.rs`)

- **Phase-Distinct Tool Schemas**:
  - Probe phase: `probe` (multiple-choice) and `recall` (free-recall) — **no `type` or `node_id` fields**.
  - Teach phase: `quiz` (multiple-choice) and `explain` (free-recall) — **require** `type: QuizRole` (`Check | Predict`) and `node_id: SmolStr` (validated against live `PlanTree`).
- **Deterministic Option Shuffle (`shuffle_options`)**:
  - Computes a 64-bit seed from `sha256(tool_call_id)` and runs Fisher-Yates over `options`, appending the synthetic `"I don't know"` option (`__idk__`) last.
- **Learner Answer Envelope**:
  - `{ selected: Vec<SmolStr>, confidence: Option<u8>, reading_thinking: Option<String>, thinking: Option<String> }`.

## 4. Generalized Argument Healing (`argument_heal.rs`)

Before schema validation on any tool call (`probe`, `quiz`, `plan_init`, `problem`, `conclude`, `submit`):
1. If an expected array/object field arrives as a JSON-encoded string (`"[\"... \"]"`), `heal_encoded_arguments` parses it in place.
2. Membership values (`correct_answer`, `node_id`, `target`) pass through `strip_model_value_debris` to strip leading colons (`:chain-rule`) or stray quotes.
