---
type: spec
title: Classroom & Async Workbench Engine (lp-agent)
description: Stateless per-turn Prober and Teacher agents, probe_done handoff wake, 4 async sub-agents (research, pdf, visual, demo), 3-permit semaphore, and deterministic steer delivery.
tags: [agent, prober, teacher, workbench, subagents, wake, steer]
timestamp: 2026-10-27
---

# Classroom & Async Workbench Engine (`lp-agent`)

Back to [Architecture Index](/architecture/index.md). Related: [Core Domain](/architecture/core-domain.md), [Knowledge Graph](/architecture/knowledge-graph.md).

## 1. Stateless Per-Turn Agent Composition

Neither `ProberAgent` nor `TeacherAgent` persists in memory across turns. On every user reply or wake trigger:
1. Load `ChatRecord` from `ChatStore`.
2. Compute `phase = phase_at_3(&record.messages)`.
3. If `phase == Phase3::Probe`: build `ProberAgent` with `prompts/prober/*.md` + `prompts/shared/*.md` and register `{ probe | recall, ask_user_question, probe_done, read, ls, find, grep, research, pdf, [list_sessions, list_propositions] }`.
4. If `phase == Phase3::Plan | Phase3::Teach`: build `TeacherAgent` with `prompts/teacher/*.md` + `prompts/shared/*.md` and register `{ quiz | explain, ask_user_question, plan_init, read, ls, find, grep, research, demo, visual, pdf }`.

## 2. The `probe_done` Seam & Handoff Steer

- `probe_done` accepts `ProbeDoneInput { assessment: { strands: Vec<{ strand, assessment }>, summary: Option<String> } }`.
- Refused if an unanswered question is open or if `probe_done` already succeeded.
- Returns `PROBE_DONE_ACK_TEXT` to the model and queues a deterministic handoff steer (`id = format!("steer_probe_{chat_id}")`, content = `HANDOFF_TEXT`).
- Once the prober's turn ends, the Wake Pump fires the handoff steer, invoking `TeacherAgent` which reasons through the teaching arc and calls `plan_init`.

## 3. Async Workbench Sub-Agents ("Ack Now, Steer Later")

When `TeacherAgent` (or `ProberAgent`) calls `research`, `pdf`, `visual`, or `demo` mid-turn:
1. **Immediate Ack**: Creates a `JobRecord` (`status: Running, delivery: Pending`), spawns the background task on Tokio, and immediately returns `"dispatched — continue your turn"` so the teacher's turn never blocks.
2. **Process-Wide Concurrency Cap**: All background sub-agent LLM calls acquire a permit from a process-wide `tokio::sync::Semaphore` of **3 permits** (`BG_LLM_SEMAPHORE`).
3. **Four Sub-Agent Runners**:
   - `researcher`: Multi-step loop using web search + `reqwest`/`scraper` readability extraction -> `ResearchResult { brief, sources }`.
   - `pdf_reader`: Local PDF triage and page extraction via `pdf-extract`, tracking `pages_read` (full extract) vs `pages_consulted` (search hits) -> `PdfResult`.
   - `visual_maker`: Generates both Unicode/Braille terminal diagrams and verified SVG markup saved to `~/.learnpuppy/artifacts/<job_id>.svg` -> `VisualResult`.
   - `demo_builder`: Generates an interactive terminal parameter/curve spec (`TuiDemoSpec`) plus a standalone HTML file (`~/.learnpuppy/artifacts/<job_id>.html`) -> `DemoResult`.

## 4. Deterministic Wake Pipeline (`claim_steers`)

- Every completed or aborted job produces a `SteerMessage` with deterministic ID `steer_<job_id>`.
- **Idle Wake Gates**: The Wake Pump checks:
  1. No turn is currently running for `chat_id`.
  2. No unanswered question (`pending_question == false`) is open in the dock.
  3. The session has not been dismissed/paused by the learner.
- When all gates pass, `JobStore::claim_steers(chat_id)` atomically claims pending steers, appends the `LhMarkerMessage` (`custom_type: "subagent-steer" | "subagent-abort"`) to `record.messages`, and triggers a continuation turn.
