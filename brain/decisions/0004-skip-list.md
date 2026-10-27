---
type: adr
title: "ADR-0004: Ponytail Skip List (Deliberate Non-Goals)"
description: Explicit non-goals skipped under Ponytail discipline to keep LearnPuppy lean, deterministic, and shippable.
tags: [adr, ponytail, skip-list, scope]
timestamp: 2026-10-27
status: accepted
---

# ADR-0004: Ponytail Skip List (Deliberate Non-Goals)

Back to [Decisions Index](/decisions/index.md). Related: [Overview](/overview.md), [Anti-Patterns](/wisdom/anti-patterns.md).

## Context
Under `/ponytail` discipline, every non-essential subsystem inherited from Electron desktop bloat must be explicitly cut so workers focus 100% on core pedagogy, multi-account CLI reliability, transcript ingestion, and 60fps TUI polish.

## Decision — Explicit Skip List
1. **Skip Electron, Next.js, and Local HTTP/SSE Servers**: The TUI communicates with `lp-agent` and `lp-store` in-process via bounded `tokio::sync::mpsc` channels and `ArcSwap`.
2. **Skip Browser Cookie Scrapers & Localhost OAuth Listeners**: All authentication comes from local CLI logins (`claude` 1..N, `codex`, `agy`) or optional API keys.
3. **Skip Voice Dictation (Deepgram / Local Whisper C++ binaries)**: Out of scope for v1 CLI/TUI.
4. **Skip Vector Embeddings & Louvain Graph Clustering**: Clusters derive deterministically from birth-session provenance (`birth_session_partition`, `MIN_CLUSTER_SIZE = 3`), matching upstream `learnharness`'s 2026-09 design.
5. **Skip Playwright / Headless Chromium**: `pdf-reader` uses pure-Rust PDF text extraction, `researcher` uses `reqwest` + `scraper`, and `demo-builder` / `visual-maker` render native TUI Braille/Unicode views plus optional standalone `.html`/`.svg` files.

## Consequences
- Eliminates all external binary runtime dependencies (Node, Chromium, Poppler, Whisper) while retaining 100% of the learning, graph, practice, and FSRS digest capabilities.
