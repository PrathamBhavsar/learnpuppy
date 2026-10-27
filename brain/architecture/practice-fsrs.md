---
type: spec
title: Adaptive Practice Sessions & FSRS v5 Engine
description: Interactive PracticeAgent (problem/conclude tools, R0-R5 hint ladder, miss classification) combined with Tracker's deterministic FSRS v5 4-choice spaced-repetition engine.
tags: [practice, fsrs, flashcards, spaced-repetition, hints, quiz]
timestamp: 2026-10-27
---

# Adaptive Practice Sessions & FSRS v5 Engine

Back to [Architecture Index](/architecture/index.md). Related: [Core Domain](/architecture/core-domain.md), [Transcript Ingestor](/architecture/transcript-ingest.md).

## 1. Adaptive Practice Sessions (`ChatKind::Practice`)

Launched from the Chain Table selection bar (`s` key on selected propositions/chains) or via `learnpuppy chat new "<topic>" --practice`.

### 1.1 Turn-Scoped `PracticeGate` & Tool Pair
A single `PracticeGate { problem_active: bool }` seeded from `active_problem_of(&record.messages)` governs two tools:
1. **`problem` (Pose & Terminate Turn)**:
   - Input:
     - `statement: String` (Markdown + LaTeX, never announces the target proposition name).
     - `mode: PracticeDemandMode` (`Recall | Discrimination | Composition`).
     - `target: Option<Vec<SmolStr>>` (validated against the whole graph via `resolve_target_id`; empty array healed to `None` for untagged topic sessions).
     - `model_solution: String` (**committed before the learner sees the problem**; hidden from the UI until `conclude`).
   - Fails if `gate.problem_active == true`. Sets `gate.problem_active = true` and ends the turn.
2. **`conclude` (Grade, Reveal & Wait for Go-Ahead)**:
   - Input:
     - `outcome: ProblemOutcome` (`Solved | SolvedWithHints | MovedOn | Surrendered | Dismissed`).
     - `verdict: String` (plain-language localization of what worked or where reasoning broke).
     - `rung: Option<HintRung>` (`R0 | R1 | R2 | R3 | R4 | R5`).
     - `miss_class: Option<MissClass>` (`Cue | Discrimination | Execution | Careless | Structural`).
     - `retest: bool` (marks the target propositions for a fresh-surface retest before the session wraps).
   - Fails if `gate.problem_active == false`. Flips `gate.problem_active = false`, reveals the committed `model_solution` verbatim in the UI, and returns `CONCLUDE_ACK_TEXT` (instructing the agent to end its turn and wait for `[NEXT PROBLEM]`).

### 1.2 The `R0..R5` Minimum-Intervention Hint Ladder
When the learner submits partial thinking or presses `Ctrl+H` (Request Hint), `PracticeAgent` responds in prose using the lowest sufficient rung:
- `R0`: Restate the goal / point to the relevant part of the problem statement.
- `R1`: Localize the exact line or assumption where the learner's attempt broke.
- `R2`: Give a domain cue toward the missing proposition without stating it.
- `R3`: Name the target proposition (`GraphNode.name`).
- `R4`: Provide the setup equation or structural first step.
- `R5`: Work through the step completely (marks `retest = true` on `conclude`).

---

## 2. Deterministic FSRS v5 4-Choice Spaced-Repetition Engine (`lp-core::fsrs`)

Ported from `/Users/pratham/code/tracker/server/digest/fsrs.ts` and `cards.ts`:
- **Parameters**: `request_retention = 0.9`, `maximum_interval = 365`, `enable_fuzz = false`, `enable_short_term = true` (FSRS v5 19-weight vector).
- **Deterministic Replay**:
  - Card state (`State::New | Learning | Review | Relearning`, `due`, `stability`, `difficulty`, `reps`, `lapses`, `scheduled_days`, `elapsed_days`) is a pure fold over `fsrs_reviews` sorted by `(reviewed_at, id)`.
  - Appending a review (`Rating::Again | Hard | Good | Easy`) writes to both SQLite (`fsrs_reviews`) and `sync/sync-<device_id>.jsonl`, then updates the cached `fsrs_state` row.
- **Six Card Kinds Across Two Tracks**:
  - **Decision Track** (from CLI coding sessions): `Why`, `Predict`, `Bug`.
  - **Book / Code / Rust Track** (from books & repos): `Why`, `Trace`, `Ownership`, `Bug`, `Cloze`.
