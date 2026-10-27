---
type: adr
title: "ADR-0003: Hybrid Append-Only JSONL + SQLite WAL Storage"
description: Combine append-only JSONL graph/sync ledgers with SQLite WAL + FTS5 indexing and ArcSwap lock-free memory snapshots.
tags: [adr, storage, sqlite, jsonl, fts5, arc-swap]
timestamp: 2026-10-27
status: accepted
---

# ADR-0003: Hybrid Append-Only JSONL + SQLite WAL Storage

Back to [Decisions Index](/decisions/index.md). Related: [Storage & Memory Model](/architecture/storage-memory.md), [Knowledge Graph](/architecture/knowledge-graph.md).

## Context
`learnharness` stores its Knowledge Graph in append-only JSONL files (`nodes.jsonl`, `chains.jsonl`, `coverage.jsonl`, `sessions.jsonl`) which makes provenance auditable and Git/Syncthing merges trivial, but requires linear scans for search. Conversely, `/Users/pratham/code/tracker` uses SQLite WAL for incremental transcript ingestion cursors, decisions, and flashcards, plus `sync-<device>.jsonl` for conflict-free multi-device replication.

## Decision
Adopt a **Hybrid Storage Engine (`lp-store`)**:
1. Keep `nodes.jsonl`, `chains.jsonl`, `coverage.jsonl`, `sessions.jsonl`, and `sync-<device>.jsonl` as append-only JSONL ledgers (the source of truth for the Knowledge Graph and multi-device FSRS sync).
2. Maintain `index.db` (SQLite WAL via `rusqlite` bundled) for FTS5 full-text search (`propositions_fts`, `transcripts_fts`), `ingest_cursors`, `coding_sessions`, `decisions`, `flashcards`, `fsrs_reviews`, and `account_usage_ledger`.
3. Hold the derived `GraphSnapshot` in an `ArcSwap<GraphSnapshot>` in memory so the 60fps TUI render loop reads wait-free with zero mutex contention.

## Consequences
- **Pros**: Combines human-readable event-sourced ledgers with sub-millisecond FTS5 search and transactional SQLite queries; zero lock contention on the UI thread.
- **Cons**: `lp-store` must update the SQLite FTS5 index whenever `merge_findings` appends new nodes to `nodes.jsonl` (encapsulated inside `FsGraphStore::merge_and_index`).
