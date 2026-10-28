---
type: spec
title: System Architecture & Crate Topology
description: Eight-crate hexagonal Rust workspace layout (including lp-ui and lp-app for Dioxus 0.7 macOS desktop), trait ports, compile-time safety rules, and curated crate stack.
tags: [architecture, workspace, cargo, traits, hexagonal, rust, dioxus]
timestamp: 2026-10-28
---

# System Architecture & Crate Topology

Back to [Architecture Index](/architecture/index.md) | [Brain Index](/index.md). Related: [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [ADR-0005](/decisions/0005-dioxus-desktop-over-tui.md).

## 1. Eight-Crate Workspace Layout

LearnPuppy enforces a strict inward-pointing dependency DAG across eight crates under `crates/`:

| Crate | Layer | Allowed Workspace Deps | Responsibility |
|---|---|---|---|
| `lp-core` | Domain Core | *None* (Zero IO, zero async) | Pure contracts, `PlanTree` & Mermaid parser, `phase_at_3`, `derive_plan_statuses`, `merge_findings`, `simplify_chains`, `question_core`, FSRS v5 math, 20k compaction cutfinder, port traits. |
| `lp-store` | Persistence | `lp-core` | Append-only JSONL ledgers (`nodes.jsonl`, `chains.jsonl`, `coverage.jsonl`, `sessions.jsonl`, `sync-<device>.jsonl`), atomic JSON chat/job files, and SQLite WAL (`index.db`) with FTS5. |
| `lp-cli-driver` | LLM & Auth Adapter | `lp-core` | Auto-discovery of `claude` (1..N accounts), `codex`, `agy`, and optional BYOK keys; `AccountPool` quota scheduler; `LlmEngine` trait; `FauxEngine` test double. |
| `lp-agent` | Agent Runtime | `lp-core`, `lp-store`, `lp-cli-driver` | Stateless per-turn `ProberAgent`, `TeacherAgent`, `PracticeAgent`; async sub-agents (`research`, `pdf`, `visual`, `demo`); Wake pump; 3-stage `Extractor -> Auditor -> Merger`. |
| `lp-ingest` | Transcript & Repo Learning | `lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent` | Incremental ingestion of Claude/Codex/AGY local transcripts + Git diffs; architectural decision distillation; 4-choice FSRS card generation; Knowledge Graph seeding. |
| `lp-ui` | Visual Component Library | `dioxus` (`0.7`) | Pure-Rust `shadcn/ui` + `dioxus-primitives` inspired component library (`Button`, `Badge`, `Card`, `Dialog`, `Tabs`, `Tooltip`, `Progress`, `Slider`, `Switch`, `ScrollArea`, `SplitPane`, `CommandPalette`, `CodeBlock`, SVG icons, OKLCH dark theme). Zero `wasm-bindgen` or inline `<script>` hacks. |
| `lp-app` (bin: `learnpuppy-app`) | Primary Visual Desktop App | `lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest`, `lp-ui` | Native macOS Dioxus 0.7 visual desktop app (`WKWebView`) + `LP_E2E_PORT` web harness for `tester-army/e2e`. Interactive SVG Knowledge Graph Canvas, Classroom split view, Plan DAG viewport, FSRS Digest, and Accounts modal. |
| `lp-tui` (bin: `learnpuppy`) | Headless CLI & Legacy TUI | `lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest` | Scriptable CLI subcommands (`auth`, `chat`, `graph`, `ingest`, `review`) + legacy terminal UI preserved on branch `legacy/tui-cli`. |

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
