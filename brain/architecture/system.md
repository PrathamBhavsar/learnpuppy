---
type: spec
title: System Architecture & Crate Topology
description: Six-crate hexagonal Rust workspace layout, trait ports, compile-time safety rules, and curated crate stack.
tags: [architecture, workspace, cargo, traits, hexagonal, rust]
timestamp: 2026-10-27
---

# System Architecture & Crate Topology

Back to [Architecture Index](/architecture/index.md) | [Brain Index](/index.md). Related: [ADR-0001 Rust TUI over Electron](/decisions/0001-rust-tui-over-electron.md).

## 1. Six-Crate Workspace Layout

LearnPuppy enforces a strict inward-pointing dependency DAG across six crates under `crates/`:

| Crate | Layer | Allowed Workspace Deps | Responsibility |
|---|---|---|---|
| `lp-core` | Domain Core | *None* (Zero IO, zero async) | Pure contracts, `PlanTree` & Mermaid parser, `phase_at_3`, `derive_plan_statuses`, `merge_findings`, `simplify_chains`, `question_core`, FSRS v5 math, 20k compaction cutfinder, port traits. |
| `lp-store` | Persistence | `lp-core` | Append-only JSONL ledgers (`nodes.jsonl`, `chains.jsonl`, `coverage.jsonl`, `sessions.jsonl`, `sync-<device>.jsonl`), atomic JSON chat/job files, and SQLite WAL (`index.db`) with FTS5. |
| `lp-cli-driver` | LLM & Auth Adapter | `lp-core` | Auto-discovery of `claude` (1..N accounts), `codex`, `agy`, and optional BYOK keys; `AccountPool` quota scheduler; `LlmEngine` trait; `FauxEngine` test double. |
| `lp-agent` | Agent Runtime | `lp-core`, `lp-store`, `lp-cli-driver` | Stateless per-turn `ProberAgent`, `TeacherAgent`, `PracticeAgent`; async sub-agents (`research`, `pdf`, `visual`, `demo`); Wake pump; 3-stage `Extractor -> Auditor -> Merger`. |
| `lp-ingest` | Transcript & Repo Learning | `lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent` | Incremental ingestion of Claude/Codex/AGY local transcripts + Git diffs; architectural decision distillation; 4-choice FSRS card generation; Knowledge Graph seeding. |
| `lp-tui` (bin: `learnpuppy`) | UI & CLI Composition Root | All workspace crates | 60fps `ratatui` + `crossterm` terminal UI and `clap` headless CLI commands (`auth`, `chat`, `graph`, `ingest`, `review`). |

## 2. Core Port Traits

Every boundary between `lp-core`/`lp-agent` and IO adapters is governed by traits so every pipeline can be tested deterministically with `FauxEngine` and `TempDir`:
- **`LlmEngine`** ([Multi-Account CLI Pool](/architecture/cli-auth-pool.md)): `stream_turn` (streaming text/thinking/tool calls) + `complete_structured<T: DeserializeOwned + JsonSchema>`.
- **`ChatStore`**, **`GraphStore`**, **`JobStore`**, **`TrackerStore`** ([Storage & Memory Model](/architecture/storage-memory.md)): Sync/async persistence ports with atomic write-ahead guarantees.

## 3. Workspace Lints (`Cargo.toml`)

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
unused_must_use = "deny"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
disallowed_methods = "deny" # Bans tokio::sync::mpsc::unbounded_channel and std::env::set_var
```
