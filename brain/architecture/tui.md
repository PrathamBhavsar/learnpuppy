---
type: spec
title: 60fps Ratatui TUI & Headless CLI (lp-tui)
description: Terminal UI layout, 4 primary workspaces, Braille force-directed Graph Canvas, 3-level Chain Table, Question Dock, and scriptable CLI commands.
tags: [tui, ratatui, crossterm, cli, braille, canvas, ux, archived]
status: superseded
timestamp: 2026-10-27
---

# 60fps Ratatui TUI & Headless CLI (`lp-tui`) (Archived)

> **Archived (2026-10-31).** `lp-tui` (TUI and headless CLI; history on branch `legacy/tui-cli`) was deleted from the workspace by [ADR-0006](/decisions/0006-web-ui-over-dioxus.md). This page is kept as history only; do not follow it for new work.
Its reusable logic (settings, engine routing, session helpers, review) now lives in `crates/lp-service`.

Back to [Architecture Index](/architecture/index.md). Related: [Storage & Memory Model](/architecture/storage-memory.md), [Classroom Engine](/architecture/classroom-engine.md).

## 1. Event Loop & Render Pipeline

- **60fps Render Tick (`16.6ms`)**: Driven by `tokio::select!` over `crossterm::event::EventStream`, a 60Hz render interval, and a bounded `mpsc::Receiver<AppEvent>` (`cap = 256`).
- **Zero-Lock Frame Rendering**: Every frame loads `Arc<GraphSnapshot>` and `Arc<ActiveSessionView>` via `ArcSwap::load()`, guaranteeing sub-millisecond draw calls even while background extraction or transcript ingestion is running.
- **Terminal Safety Guard**: Installs a panic hook before `enable_raw_mode()` / `EnterAlternateScreen` that always restores the terminal cursor and main screen before printing backtraces. All `tracing` logs write to `~/.learnpuppy/logs/learnpuppy.log` via `tracing_appender::non_blocking`.

## 2. Four Primary TUI Workspaces (`1..4` / `Tab`)

| Key | Workspace | Key Widgets & Controls |
|---|---|---|
| **`1`** | **Classroom (`Teach` & `Practice`)** | Collapsible Session Sidebar (`Ctrl+B`), Streaming Markdown + `syntect` Code + Unicode Math Transcript, Interactive **Question Dock** (shuffled options `A..D`, `I` = "I don't know", `1..5` Confidence Bar, dual `readingThinking` + `thinking` textareas, `Ctrl+H` Hint Rung request), Persistent **Plan DAG Split Pane** (`Ctrl+P`), and **Background Sub-Agents Dock**. |
| **`2`** | **Knowledge Graph & Chain Table** | Toggle `g`/`t`:<br>• **Braille Force-Directed Graph Canvas** (`ratatui::widgets::Canvas` with `Marker::Braille`, pan `hjkl`, zoom `+/-`, `/` fuzzy node search, `Space` select for practice, `Enter` provenance drawer).<br>• **3-Level Collapsible Chain Table** (`Cluster -> Premise A ∧ Premise B ⟹ Conclusion C -> Statements`, `s` to launch Practice Session on selection, `x` cascade delete with blast-radius preview). |
| **`3`** | **Tracker Digest & FSRS Review** | Toggle `d`/`r`:<br>• **CLI Decision Digest**: Browse ingested `claude` (all accounts), `codex`, and `agy` coding sessions and `DecisionRecord`s (`what`, `why`, `alternatives`, `risk`, `g` to fold into Knowledge Graph).<br>• **FSRS 4-Choice Quiz**: Answer due System Design & Rust cards (`1..4`), read rationale, and grade FSRS retention (`[1] Again [2] Hard [3] Good [4] Easy`). |
| **`4` / `Ctrl+A`** | **Accounts & Model Slots** | Live status of discovered `claude` accounts (`#1 pratham.bhavsar501@gmail.com`, `#2 q97159745@gmail.com`, etc.), `codex`, and `agy`; 5h/24h token telemetry; cooldown timers; role slot overrides (`prober`, `teacher`, `practice`, `extractor`, `auditor`, `distiller`). |

## 3. Scriptable Headless CLI Subcommands (`clap`)

For scripting, CI verification, and [`tester-army/e2e`](https://github.com/tester-army/e2e) testing:
```sh
learnpuppy                                  # Launch interactive 60fps TUI
learnpuppy auth status [--json]             # List all discovered CLI accounts & quota state
learnpuppy auth add-claude <config-dir>     # Register an extra CLAUDE_CONFIG_DIR profile
learnpuppy chat new "<prompt>" [--practice] # Create a session and run first turn
learnpuppy chat step <chat-id> "<reply>"    # Send reply/quiz answer headlessly
learnpuppy graph stats [--json]             # Output node, chain, and cluster counts
learnpuppy graph search "<query>"           # FTS5 search over propositions
learnpuppy graph import <path.md|path.pdf>  # Run Extractor -> Auditor -> Merger on file
learnpuppy ingest [--seed-rust-sysdesign]   # Ingest local CLI histories + git repos & generate cards
learnpuppy review [--limit 10]              # Interactive CLI FSRS review loop
```
