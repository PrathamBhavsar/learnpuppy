---
okf_version: "0.1"
---

# LearnPuppy Brain (`brain/`)

Cold-start entrypoint for orchestrators (`Claude Opus 4.6 Thinking High`) and implementation workers (`Claude Sonnet 4.6 High`, `Codex gpt-5.4 / gpt-6-luna High`, `Antigravity gemini-3.1-pro-high`). Read `[/overview.md](/overview.md)` and `[/environment.md](/environment.md)` first, then jump to your assigned task in `[/tasks/index.md](/tasks/index.md)`.

## Core Orientation
- [Project Overview & Product Invariants](/overview.md) — What LearnPuppy is, the 8-crate Rust workspace (`lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest`, `lp-ui`, `lp-app`, `lp-tui`), and the non-negotiable domain invariants.
- [Environment, Tooling & CLI Auth Facts](/environment.md) — Verified macOS arm64 paths, Rust toolchain, discovered CLI accounts (`claude` 1..N, `codex`, `agy`), installed `humanlayer/skills` + `tester-army/e2e` skills, and Orca worktree rules.
- [Chronological Project Log](/log.md) — Append-only timeline of reverse-engineering, planning, audits, and implementation milestones.

## Sections
- [Architecture Index](/architecture/index.md) — Crate boundaries, pure domain core, hybrid JSONL+SQLite storage, multi-account CLI pool, Classroom engine, Knowledge Graph pipeline, Practice + FSRS, Transcript Ingestor, **Dioxus 0.7 + `lp-ui` Visual Desktop UI**, and isolated Ratatui CLI/TUI.
- [Decisions (ADR) Index](/decisions/index.md) — Architectural decision records (`0001` through `0005`), including the Dioxus 0.7 + `lp-ui` visual desktop pivot and Ponytail skip list.
- [Wisdom & Guardrails Index](/wisdom/index.md) — Banned anti-patterns (including Dioxus `wasm-bindgen` / inline-JS traps), time-traps, and mandatory pre-merge verification gates (`cargo`, `tester-army/e2e`, `alibaba/open-code-review`).
- [Task DAG & Worker Dispatch (`T00`–`T80`)](/tasks/index.md) — Phase-wise task breakdown and exact model/effort matrix for Orca multi-agent orchestration.
