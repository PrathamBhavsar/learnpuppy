---
type: doc
title: LearnPuppy Overview & Core Invariants
description: High-level summary of LearnPuppy — pure-Rust Dioxus 0.7 + lp-ui visual learning desktop app (macOS first, Windows next, mobile last) with isolated legacy CLI/TUI on legacy/tui-cli.
tags: [overview, learnpuppy, rust, dioxus, lp-ui, architecture, invariants]
timestamp: 2026-10-28
---

# LearnPuppy Overview

Back to [Brain Index](/index.md). Related: [System Architecture](/architecture/system.md), [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Task DAG](/tasks/index.md).

## 1. Product Mission

**LearnPuppy** (`learnpuppy`) is a production-grade, pure-Rust visual learning application built with **Dioxus 0.7** and our own **`lp-ui`** (`shadcn/ui` + `dioxus-primitives` inspired Rust component library).
- **Platform Order**: **macOS native desktop first** (active deliverable), **Windows desktop second** (`TODO` `T70`), and **iOS & Android mobile companion last** (`TODO` `T80`).
- **Branch Isolation**: The completed Ratatui terminal UI and headless CLI are preserved in isolation on git branch `legacy/tui-cli` (and headless CLI subcommands remain available for scripting).

It unifies two systems in 100% Rust:
1. **`amosblomqvist/learnharness`**: Prober -> Teacher phase split, author-once Mermaid dependency DAG (`plan_init`), one-step-then-check pedagogy (`probe`/`recall` and `quiz`/`explain`), async "ack now, steer later" background sub-agents (`research`, `pdf`, `visual`, `demo`), global cross-session Knowledge Graph (`Extractor -> Verdicts-Only Auditor -> Deterministic Merger` with `simplify_chains`), and Adaptive Practice Sessions (`problem`/`conclude` with `R0..R5` hint ladder).
2. **`/Users/pratham/code/tracker` (`server/digest/`)**: Incremental ingestion of local CLI coding transcripts (`claude` across all subscription accounts, `codex`, `agy`) and local Git commits/diffs, architectural decision distillation (`what`, `why`, `basis`, `alternatives`, `risk`), and deterministic FSRS v5 4-choice spaced-repetition flashcards (`why`, `predict`, `bug`, `trace`, `ownership`, `cloze`).

## 2. Six Non-Negotiable Invariants

1. **Hexagonal Crate Law**: `lp-core` has **zero IO, zero async, and zero network/filesystem imports**. All persistence and LLM calls cross explicit traits defined in [System Architecture](/architecture/system.md) and [Core Domain](/architecture/core-domain.md).
2. **Zero-Config Multi-Account CLI Auth**: `lp-cli-driver` automatically detects local `claude`, `codex`, and `agy` logins. Every distinct Claude config directory (`~/.claude`, `~/.claude-accounts/*`, `~/.claude-work`, custom `CLAUDE_CONFIG_DIR`) is registered as an **independent Claude account** with separate rate-limit buckets, cooldown tracking, and automatic failover. See [Multi-Account CLI Pool](/architecture/cli-auth-pool.md) and [ADR-0002](/decisions/0002-cli-multi-account-pool.md).
3. **Position-Not-Completion Plan Tree**: `PlanTree` stores structure only (`nodes`, `edges`, `version`). Node status (`Current`, `Visited`, `Upcoming`) is derived at render time from `quiz`/`explain` tool **calls** (`derive_plan_statuses`), never from answer outcomes. See [Classroom & Practice Engine](/architecture/classroom-engine.md).
4. **Chains-Primary Knowledge Graph**: `nodes.jsonl` and `chains.jsonl` are the sole stored graph truth. Edges and implication junction nodes (`arg_...`) are derived at read time (`derive_graph_snapshot`). Only the 3-stage pipeline (`Extractor -> Auditor -> Merger`) writes to graph ledgers, running `simplify_chains` (prefer-fewest-premises + transitive ancestry-cover) inside the serialized audit-merge critical section. See [Knowledge Graph Pipeline](/architecture/knowledge-graph.md).
5. **Safe Live-DB Transcript Ingestion**: When reading Antigravity's `conversation_summaries.db` (or any live SQLite WAL database), `lp-ingest` **always copies `.db`, `-wal`, and `-shm` to a temp snapshot directory** before opening (`with_snapshot`), never locking the live CLI's database. See [Transcript & Git Ingestor](/architecture/transcript-ingest.md).
6. **Pure-Rust Dioxus 0.7 Desktop Reactivity (`lp-ui` + `lp-app`)**: All visual UI state (modals, tabs, command palette, interactive SVG Knowledge Graph camera, quiz cards) uses Dioxus `Signal<T>` and `Memo<T>`. `wasm-bindgen`/`web-sys` imports and inline `<script>` DOM polling are strictly banned so the codebase compiles natively to `aarch64-apple-darwin` (`WKWebView`). See [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md) and [ADR-0005](/decisions/0005-dioxus-desktop-over-tui.md).
