# Architectural Decisions (ADR) Index

Back to [Brain Index](/index.md). Related: [Architecture Index](/architecture/index.md), [Wisdom Index](/wisdom/index.md).

## Decisions
- [ADR-0001: Pure Rust CLI + Ratatui TUI Over Electron/Next.js](/decisions/0001-rust-tui-over-electron.md) — Replacing the 450 MB Electron + Chromium + Next.js stack with a single statically-linked Rust binary.
- [ADR-0002: Zero-Config Multi-Account CLI Auth & Failover Pool](/decisions/0002-cli-multi-account-pool.md) — Auto-discovering local `claude` (multiple subscription accounts via `CLAUDE_CONFIG_DIR`), `codex`, and `agy` logins with independent quota buckets.
- [ADR-0003: Hybrid Append-Only JSONL + SQLite WAL Storage](/decisions/0003-hybrid-jsonl-sqlite-storage.md) — Combining `learnharness` JSONL graph ledgers with `tracker` SQLite WAL + FTS5 and `ArcSwap` memory snapshots.
- [ADR-0004: Ponytail Skip List (Deliberate Non-Goals)](/decisions/0004-skip-list.md) — Explicit scope boundaries to prevent bloat.
