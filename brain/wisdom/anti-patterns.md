---
type: guide
title: Banned Anti-Patterns
description: Strict list of banned architectural, concurrency, and domain anti-patterns in LearnPuppy.
tags: [wisdom, anti-patterns, rust, concurrency, architecture]
timestamp: 2026-10-27
---

# Banned Anti-Patterns

Back to [Wisdom Index](/wisdom/index.md). Related: [Storage & Memory Model](/architecture/storage-memory.md), [Core Domain](/architecture/core-domain.md).

1. **NEVER put IO, `tokio`, `rusqlite`, or `std::fs` inside `lp-core`**:
   - `lp-core` is 100% pure synchronous Rust. Every store or LLM interaction belongs behind a trait in `lp-store`, `lp-cli-driver`, or `lp-agent`.
2. **NEVER use `tokio::sync::mpsc::unbounded_channel`**:
   - Enforced by `clippy::disallowed_methods`. Always use bounded `mpsc::channel(cap)` so slow consumers apply backpressure rather than exhausting RAM.
3. **NEVER lock a `Mutex` on the 60fps Ratatui render path**:
   - The TUI render function (`Terminal::draw`) must read state via `ArcSwap::load()` (`lp-tui`).
4. **NEVER store `currentNodeId` or `doneNodeIds` in `PlanTree`**:
   - Plan node statuses (`Current`, `Visited`, `Upcoming`) are derived exclusively from `quiz` and `explain` tool **calls** via `derive_plan_statuses`.
5. **NEVER share tool names (`quiz` / `explain`) across `Probe` and `Teach` phases**:
   - `Probe` uses `probe` and `recall` (no `type` or `node_id` fields); `Teach` uses `quiz` and `explain` (`type` and `node_id` required). Sharing names causes models to omit `node_id` after `plan_init`.
6. **NEVER allow the Auditor to mint edges/chains or drop propositions**:
   - The Auditor emits **identity verdicts (`{ links }`) only**. Chains come solely from the digest-aware Extractor and are simplified deterministically by `simplify_chains`.
7. **NEVER open `~/.gemini/antigravity-cli/conversation_summaries.db` directly in place**:
   - Opening the live SQLite WAL database directly races with running `agy` instances. Always copy `.db`, `-wal`, and `-shm` to a `TempDir` snapshot first (`with_snapshot`).
8. **NEVER write to `stdout` or `stderr` while the TUI alternate screen is active**:
   - Route all `tracing` logs to `~/.learnpuppy/logs/learnpuppy.log` via non-blocking file appender.
