---
type: adr
title: "ADR-0001: Pure Rust CLI + Ratatui TUI Over Electron/Next.js"
description: Replace upstream LearnHarness's Electron + Next.js + Playwright runtime with a pure-Rust single binary using Ratatui and Crossterm.
tags: [adr, rust, ratatui, tui, electron, performance]
timestamp: 2026-10-27
status: accepted
---

# ADR-0001: Pure Rust CLI + Ratatui TUI Over Electron/Next.js

Back to [Decisions Index](/decisions/index.md). Related: [System Architecture](/architecture/system.md), [TUI Architecture](/architecture/tui.md).

## Context
Upstream `learnharness` (`v0.3.8`) packages an Electron shell around a standalone Next.js 16 HTTP/SSE server and spawns Playwright/Chromium subprocesses for diagrams and demos. Static analysis with `rea` showed ~450 MB of bundle overhead, slow cold-start latency, fragile browser OAuth/cookie bridges, and zero compatibility with local terminal workflows.

## Decision
Build **LearnPuppy** as a **100% pure-Rust single binary (`learnpuppy`)** providing both an interactive 60fps Terminal UI (`ratatui` + `crossterm`) and scriptable headless CLI subcommands (`clap`):
- Force-directed Knowledge Graph and Plan DAG render directly in the terminal via `ratatui::widgets::Canvas` with `Marker::Braille` (2x4 sub-pixel dots per terminal cell).
- Interactive demos run natively in a TUI parameter/curve explorer modal while also exporting standalone `.html`/`.svg` artifacts for optional browser viewing.

## Consequences
- **Pros**: <15 ms startup time, <35 MB RSS memory footprint, trivial deployment (`cargo install`), deterministic `TestBackend` snapshot testing (`insta`), and strong systems-engineering portfolio value.
- **Cons**: Requires custom Braille canvas layout and Unicode math normalization instead of browser DOM/KaTeX rendering (addressed in `lp-core` and `lp-tui`).
