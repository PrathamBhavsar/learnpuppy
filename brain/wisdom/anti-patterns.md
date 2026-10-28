---
type: guide
title: Banned Anti-Patterns
description: Strict list of banned architectural, Dioxus 0.7, concurrency, and domain anti-patterns in LearnPuppy.
tags: [wisdom, anti-patterns, rust, dioxus, concurrency, architecture]
timestamp: 2026-10-28
---

# Banned Anti-Patterns

Back to [Wisdom Index](/wisdom/index.md). Related: [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Storage & Memory Model](/architecture/storage-memory.md), [Core Domain](/architecture/core-domain.md).

1. **NEVER use inline `<script>` DOM polling (`dangerous_inner_html`) in `lp-ui` or `lp-app`**:
   - We inspected `rust-ui/dioxus-ui` and found it injects `<script>` blocks with `setTimeout(setupDialog, 50)` to attach click and Escape handlers outside Dioxus's VirtualDOM. This breaks on re-renders and leaks listeners. All interactivity in `lp-ui` and `lp-app` MUST use pure Rust `Signal<T>` (`use_signal`, `use_memo`, `use_context`) and native Dioxus event handlers (`onclick`, `onkeydown`, `onwheel`).
2. **NEVER import `wasm_bindgen`, `web_sys`, `js_sys`, or `gloo_timers` in `lp-ui` or `lp-app`**:
   - Those crates only compile on `wasm32-unknown-unknown` and break native macOS desktop (`aarch64-apple-darwin`) compilation. Use `dioxus::prelude::*` events and `tokio::time`.
3. **NEVER allow `<form onsubmit>` without `event.prevent_default()` in Dioxus 0.7**:
   - Unhandled form submissions cause macOS `WKWebView` to perform a page navigation reload, wiping application state.
4. **NEVER allow root window scrollbars in `lp-app`**:
   - The root container must use `h-screen w-screen overflow-hidden select-none flex flex-col bg-background text-foreground`. Constrain scrolling to internal `ScrollArea` containers.
5. **NEVER block the Dioxus main thread with synchronous SQLite or subprocess IO**:
   - Run blocking `rusqlite` or `std::fs` operations inside `tokio::task::spawn_blocking` and async `LlmEngine` turns inside `spawn(async move { ... })`.
6. **NEVER put IO, `tokio`, `rusqlite`, or `std::fs` inside `lp-core`**:
   - `lp-core` is 100% pure synchronous Rust. Every store or LLM interaction belongs behind a trait in `lp-store`, `lp-cli-driver`, or `lp-agent`.
7. **NEVER use `tokio::sync::mpsc::unbounded_channel`**:
   - Enforced by `clippy::disallowed_methods`. Always use bounded `mpsc::channel(cap)` so slow consumers apply backpressure rather than exhausting RAM.
8. **NEVER store `currentNodeId` or `doneNodeIds` in `PlanTree`**:
   - Plan node statuses (`Current`, `Visited`, `Upcoming`) are derived exclusively from `quiz` and `explain` tool **calls** via `derive_plan_statuses`.
9. **NEVER share tool names (`quiz` / `explain`) across `Probe` and `Teach` phases**:
   - `Probe` uses `probe` and `recall` (no `type` or `node_id` fields); `Teach` uses `quiz` and `explain` (`type` and `node_id` required).
10. **NEVER allow the Auditor to mint edges/chains or drop propositions**:
    - The Auditor emits **identity verdicts (`{ links }`) only**. Chains come solely from the digest-aware Extractor and are simplified deterministically by `simplify_chains`.
11. **NEVER open `~/.gemini/antigravity-cli/conversation_summaries.db` directly in place**:
    - Always copy `.db`, `-wal`, and `-shm` to a `TempDir` snapshot first (`with_snapshot`).
