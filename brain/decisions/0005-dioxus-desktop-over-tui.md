---
type: adr
title: "ADR-0005: Pivot Primary UI to Native Dioxus 0.7 + lp-ui (Isolate TUI on legacy/tui-cli)"
description: Architectural decision to make Dioxus 0.7 + custom shadcn-style lp-ui the primary visual learning desktop app (macOS first, Windows next, mobile last) while preserving the Ratatui CLI/TUI on legacy/tui-cli.
tags: [adr, dioxus, lp-ui, desktop, macos, architecture, ui, superseded]
status: superseded
timestamp: 2026-10-28
---

# ADR-0005: Pivot Primary UI to Native Dioxus 0.7 + `lp-ui` (Isolate TUI on `legacy/tui-cli`)

Back to [Decisions Index](/decisions/index.md) | [Brain Index](/index.md). Related: [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [ADR-0001](/decisions/0001-rust-tui-over-electron.md).

## Status
**Superseded** (2026-10-31) by [ADR-0006: Local Web UI Over Dioxus Desktop](/decisions/0006-web-ui-over-dioxus.md); `lp-ui` and `lp-app` were deleted from the workspace. Historical record follows.

Originally **Accepted** (2026-10-28) — Supersedes the UI layer of [ADR-0001](/decisions/0001-rust-tui-over-electron.md) while retaining 100% of the Rust backend crates (`lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest`).

## Context
1. Visual learning requires clickable icons, an interactive zoomable Knowledge Graph canvas, visual Mermaid/DAG node rings, and effortless mouse + keyboard ergonomics. A terminal-only view (`ratatui`) sacrifices quick icons and visual spatial memory.
2. We evaluated `DioxusLabs/components` (`dioxus-primitives`) and `rust-ui/dioxus-ui`. While `rust-ui/dioxus-ui` has good OKLCH Tailwind tokens, several of its components rely on `wasm-bindgen`/`web-sys` and inline `<script>` DOM polling that break native macOS desktop builds.
3. The user wants **100% Rust** as the portfolio highlight, targeting **macOS first**, **Windows second**, and **iOS/Android mobile last**, while preserving the existing CLI/TUI work in an isolated git branch (`legacy/tui-cli`).

## Decision
1. **Isolate Legacy CLI/TUI**: Branch `legacy/tui-cli` preserves the complete Ratatui TUI + CLI implementation.
2. **Build `crates/lp-ui` (Pure-Rust shadcn/Radix Component Library for Dioxus 0.7)**:
   - Own our component source code directly in `crates/lp-ui`, combining `dioxus-primitives` signal/ARIA patterns with `shadcn/ui` OKLCH dark-mode styling and inline Lucide SVG icons (`icons.rs`).
   - Strictly ban `wasm-bindgen`/`web-sys` and inline `<script>` DOM polling so `lp-ui` compiles natively on macOS (`aarch64-apple-darwin`), Windows, and mobile.
3. **Build `crates/lp-app` (Native macOS Dioxus 0.7 Visual Desktop App + E2E Web Harness)**:
   - Launches a native macOS `WKWebView` desktop window by default (`dioxus::desktop`) and supports a local web server mode (`LP_E2E_PORT`) so `tester-army/e2e` (`@e2e-dev/web`) can run automated Playwright + AI agent E2E tests against every screen.
4. **Phased Platform Rollout**:
   - **Phase D (Now)**: macOS Desktop (`lp-ui` + `lp-app`) verified with `cargo test`, `tester-army/e2e`, and `alibaba/open-code-review` (`ocr`).
   - **Phase W (Next — `TODO`)**: Windows Desktop (`WebView2`) packaging and path normalization (`T70`).
   - **Phase M (Last — `TODO`)**: iOS & Android mobile companion (`dioxus` `mobile`) with LAN/file sync for FSRS card review and Graph browsing (`T80`).
