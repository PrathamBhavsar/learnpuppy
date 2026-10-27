---
okf_version: "0.1"
---

# LearnPuppy Brain (`brain/`)

Cold-start entrypoint for orchestrators (`Claude Opus High`) and implementation workers (`Claude Sonnet`, `Codex`, `Antigravity`). Read `[/overview.md](/overview.md)` and `[/environment.md](/environment.md)` first, then jump to your assigned task in `[/tasks/index.md](/tasks/index.md)`.

## Core Orientation
- [Project Overview & Product Invariants](/overview.md) — What LearnPuppy is, the 6-crate Rust workspace, and the non-negotiable domain invariants.
- [Environment, Tooling & CLI Auth Facts](/environment.md) — Verified macOS arm64 paths, Rust toolchain (`rustc 1.99`), discovered CLI accounts (`claude` 1..N, `codex`, `agy`), and Orca worktree rules.
- [Chronological Project Log](/log.md) — Append-only timeline of reverse-engineering, planning, audits, and implementation milestones.

## Sections
- [Architecture Index](/architecture/index.md) — Crate boundaries, pure domain core, hybrid JSONL+SQLite storage, lock-free `ArcSwap` memory model, multi-account CLI pool, Classroom engine, Knowledge Graph pipeline, Practice + FSRS, Transcript Ingestor, and Ratatui TUI.
- [Decisions (ADR) Index](/decisions/index.md) — Architectural decision records (`0001` through `0004`), including the Ponytail skip list.
- [Wisdom & Guardrails Index](/wisdom/index.md) — Anti-patterns, time-traps, and mandatory pre-merge verification gates (`cargo`, `tester-army/e2e`, `alibaba/open-code-review`).
- [Task DAG & Worker Dispatch (`T00`–`T60`)](/tasks/index.md) — Phase-wise task breakdown for Orca multi-agent orchestration.
