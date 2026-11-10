---
okf_version: "0.1"
---

# LearnPuppy Brain (`brain/`)

Cold-start entrypoint for orchestrators (`Claude Opus 4.6 Thinking High`) and implementation workers (`Claude Sonnet 4.6 High`, `Codex gpt-5.4 / gpt-6-luna High`, `Antigravity gemini-3.1-pro-high`). Read `[/overview.md](/overview.md)` and `[/environment.md](/environment.md)` first, then jump to your assigned task in `[/tasks/index.md](/tasks/index.md)`.

## Core Orientation
- [Project Overview & Product Invariants](/overview.md) — What LearnPuppy is, the 7-crate Rust workspace (`lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest`, `lp-service`, `lp-server`) behind the local web UI, and the non-negotiable domain invariants.
- [Environment, Tooling & CLI Auth Facts](/environment.md) — Verified macOS arm64 paths, Rust toolchain, discovered CLI accounts (`claude` 1..N, `codex`, `agy`), installed `humanlayer/skills` + `tester-army/e2e` skills, and Orca worktree rules.
- [Chronological Project Log](/log.md) — Append-only timeline of reverse-engineering, planning, audits, and implementation milestones.

## Sections
- [Architecture Index](/architecture/index.md) — Crate boundaries, pure domain core, hybrid JSONL+SQLite storage, multi-account CLI pool, Classroom engine, Knowledge Graph pipeline, Practice + FSRS, Transcript Ingestor, and the archived Dioxus desktop and Ratatui TUI specs.
- [Decisions (ADR) Index](/decisions/index.md) — Architectural decision records (`0001` through `0006`), including the web UI over Dioxus decision (`0006`, superseding `0005`) and Ponytail skip list.
- [Wisdom & Guardrails Index](/wisdom/index.md) — Banned anti-patterns (including Dioxus `wasm-bindgen` / inline-JS traps), time-traps, and mandatory pre-merge verification gates (`cargo`, `tester-army/e2e`, `alibaba/open-code-review`).
- [Alvar Method Index](/alvar-method/index.md) — Eero Alvar's AI-powered one-to-one learning method (Probe → Plan → Teach → Close) from "How I Use AI to Learn Things", including philosophy, process, teaching protocols (mastery bar, graded quizzes, spaced review), visual/verify skills, skill reference, community implementations, and full video transcript.
- [Huberman Learning Index](/huberman-learning/index.md) — Andrew Huberman's science-based learning protocols from "Optimal Protocols for Studying & Learning", including neuroplasticity fundamentals, self-testing as the #1 study tool, study habits of top students, the 9-step Neuroplasticity Super-Protocol, circadian optimization, gap effects/interleaving/emotion, and transcript excerpts.
- [Gornik Learning Index](/gornik-learning/index.md) — Aleks Gornik's 4 practical AI methods from "How I use AI to learn engineering (anti brain rot studying)": visualize concepts with 3D models, filter resources with AI prompts, find the essential 20% to get unstuck, and use sub-agents to find the best resources — AI as a filter, not a replacement for thinking.
- [Learning Method Plan](/learning-method/index.md) — Plan, built and integrated (ADR-0008), to enforce the Alvar loop and Huberman protocols in code on top of the cheap course-as-data tutor: features, architecture, folder map, and task DAG.
- [Task DAG & Worker Dispatch](/tasks/index.md) — Active web UI rollout (`V00`–`A70`), completed backend tasks, archived Dioxus phase, and the model/effort matrix for Orca multi-agent orchestration.
- [Cheap Tutor Plan](/cheap-tutor/index.md) — Plan to run hour-long learning sessions at a fraction of today's usage: measured audit, benchmark, course-as-data design, memory system, routing and task DAG.
