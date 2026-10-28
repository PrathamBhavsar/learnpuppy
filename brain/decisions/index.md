# Architectural Decisions (ADR) Index

Back to [Brain Index](/index.md). Related: [Architecture Index](/architecture/index.md), [Wisdom Index](/wisdom/index.md).

## Decisions
- [ADR-0001: Pure Rust Over Electron/Next.js](/decisions/0001-rust-tui-over-electron.md) — Replacing the 450 MB Electron + Chromium + Next.js stack with statically-linked Rust binaries.
- [ADR-0002: Zero-Config Multi-Account CLI Auth & Failover Pool](/decisions/0002-cli-multi-account-pool.md) — Auto-discovering local `claude` (multiple subscription accounts via `CLAUDE_CONFIG_DIR`), `codex`, and `agy` logins with independent quota buckets.
- [ADR-0003: Hybrid Append-Only JSONL + SQLite WAL Storage](/decisions/0003-hybrid-jsonl-sqlite-storage.md) — Combining `learnharness` JSONL graph ledgers with `tracker` SQLite WAL + FTS5 and `ArcSwap` memory snapshots.
- [ADR-0004: Ponytail Skip List (Deliberate Non-Goals)](/decisions/0004-skip-list.md) — Explicit scope boundaries to prevent bloat.
- [ADR-0005: Pivot Primary UI to Native Dioxus 0.7 + `lp-ui` (Isolate TUI on `legacy/tui-cli`)](/decisions/0005-dioxus-desktop-over-tui.md) — Building a visual macOS desktop app (`lp-app`) and pure-Rust shadcn component crate (`lp-ui`) first, Windows second (`T70`), and iOS/Android last (`T80`), while preserving the Ratatui CLI/TUI on branch `legacy/tui-cli`.
