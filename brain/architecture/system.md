---
type: spec
title: System Architecture & Crate Topology
description: Seven-crate hexagonal Rust workspace layout (domain, storage, CLI driver, agents, ingest, lp-service logic and the lp-server web binary), trait ports, compile-time safety rules, and curated crate stack.
tags: [architecture, workspace, cargo, traits, hexagonal, rust, web]
timestamp: 2026-10-28
---

# System Architecture & Crate Topology

Back to [Architecture Index](/architecture/index.md) | [Brain Index](/index.md). Related: [ADR-0006 Web UI Over Dioxus](/decisions/0006-web-ui-over-dioxus.md).

## 1. Seven-Crate Workspace Layout

LearnPuppy enforces a strict inward-pointing dependency DAG across seven crates under `crates/`:

| Crate | Layer | Allowed Workspace Deps | Responsibility |
|---|---|---|---|
| `lp-core` | Domain Core | *None* (Zero IO, zero async) | Pure contracts, `PlanTree` & Mermaid parser, `phase_at_3`, `derive_plan_statuses`, `merge_findings`, `simplify_chains`, `question_core`, FSRS v5 math, 20k compaction cutfinder, port traits. |
| `lp-store` | Persistence | `lp-core` | Append-only JSONL ledgers (`nodes.jsonl`, `chains.jsonl`, `coverage.jsonl`, `sessions.jsonl`, `sync-<device>.jsonl`), atomic JSON chat/job files, and SQLite WAL (`index.db`) with FTS5. |
| `lp-cli-driver` | LLM & Auth Adapter | `lp-core` | Auto-discovery of `claude` (1..N accounts), `codex`, `agy`, and optional BYOK keys; `AccountPool` quota scheduler; `LlmEngine` trait; `FauxEngine` test double. |
| `lp-agent` | Agent Runtime | `lp-core`, `lp-store`, `lp-cli-driver` | Stateless per-turn `ProberAgent`, `TeacherAgent`, `PracticeAgent`; async sub-agents (`research`, `pdf`, `visual`, `demo`); Wake pump; 3-stage `Extractor -> Auditor -> Merger`. |
| `lp-ingest` | Transcript & Repo Learning | `lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent` | Incremental ingestion of Claude/Codex/AGY local transcripts + Git diffs; architectural decision distillation; 4-choice FSRS card generation; Knowledge Graph seeding. |
| `lp-service` | Application Logic (no HTTP) | `lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest` | Settings/profiles, `CliPoolEngine` role routing, classroom turn loop and `ClassroomView`, FSRS review/sync/streak, digest, graph edits, accounts, ingest, data-root `paths`. Ported from the deleted `lp-tui`/`lp-app`. |
| `lp-server` (bin: `learnpuppy`) | Local Web Server | `lp-service` (+ the crates above) | `serve`, `migrate`, `seed`, `doctor`, `openapi`; axum + SSE + OpenAPI + embedded SPA on `127.0.0.1` (built out per `docs/web/08-task-dag.md`). |

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
