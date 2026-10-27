---
type: spec
title: Hybrid Storage & Memory Model (lp-store)
description: Append-only JSONL ledgers combined with SQLite WAL + FTS5 indexing, atomic write-ahead durability, ArcSwap zero-copy TUI reads, and bounded channels.
tags: [storage, memory, sqlite, jsonl, fts5, arc-swap, concurrency]
timestamp: 2026-10-27
---

# Hybrid Storage & Memory Model (`lp-store`)

Back to [Architecture Index](/architecture/index.md). Related: [ADR-0003 Hybrid Storage](/decisions/0003-hybrid-jsonl-sqlite-storage.md), [Anti-Patterns](/wisdom/anti-patterns.md).

## 1. On-Disk Layout (`~/.learnpuppy/`)

LearnPuppy pairs **append-only JSONL ledgers** (for human auditability, Git/Syncthing sync, and zero-migration graph rebuilds) with **SQLite WAL** (`index.db`, via `rusqlite` bundled) for sub-millisecond full-text search, incremental ingest cursors, and FSRS queries:

| Path | Format | Purpose |
|---|---|---|
| `graph/nodes.jsonl` | Append-only JSONL | `GraphNode` birth lines and re-encounter link-append lines. |
| `graph/chains.jsonl` | Append-only JSONL | `GraphChain` (`arg_...`) assertions; rewritten atomically when `simplify_chains` drops redundant chains. |
| `graph/coverage.jsonl` | Append-only JSONL | Per-message extraction coverage (`{ chat_id, fp, tokens, folded_at }`). |
| `graph/sessions.jsonl` | Append-only JSONL | Checkpoints for one-shot imported notes/PDFs (`src_<hash>`). |
| `graph/positions.json` | Atomic JSON | Settled 2D ForceAtlas2 layout coordinates per node ID. |
| `chats/<id>.json` | Atomic JSON | Full `ChatRecord` (`messages`, `PlanTree`, `turns` usage log, `extractions` usage log). |
| `sync/sync-<device>.jsonl` | Append-only JSONL | Multi-device replication log for `tracker` flashcards and FSRS review events. |
| `index.db` | SQLite WAL | `propositions_fts` (FTS5), `transcripts_fts` (FTS5), `ingest_cursors`, `coding_sessions`, `decisions`, `book_chunks`, `flashcards`, `fsrs_reviews`, `account_usage_ledger`. |

## 2. Crash-Safe Atomic Persistence

- **Atomic File Replacement (`atomic_write_json`)**: Writes to `<target>.tmp.<pid>.<counter>`, calls `file.sync_all()`, and executes `std::fs::rename` over `<target>`.
- **Corrupt Tail Recovery on JSONL Ledgers**: When reading `.jsonl` files at startup, `lp-store` parses line-by-line; if the final line is truncated (e.g. from a mid-append power cut), it logs a warning, truncates the incomplete tail line, and preserves all preceding committed lines.
- **SQLite Pragmas**:
  ```sql
  PRAGMA journal_mode = WAL;
  PRAGMA synchronous = NORMAL;
  PRAGMA foreign_keys = ON;
  PRAGMA busy_timeout = 5000;
  ```

## 3. Memory & Concurrency Discipline

1. **Wait-Free TUI State via `ArcSwap`**:
   - `TuiAppState` reads `ArcSwap<GraphSnapshot>` and `ArcSwap<SessionListSnapshot>`. The 60fps render loop never blocks on a storage or extraction lock.
2. **Inline Identifier Interning (`SmolStr`)**:
   - All IDs (`n_<12hex>`, `arg_<12hex>`, `chat_id`, `tool_call_id`, `node_id`) use `smol_str::SmolStr` (23 bytes stack-inlined, `Clone` is a cheap `memcpy` with zero heap allocation).
3. **Bounded Channels Everywhere**:
   - `mpsc::channel(256)` for UI input/action events; `mpsc::channel(512)` for LLM streaming deltas; `mpsc::channel(64)` for background wake triggers.
4. **Line-Buffer Reuse in JSONL Ingestion**:
   - `lp-ingest` scans multi-MB `.jsonl` session transcripts using a single pre-allocated `String` buffer (`buf.clear(); reader.read_line(&mut buf)`), keeping peak heap usage under 35 MB.
